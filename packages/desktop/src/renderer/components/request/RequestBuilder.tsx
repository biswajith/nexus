import { useState, useCallback, useEffect, useRef } from 'react';
import type { AuthConfig, HttpMethod, KeyValuePair, RequestBody, NexusRequest } from '@nexus/core';
import { useRequestStore } from '../../stores/request-store.js';
import { useCollectionStore } from '../../stores/collection-store.js';
import { KeyValueEditor } from './KeyValueEditor.js';
import { BodyEditor } from './BodyEditor.js';
import { AuthEditor } from './AuthEditor.js';
import { ScriptPanel } from '../scripts/ScriptPanel.js';
import styles from './RequestBuilder.module.css';

interface CollectionOrigin {
  dirName: string;
  requestId: string;
}

interface RequestBuilderProps {
  tab: {
    id: string;
    request: {
      name?: string;
      method: HttpMethod;
      url: string;
      headers: KeyValuePair[];
      params: KeyValuePair[];
      body: RequestBody;
      auth: AuthConfig;
      preRequestScript?: string;
      postResponseScript?: string;
    };
    loading: boolean;
    origin?: CollectionOrigin;
  };
}

const METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

const METHOD_COLORS: Record<string, string> = {
  GET: 'var(--nx-method-get)',
  POST: 'var(--nx-method-post)',
  PUT: 'var(--nx-method-put)',
  PATCH: 'var(--nx-method-patch)',
  DELETE: 'var(--nx-method-delete)',
  HEAD: 'var(--nx-method-head)',
  OPTIONS: 'var(--nx-method-options)',
};

type RequestTab = 'params' | 'headers' | 'body' | 'auth' | 'scripts';

