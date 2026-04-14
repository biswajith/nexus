import { useState, useEffect, useRef, useCallback } from 'react';
import styles from './WebSocketPanel.module.css';

interface WsMessage {
  id: string;
  direction: 'sent' | 'received';
  timestamp: number;
  type: 'text' | 'binary';
  data: string;
  size: number;
}

type WsStatus = 'idle' | 'connecting' | 'open' | 'closing' | 'closed' | 'error';

export function WebSocketPanel() {
  const [url, setUrl] = useState('ws://localhost:8080');
  const [status, setStatus] = useState<WsStatus>('idle');
  const [messages, setMessages] = useState<WsMessage[]>([]);
  const [draft, setDraft] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const connectionId = useRef(`ws_${Date.now()}`);

  useEffect(() => {
    const cleanups = [
      window.nexus.ws.onStatus((data: unknown) => {
        const d = data as { id: string; status: WsStatus };
        if (d.id === connectionId.current) setStatus(d.status);
      }),
      window.nexus.ws.onMessage((data: unknown) => {
        const d = data as { id: string; message: WsMessage };
        if (d.id === connectionId.current) {
          setMessages((prev) => [...prev, d.message]);
        }
      }),
      window.nexus.ws.onClose((data: unknown) => {
        const d = data as { id: string };
        if (d.id === connectionId.current) setStatus('closed');
      }),
      window.nexus.ws.onError(() => {}),
    ];
    return () => cleanups.forEach((c) => c());
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleConnect = useCallback(() => {
    connectionId.current = `ws_${Date.now()}`;
    setMessages([]);
    window.nexus.ws.connect(connectionId.current, { url });
  }, [url]);

  const handleDisconnect = useCallback(() => {
    window.nexus.ws.disconnect(connectionId.current);
  }, []);

  const handleSend = useCallback(() => {
    if (!draft.trim() || status !== 'open') return;
    window.nexus.ws.send(connectionId.current, draft).then((msg: unknown) => {
      setMessages((prev) => [...prev, msg as WsMessage]);
    });
    setDraft('');
  }, [draft, status]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }, [handleSend]);

  const isConnected = status === 'open';

  return (
    <div className={styles.panel}>
      <div className={styles.connectionBar}>
        <span className={`${styles.statusDot} ${statusClass(status)}`} title={status} />
        <input
          className={styles.urlInput}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="ws://localhost:8080"
          disabled={isConnected}
        />
        {isConnected ? (
          <button type="button" className={styles.dangerBtn} onClick={handleDisconnect}>
            Disconnect
          </button>
        ) : (
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={handleConnect}
            disabled={!url.trim() || status === 'connecting'}
          >
            Connect
          </button>
        )}
      </div>

      {messages.length > 0 ? (
        <div className={styles.messages}>
          {messages.map((msg) => (
            <div key={msg.id} className={styles.message}>
              <span className={`${styles.msgDirection} ${msg.direction === 'sent' ? styles.msgSent : styles.msgReceived}`}>
                {msg.direction === 'sent' ? '→' : '←'}
              </span>
              <span className={styles.msgTime}>
                {new Date(msg.timestamp).toLocaleTimeString()}
              </span>
              <span className={styles.msgData}>{msg.data}</span>
            </div>
          ))}
          <div ref={messagesEndRef} />
        </div>
      ) : (
        <div className={styles.empty}>
          {isConnected ? 'Waiting for messages...' : 'Connect to a WebSocket server to start'}
        </div>
      )}

      <div className={styles.composer}>
        <textarea
          className={styles.messageInput}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={isConnected ? 'Type a message and press Enter...' : 'Connect first...'}
          disabled={!isConnected}
          rows={1}
        />
        <button
          type="button"
          className={styles.primaryBtn}
          onClick={handleSend}
          disabled={!isConnected || !draft.trim()}
        >
          Send
        </button>
      </div>
    </div>
  );
}

function statusClass(status: WsStatus): string {
  switch (status) {
    case 'idle': return styles.statusIdle ?? '';
    case 'connecting': return styles.statusConnecting ?? '';
    case 'open': return styles.statusOpen ?? '';
    case 'closing': return styles.statusClosing ?? '';
    case 'closed': return styles.statusClosed ?? '';
    case 'error': return styles.statusError ?? '';
    default: return '';
  }
}
