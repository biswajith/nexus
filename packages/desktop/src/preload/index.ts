import { contextBridge, ipcRenderer } from 'electron';

/** Preload bridge exposing IPC-backed methods to the renderer process. */
const api = {
  http: {
    send: (opts: unknown) => ipcRenderer.invoke('http:send', opts),
  },
  variables: {
    resolve: (template: string, scopes: unknown) => ipcRenderer.invoke('variables:resolve', template, scopes),
  },
  collections: {
    list: () => ipcRenderer.invoke('collections:list'),
    load: (dirName: string) => ipcRenderer.invoke('collections:load', dirName),
    create: (collection: unknown) => ipcRenderer.invoke('collections:create', collection),
    delete: (dirName: string) => ipcRenderer.invoke('collections:delete', dirName),
    rename: (dirName: string, newName: string) => ipcRenderer.invoke('collections:rename', dirName, newName),
    addRequest: (dirName: string, request: unknown) => ipcRenderer.invoke('collections:addRequest', dirName, request),
    updateRequest: (dirName: string, requestId: string, request: unknown) => ipcRenderer.invoke('collections:updateRequest', dirName, requestId, request),
    deleteRequest: (dirName: string, requestId: string) => ipcRenderer.invoke('collections:deleteRequest', dirName, requestId),
  },
  environments: {
    list: () => ipcRenderer.invoke('environments:list'),
    create: (env: unknown) => ipcRenderer.invoke('environments:create', env),
    update: (env: unknown) => ipcRenderer.invoke('environments:update', env),
    delete: (envId: string) => ipcRenderer.invoke('environments:delete', envId),
  },
  history: {
    log: (entry: unknown) => ipcRenderer.invoke('history:log', entry),
    query: (filters?: unknown) => ipcRenderer.invoke('history:query', filters),
    clear: (before?: string) => ipcRenderer.invoke('history:clear', before),
  },
  import: {
    detectFormat: (data: string) => ipcRenderer.invoke('import:detect-format', data),
    file: (data: string, format?: string) => ipcRenderer.invoke('import:file', data, format),
    save: (collections: unknown[], environments: unknown[]) => ipcRenderer.invoke('import:save', collections, environments),
    curl: (curlCommand: string) => ipcRenderer.invoke('import:curl', curlCommand),
  },
  export: {
    collection: (collection: unknown, format: string) => ipcRenderer.invoke('export:collection', collection, format),
    environment: (environment: unknown, format: string) => ipcRenderer.invoke('export:environment', environment, format),
  },
  runner: {
    start: (config: unknown) => ipcRenderer.invoke('runner:start', config),
    cancel: () => ipcRenderer.invoke('runner:cancel'),
    onEvent: (callback: (event: unknown) => void) => {
      ipcRenderer.on('runner:event', (_e, data) => callback(data));
      return () => { ipcRenderer.removeAllListeners('runner:event'); };
    },
  },
  ws: {
    connect: (id: string, opts: unknown) => ipcRenderer.invoke('ws:connect', id, opts),
    send: (id: string, data: string) => ipcRenderer.invoke('ws:send', id, data),
    disconnect: (id: string) => ipcRenderer.invoke('ws:disconnect', id),
    messages: (id: string) => ipcRenderer.invoke('ws:messages', id),
    onStatus: (callback: (data: unknown) => void) => {
      ipcRenderer.on('ws:status', (_e, d) => callback(d));
      return () => { ipcRenderer.removeAllListeners('ws:status'); };
    },
    onMessage: (callback: (data: unknown) => void) => {
      ipcRenderer.on('ws:message', (_e, d) => callback(d));
      return () => { ipcRenderer.removeAllListeners('ws:message'); };
    },
    onClose: (callback: (data: unknown) => void) => {
      ipcRenderer.on('ws:close', (_e, d) => callback(d));
      return () => { ipcRenderer.removeAllListeners('ws:close'); };
    },
    onError: (callback: (data: unknown) => void) => {
      ipcRenderer.on('ws:error', (_e, d) => callback(d));
      return () => { ipcRenderer.removeAllListeners('ws:error'); };
    },
  },
  mcp: {
    connect: (id: string, config: unknown) => ipcRenderer.invoke('mcp:connect', id, config),
    listTools: (id: string) => ipcRenderer.invoke('mcp:listTools', id),
    callTool: (id: string, toolName: string, args: unknown) => ipcRenderer.invoke('mcp:callTool', id, toolName, args),
    listResources: (id: string) => ipcRenderer.invoke('mcp:listResources', id),
    listPrompts: (id: string) => ipcRenderer.invoke('mcp:listPrompts', id),
    disconnect: (id: string) => ipcRenderer.invoke('mcp:disconnect', id),
  },
  graphql: {
    send: (opts: unknown) => ipcRenderer.invoke('graphql:send', opts),
    introspect: (url: string, headers?: Record<string, string>) => ipcRenderer.invoke('graphql:introspect', url, headers),
    subscribe: (opts: unknown) => ipcRenderer.invoke('graphql:subscribe', opts),
    unsubscribe: () => ipcRenderer.invoke('graphql:unsubscribe'),
    onSubscriptionData: (callback: (data: unknown) => void) => {
      ipcRenderer.on('graphql:subscription-data', (_e, d) => callback(d));
      return () => { ipcRenderer.removeAllListeners('graphql:subscription-data'); };
    },
    onSubscriptionError: (callback: (data: unknown) => void) => {
      ipcRenderer.on('graphql:subscription-error', (_e, d) => callback(d));
      return () => { ipcRenderer.removeAllListeners('graphql:subscription-error'); };
    },
    onSubscriptionComplete: (callback: (data: unknown) => void) => {
      ipcRenderer.on('graphql:subscription-complete', (_e, d) => callback(d));
      return () => { ipcRenderer.removeAllListeners('graphql:subscription-complete'); };
    },
  },
  codegen: {
    curl: (request: unknown) => ipcRenderer.invoke('codegen:curl', request),
  },
  docs: {
    markdown: (collection: unknown) => ipcRenderer.invoke('docs:markdown', collection),
    html: (collection: unknown) => ipcRenderer.invoke('docs:html', collection),
  },
} as const;

export type NexusApi = typeof api;

contextBridge.exposeInMainWorld('nexus', api);