/** Edits the active tab's URL, method, send/cURL/save actions, and sub-panels (params through scripts). */
export function RequestBuilder({ tab }: RequestBuilderProps) {
  const [activeTab, setActiveTab] = useState<RequestTab>('params');
  const setMethod = useRequestStore((s) => s.setMethod);
  const setUrl = useRequestStore((s) => s.setUrl);
  const setHeaders = useRequestStore((s) => s.setHeaders);
  const setParams = useRequestStore((s) => s.setParams);
  const setBody = useRequestStore((s) => s.setBody);
  const setAuth = useRequestStore((s) => s.setAuth);
  const updateRequest = useRequestStore((s) => s.updateRequest);
  const sendRequest = useRequestStore((s) => s.sendRequest);
  const cancelRequest = useRequestStore((s) => s.cancelRequest);
  const [curlCopied, setCurlCopied] = useState(false);
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState<string | null>(null);
  const [savingToDirName, setSavingToDirName] = useState<string | null>(null);
  const [saveNameInput, setSaveNameInput] = useState('');
  const saveRef = useRef<HTMLDivElement>(null);
  const saveNameRef = useRef<HTMLInputElement>(null);
  const collections = useCollectionStore((s) => s.collections);
  const addRequestToCollection = useCollectionStore((s) => s.addRequest);
  const updateRequestInCollection = useCollectionStore((s) => s.updateRequestInCollection);
  const setOrigin = useRequestStore((s) => s.setOrigin);

  const origin = tab.origin;

  useEffect(() => {
    if (!saveOpen) return;
    const close = (e: MouseEvent) => {
      if (saveRef.current && !saveRef.current.contains(e.target as Node)) {
        setSaveOpen(false);
        setSavingToDirName(null);
      }
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [saveOpen]);

  useEffect(() => {
    if (savingToDirName && saveNameRef.current) {
      saveNameRef.current.focus();
    }
  }, [savingToDirName]);

  const getLatestRequest = useCallback((): NexusRequest => {
    const storeTab = useRequestStore.getState().tabs.find((t) => t.id === tab.id);
    if (storeTab) return storeTab.request;
    return {
      id: tab.id,
      name: tab.request.name ?? 'Untitled',
      method: tab.request.method,
      url: tab.request.url,
      headers: tab.request.headers,
      params: tab.request.params,
      body: tab.request.body,
      auth: tab.request.auth,
      preRequestScript: tab.request.preRequestScript,
      postResponseScript: tab.request.postResponseScript,
      settings: {},
    };
  }, [tab.id]);

  const handleSave = useCallback(async () => {
    if (origin) {
      const request = getLatestRequest();
      await updateRequestInCollection(origin.dirName, origin.requestId, { ...request, id: origin.requestId });
      setSaveSuccess('updated');
      setTimeout(() => setSaveSuccess(null), 2000);
    } else {
      setSaveOpen(!saveOpen);
    }
  }, [origin, getLatestRequest, updateRequestInCollection, saveOpen]);

  const handlePickCollection = useCallback((dirName: string) => {
    setSavingToDirName(dirName);
    setSaveNameInput('');
  }, []);

  const handleConfirmSave = useCallback(async () => {
    if (!savingToDirName) return;
    const name = saveNameInput.trim() || 'Untitled';
    const request = { ...getLatestRequest(), id: crypto.randomUUID(), name };
    await addRequestToCollection(savingToDirName, request);
    setOrigin(tab.id, { dirName: savingToDirName, requestId: request.id });
    updateRequest(tab.id, { name });
    setSaveOpen(false);
    setSavingToDirName(null);
    setSaveSuccess(savingToDirName);
    setTimeout(() => setSaveSuccess(null), 2000);
  }, [savingToDirName, saveNameInput, tab.id, getLatestRequest, addRequestToCollection, setOrigin, updateRequest]);

  const paramCount = tab.request.params.filter((p) => p.enabled && p.key).length;
  const headerCount = tab.request.headers.filter((h) => h.enabled && h.key).length;

  const handlePaste = useCallback(async (e: React.ClipboardEvent<HTMLInputElement>) => {
    const text = e.clipboardData.getData('text').trim();
    if (!text.startsWith('curl ') && !text.startsWith('curl\t')) return;

    e.preventDefault();
    try {
      const parsed = await window.nexus.import.curl(text) as NexusRequest;
      updateRequest(tab.id, {
        method: parsed.method,
        url: parsed.url,
        headers: parsed.headers,
        params: parsed.params,
        body: parsed.body,
      });
    } catch {
      setUrl(tab.id, text);
    }
  }, [tab.id, updateRequest, setUrl]);

  const handleCopyCurl = useCallback(async () => {
    const nexusReq = {
      id: tab.id,
      name: 'request',
      method: tab.request.method,
      url: tab.request.url,
      headers: tab.request.headers,
      params: tab.request.params,
      body: tab.request.body,
      auth: tab.request.auth,
      settings: {},
    };
    const curl = await window.nexus.codegen.curl(nexusReq) as string;
    await navigator.clipboard.writeText(curl);
    setCurlCopied(true);
    setTimeout(() => setCurlCopied(false), 2000);
  }, [tab]);

  return (
    <div className={styles.builder}>
      <div className={styles.urlBar}>
        <select
          className={styles.methodSelect}
          value={tab.request.method}
          onChange={(e) => setMethod(tab.id, e.target.value as HttpMethod)}
          style={{ color: METHOD_COLORS[tab.request.method] }}
        >
          {METHODS.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
        <input
          className={styles.urlInput}
          type="text"
          placeholder="Enter request URL or paste cURL"
          value={tab.request.url}
          onChange={(e) => setUrl(tab.id, e.target.value)}
          onPaste={handlePaste}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !tab.loading) {
              sendRequest(tab.id);
            }
          }}
          spellCheck={false}
        />
        {tab.loading ? (
          <button className={styles.cancelBtn} onClick={() => cancelRequest(tab.id)}>
            Cancel
          </button>
        ) : (
          <button className={styles.sendBtn} onClick={() => sendRequest(tab.id)}>
            Send
          </button>
        )}
        <button className={styles.curlBtn} onClick={handleCopyCurl}>
          {curlCopied ? 'Copied!' : 'cURL'}
        </button>
        <div className={styles.saveWrapper} ref={saveRef}>
          <button
            className={`${styles.saveBtn} ${origin ? styles.saveBtnBound : ''}`}
            onClick={handleSave}
            title={origin ? `Update in ${origin.dirName}` : 'Save to collection'}
          >
            {saveSuccess ? '✓ Saved' : origin ? 'Save' : 'Save ▾'}
          </button>
          {!origin && saveOpen && (
            <div className={styles.saveDropdown}>
              {savingToDirName ? (
                <div className={styles.saveNameForm}>
                  <label className={styles.saveNameLabel}>Request name</label>
                  <input
                    ref={saveNameRef}
                    className={styles.saveNameInput}
                    type="text"
                    placeholder="e.g. Get Users"
                    value={saveNameInput}
                    onChange={(e) => setSaveNameInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleConfirmSave();
                      if (e.key === 'Escape') { setSavingToDirName(null); }
                    }}
                  />
                  <div className={styles.saveNameActions}>
                    <button
                      className={styles.saveNameBack}
                      onClick={() => setSavingToDirName(null)}
                    >
                      ← Back
                    </button>
                    <button
                      className={styles.saveNameConfirm}
                      onClick={handleConfirmSave}
                      disabled={!saveNameInput.trim()}
                    >
                      Save
                    </button>
                  </div>
                </div>
              ) : collections.length === 0 ? (
                <div className={styles.saveEmpty}>No collections yet</div>
              ) : (
                collections.map((col) => {
                  const dirName = col.path.split('/').pop() ?? col.path;
                  return (
                    <button
                      key={col.id}
                      className={styles.saveItem}
                      onClick={() => handlePickCollection(dirName)}
                    >
                      {col.name}
                    </button>
                  );
                })
              )}
            </div>
          )}
        </div>
      </div>

      <div className={styles.tabRow}>
        {(['params', 'headers', 'body', 'auth', 'scripts'] as const).map((t) => (
          <button
            key={t}
            className={`${styles.tabBtn} ${activeTab === t ? styles.activeTab : ''}`}
            onClick={() => setActiveTab(t)}
          >
            {t === 'params' ? `Params${paramCount > 0 ? ` (${paramCount})` : ''}` : ''}
            {t === 'headers' ? `Headers${headerCount > 0 ? ` (${headerCount})` : ''}` : ''}
            {t === 'body' ? 'Body' : ''}
            {t === 'auth' ? 'Auth' : ''}
            {t === 'scripts' ? 'Scripts' : ''}
          </button>
        ))}
      </div>

      <div className={styles.tabContent}>
        {activeTab === 'params' && (
          <KeyValueEditor
            pairs={tab.request.params}
            onChange={(params) => setParams(tab.id, params)}
            namePlaceholder="Parameter name"
            valuePlaceholder="Value"
          />
        )}
        {activeTab === 'headers' && (
          <KeyValueEditor
            pairs={tab.request.headers}
            onChange={(headers) => setHeaders(tab.id, headers)}
            namePlaceholder="Header name"
            valuePlaceholder="Value"
          />
        )}
        {activeTab === 'body' && (
          <BodyEditor
            body={tab.request.body}
            onChange={(body) => setBody(tab.id, body)}
          />
        )}
        {activeTab === 'auth' && (
          <AuthEditor auth={tab.request.auth} onChange={(auth) => setAuth(tab.id, auth)} />
        )}
        {activeTab === 'scripts' && (
          <ScriptPanel
            preRequestScript={tab.request.preRequestScript ?? ''}
            postResponseScript={tab.request.postResponseScript ?? ''}
            onPreRequestChange={(script) => updateRequest(tab.id, { preRequestScript: script })}
            onPostResponseChange={(script) => updateRequest(tab.id, { postResponseScript: script })}
          />
        )}
      </div>
    </div>
  );
}
