import { create } from 'zustand';
import type {
  NexusRequest,
  HttpMethod,
  KeyValuePair,
  RequestBody,
  AuthConfig,
  ResponseTiming,
  HistoryEntry,
} from '@nexus/core';
import { useHistoryStore } from './history-store.js';

interface TestResultItem {
  name: string;
  passed: boolean;
  error?: string;
  duration: number;
}

interface SerializedResponse {
  status: number;
  statusText: string;
  headers: Record<string, string | string[]>;
  bodyText: string;
  bodyJson?: unknown;
  timing: ResponseTiming;
  size: number;
  testResults?: TestResultItem[];
  scriptLogs?: Array<{ level: string; args: unknown[]; timestamp: number }>;
  visualizerHtml?: string;
}

interface CollectionOrigin {
  dirName: string;
  requestId: string;
}

interface RequestTab {
  id: string;
  request: NexusRequest;
  response: SerializedResponse | null;
  loading: boolean;
  dirty: boolean;
  origin?: CollectionOrigin;
}

interface RequestState {
  tabs: RequestTab[];
  activeTabId: string | null;

  openTab: (request?: Partial<NexusRequest>, origin?: CollectionOrigin) => void;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  setOrigin: (tabId: string, origin: CollectionOrigin) => void;
  updateRequest: (id: string, updates: Partial<NexusRequest>) => void;
  setMethod: (id: string, method: HttpMethod) => void;
  setUrl: (id: string, url: string) => void;
  setHeaders: (id: string, headers: KeyValuePair[]) => void;
  setParams: (id: string, params: KeyValuePair[]) => void;
  setBody: (id: string, body: RequestBody) => void;
  setAuth: (id: string, auth: AuthConfig) => void;
  sendRequest: (id: string) => Promise<void>;
  cancelRequest: (id: string) => void;
}

const abortControllers = new Map<string, AbortController>();

/** Open request tabs, edits, send/cancel lifecycle, and last response payload per tab. */
export const useRequestStore = create<RequestState>((set, get) => ({
  tabs: [],
  activeTabId: null,

  openTab: (partial, origin) => {
    const id = crypto.randomUUID();
    const request: NexusRequest = {
      id,
      name: partial?.name ?? 'New Request',
      method: partial?.method ?? 'GET',
      url: partial?.url ?? '',
      headers: partial?.headers ?? [],
      params: partial?.params ?? [],
      body: partial?.body ?? { mode: 'none' },
      auth: partial?.auth ?? { type: 'inherit' },
      settings: partial?.settings ?? {},
      ...partial,
    };
    set((state) => ({
      tabs: [...state.tabs, { id, request, response: null, loading: false, dirty: false, origin }],
      activeTabId: id,
    }));
  },

  closeTab: (id) => {
    abortControllers.get(id)?.abort();
    abortControllers.delete(id);
    set((state) => {
      const tabs = state.tabs.filter((t) => t.id !== id);
      const activeTabId =
        state.activeTabId === id ? (tabs[tabs.length - 1]?.id ?? null) : state.activeTabId;
      return { tabs, activeTabId };
    });
  },

  setActiveTab: (id) => set({ activeTabId: id }),

  setOrigin: (tabId, origin) =>
    set((state) => ({
      tabs: state.tabs.map((t) =>
        t.id === tabId ? { ...t, origin } : t,
      ),
    })),

  updateRequest: (id, updates) =>
    set((state) => ({
      tabs: state.tabs.map((t) =>
        t.id === id ? { ...t, request: { ...t.request, ...updates }, dirty: true } : t,
      ),
    })),

  setMethod: (id, method) => get().updateRequest(id, { method }),
  setUrl: (id, url) => get().updateRequest(id, { url }),
  setHeaders: (id, headers) => get().updateRequest(id, { headers }),
  setParams: (id, params) => get().updateRequest(id, { params }),
  setBody: (id, body) => get().updateRequest(id, { body }),
  setAuth: (id, auth) => get().updateRequest(id, { auth }),

  sendRequest: async (id) => {
    const tab = get().tabs.find((t) => t.id === id);
    if (!tab) return;

    const controller = new AbortController();
    abortControllers.set(id, controller);

    set((state) => ({
      tabs: state.tabs.map((t) =>
        t.id === id ? { ...t, loading: true, response: null } : t,
      ),
    }));

    const { request } = tab;
    const headers: Record<string, string> = {};
    for (const h of request.headers) {
      if (h.enabled && h.key) headers[h.key] = h.value;
    }

    let url = request.url;
    const enabledParams = request.params.filter((p) => p.enabled && p.key);
    if (enabledParams.length > 0) {
      const sep = url.includes('?') ? '&' : '?';
      const qs = enabledParams
        .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
        .join('&');
      url = `${url}${sep}${qs}`;
    }

    let body: string | null = null;
    if (request.body.mode === 'json' || request.body.mode === 'xml' || request.body.mode === 'text' || request.body.mode === 'html') {
      body = request.body.raw ?? null;
      if (request.body.mode === 'json' && !headers['content-type']) {
        headers['content-type'] = 'application/json';
      }
    } else if (request.body.mode === 'x-www-form-urlencoded' && request.body.urlencoded) {
      body = request.body.urlencoded
        .filter((p) => p.enabled)
        .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
        .join('&');
      if (!headers['content-type']) {
        headers['content-type'] = 'application/x-www-form-urlencoded';
      }
    }

    try {
      const response = await window.nexus.http.send({
        method: request.method,
        url,
        headers,
        body,
        settings: request.settings,
        auth: request.auth,
        preRequestScript: request.preRequestScript,
        postResponseScript: request.postResponseScript,
      });

      set((state) => ({
        tabs: state.tabs.map((t) =>
          t.id === id ? { ...t, loading: false, response } : t,
        ),
      }));

      const historyEntry: HistoryEntry = {
        id: crypto.randomUUID(),
        timestamp: Date.now(),
        request: { method: request.method, url, headers, body: body ?? undefined },
        response: {
          status: response.status,
          statusText: response.statusText,
          responseTime: response.timing?.total ?? 0,
          size: response.size ?? 0,
        },
      };
      void useHistoryStore.getState().logEntry(historyEntry);
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : 'Request failed';
      set((state) => ({
        tabs: state.tabs.map((t) =>
          t.id === id
            ? {
                ...t,
                loading: false,
                response: {
                  status: 0,
                  statusText: errorMsg,
                  headers: {},
                  bodyText: errorMsg,
                  timing: { dns: 0, tcp: 0, tls: 0, ttfb: 0, download: 0, total: 0 },
                  size: 0,
                },
              }
            : t,
        ),
      }));

      const historyEntry: HistoryEntry = {
        id: crypto.randomUUID(),
        timestamp: Date.now(),
        request: { method: request.method, url, headers, body: body ?? undefined },
        response: { status: 0, statusText: errorMsg, responseTime: 0, size: 0 },
      };
      void useHistoryStore.getState().logEntry(historyEntry);
    } finally {
      abortControllers.delete(id);
    }
  },

  cancelRequest: (id) => {
    abortControllers.get(id)?.abort();
    abortControllers.delete(id);
    set((state) => ({
      tabs: state.tabs.map((t) =>
        t.id === id ? { ...t, loading: false } : t,
      ),
    }));
  },
}));
