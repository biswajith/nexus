import { useState, useEffect, useRef, useCallback } from 'react';
import { EditorView, keymap, placeholder as placeholderExt } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { javascript } from '@codemirror/lang-javascript';
import { json } from '@codemirror/lang-json';
import { oneDark } from '@codemirror/theme-one-dark';
import { bracketMatching, foldGutter } from '@codemirror/language';
import styles from './GraphQLPanel.module.css';

interface GraphQLResponse {
  data?: unknown;
  errors?: Array<{ message: string; locations?: Array<{ line: number; column: number }>; path?: Array<string | number> }>;
  status: number;
  responseTime: number;
}

interface SubscriptionMessage {
  id: string;
  timestamp: number;
  data?: unknown;
  errors?: Array<{ message: string }>;
}

type ResponseTab = 'response' | 'schema' | 'subscription';

const DEFAULT_QUERY = `# Enter your GraphQL query
query {
  
}`;

export function GraphQLPanel() {
  const [url, setUrl] = useState('');
  const [response, setResponse] = useState<GraphQLResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [schemaSDL, setSchemaSDL] = useState('');
  const [schemaLoading, setSchemaLoading] = useState(false);
  const [responseTab, setResponseTab] = useState<ResponseTab>('response');
  const [subscriptionMessages, setSubscriptionMessages] = useState<SubscriptionMessage[]>([]);
  const [subscriptionActive, setSubscriptionActive] = useState(false);

  const queryEditorRef = useRef<HTMLDivElement>(null);
  const variablesEditorRef = useRef<HTMLDivElement>(null);
  const queryViewRef = useRef<EditorView | null>(null);
  const variablesViewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!queryEditorRef.current) return;
    const view = new EditorView({
      state: EditorState.create({
        doc: DEFAULT_QUERY,
        extensions: [
          javascript(),
          oneDark,
          history(),
          bracketMatching(),
          foldGutter(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          placeholderExt('query { ... }'),
          EditorView.lineWrapping,
        ],
      }),
      parent: queryEditorRef.current,
    });
    queryViewRef.current = view;
    return () => view.destroy();
  }, []);

  useEffect(() => {
    if (!variablesEditorRef.current) return;
    const view = new EditorView({
      state: EditorState.create({
        doc: '{}',
        extensions: [
          json(),
          oneDark,
          history(),
          bracketMatching(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          placeholderExt('{ "key": "value" }'),
          EditorView.lineWrapping,
        ],
      }),
      parent: variablesEditorRef.current,
    });
    variablesViewRef.current = view;
    return () => view.destroy();
  }, []);

  useEffect(() => {
    const cleanups = [
      window.nexus.graphql.onSubscriptionData((data: unknown) => {
        setSubscriptionMessages((prev) => [...prev, data as SubscriptionMessage]);
      }),
      window.nexus.graphql.onSubscriptionError((err: unknown) => {
        setSubscriptionMessages((prev) => [...prev, {
          id: crypto.randomUUID(),
          timestamp: Date.now(),
          errors: [{ message: String(err) }],
        }]);
      }),
      window.nexus.graphql.onSubscriptionComplete(() => {
        setSubscriptionActive(false);
      }),
    ];
    return () => cleanups.forEach((c) => c());
  }, []);

  const getQuery = useCallback(() => {
    return queryViewRef.current?.state.doc.toString() ?? '';
  }, []);

  const getVariables = useCallback(() => {
    const raw = variablesViewRef.current?.state.doc.toString() ?? '{}';
    try { return JSON.parse(raw); }
    catch { return {}; }
  }, []);

  const handleSend = useCallback(async () => {
    if (!url.trim()) return;
    setLoading(true);
    setResponse(null);
    try {
      const result = await window.nexus.graphql.send({
        url,
        query: getQuery(),
        variables: getVariables(),
      }) as GraphQLResponse;
      setResponse(result);
      setResponseTab('response');
    } catch (err) {
      setResponse({
        errors: [{ message: err instanceof Error ? err.message : String(err) }],
        status: 0,
        responseTime: 0,
      });
    } finally {
      setLoading(false);
    }
  }, [url, getQuery, getVariables]);

  const handleIntrospect = useCallback(async () => {
    if (!url.trim()) return;
    setSchemaLoading(true);
    try {
      const sdl = await window.nexus.graphql.introspect(url) as string;
      setSchemaSDL(sdl);
      setResponseTab('schema');
    } catch (err) {
      setSchemaSDL(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setSchemaLoading(false);
    }
  }, [url]);

  const handleSubscribe = useCallback(async () => {
    if (!url.trim()) return;
    setSubscriptionMessages([]);
    setSubscriptionActive(true);
    setResponseTab('subscription');
    await window.nexus.graphql.subscribe({
      url,
      query: getQuery(),
      variables: getVariables(),
    });
  }, [url, getQuery, getVariables]);

  const handleUnsubscribe = useCallback(async () => {
    await window.nexus.graphql.unsubscribe();
    setSubscriptionActive(false);
  }, []);

  return (
    <div className={styles.panel}>
      <div className={styles.toolbar}>
        <input
          className={styles.urlInput}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://api.example.com/graphql"
        />
        <button
          type="button"
          className={styles.primaryBtn}
          onClick={handleSend}
          disabled={loading || !url.trim()}
        >
          {loading ? 'Sending...' : 'Send'}
        </button>
        <button
          type="button"
          className={styles.secondaryBtn}
          onClick={handleIntrospect}
          disabled={schemaLoading || !url.trim()}
        >
          {schemaLoading ? 'Loading...' : 'Schema'}
        </button>
        {subscriptionActive ? (
          <button type="button" className={styles.dangerBtn} onClick={handleUnsubscribe}>
            Stop
          </button>
        ) : (
          <button
            type="button"
            className={styles.secondaryBtn}
            onClick={handleSubscribe}
            disabled={!url.trim()}
          >
            Subscribe
          </button>
        )}
      </div>

      <div className={styles.body}>
        <div className={styles.editorPane}>
          <div className={styles.paneHeader}>Query</div>
          <div className={styles.editorContainer} ref={queryEditorRef} />
          <div className={styles.variablesContainer}>
            <div className={styles.paneHeader}>Variables</div>
            <div style={{ height: 120, overflow: 'auto' }} ref={variablesEditorRef} />
          </div>
        </div>

        <div className={styles.responsePane}>
          <div className={styles.paneHeader}>
            <div className={styles.tabRow}>
              {(['response', 'schema', 'subscription'] as const).map((tab) => (
                <button
                  key={tab}
                  className={`${styles.tabBtn} ${responseTab === tab ? styles.tabBtnActive : ''}`}
                  onClick={() => setResponseTab(tab)}
                >
                  {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  {tab === 'subscription' && subscriptionActive && ' ●'}
                </button>
              ))}
            </div>
          </div>

          <div className={styles.paneContent}>
            {responseTab === 'response' && (
              response ? (
                <div>
                  {response.errors && response.errors.length > 0 && (
                    <div>
                      {response.errors.map((err, i) => (
                        <div key={i} className={styles.errorBlock}>{err.message}</div>
                      ))}
                    </div>
                  )}
                  {response.data !== undefined && response.data !== null ? (
                    <div className={styles.responseBody}>
                      {JSON.stringify(response.data, null, 2)}
                    </div>
                  ) : null}
                </div>
              ) : (
                <div className={styles.empty}>
                  {loading ? <span className={styles.spinner} /> : 'Send a query to see the response'}
                </div>
              )
            )}

            {responseTab === 'schema' && (
              schemaSDL ? (
                <div className={styles.schemaPanel} style={{ maxHeight: 'none', flex: 1 }}>
                  {schemaSDL}
                </div>
              ) : (
                <div className={styles.empty}>
                  {schemaLoading ? <span className={styles.spinner} /> : 'Click "Schema" to introspect'}
                </div>
              )
            )}

            {responseTab === 'subscription' && (
              subscriptionMessages.length > 0 ? (
                <div className={styles.subscriptionMessages}>
                  {subscriptionMessages.map((msg) => (
                    <div key={msg.id} className={styles.subMessage}>
                      <div className={styles.subMessageTime}>
                        {new Date(msg.timestamp).toLocaleTimeString()}
                      </div>
                      {msg.errors ? (
                        <div className={styles.errorBlock}>
                          {msg.errors.map((e) => e.message).join(', ')}
                        </div>
                      ) : (
                        <div className={styles.subMessageData}>
                          {JSON.stringify(msg.data, null, 2)}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <div className={styles.empty}>
                  {subscriptionActive ? 'Waiting for subscription data...' : 'Subscribe to see real-time data'}
                </div>
              )
            )}
          </div>

          {response && responseTab === 'response' && (
            <div className={styles.statusBar}>
              <span className={`${styles.statusBadge} ${response.status >= 200 && response.status < 300 ? styles.statusSuccess : styles.statusError}`}>
                {response.status}
              </span>
              <span>{response.responseTime}ms</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
