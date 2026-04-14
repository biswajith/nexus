import { useEffect, useMemo } from 'react';
import { List } from 'react-window';
import { useHistoryStore } from '../../stores/history-store.js';
import { useRequestStore } from '../../stores/request-store.js';
import styles from './HistoryList.module.css';
import type { CSSProperties, ReactElement } from 'react';

const METHOD_COLORS: Record<string, string> = {
  GET: 'var(--nx-method-get)',
  POST: 'var(--nx-method-post)',
  PUT: 'var(--nx-method-put)',
  PATCH: 'var(--nx-method-patch)',
  DELETE: 'var(--nx-method-delete)',
  HEAD: 'var(--nx-method-head)',
  OPTIONS: 'var(--nx-method-options)',
};

const ITEM_HEIGHT = 52;
const VIRTUALIZATION_THRESHOLD = 100;

interface HistoryEntry {
  id: string;
  request: { method: string; url: string };
  response: { status: number };
  timestamp: number;
}

/**
 * Formats a history timestamp as a short local time (hour and minute).
 * @param timestamp - Epoch milliseconds.
 * @returns Locale time string suitable for list rows.
 */
function formatTime(timestamp: number) {
  const d = new Date(timestamp);
  return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

interface HistoryRowCustomProps {
  entries: HistoryEntry[];
  onOpen: (entry: HistoryEntry) => void;
}

/**
 * Virtualized row for a single history entry (method, status, time, URL); invokes `onOpen` when clicked.
 * @param props - `react-window` row props (`index`, `style`) plus `entries` and `onOpen`.
 * @returns The positioned row content.
 */
function HistoryRow(
  props: { index: number; style: CSSProperties } & HistoryRowCustomProps,
): ReactElement {
  const entry = props.entries[props.index]!;
  return (
    <div style={props.style}>
      <button
        className={styles.entry}
        onClick={() => props.onOpen(entry)}
      >
        <div className={styles.entryTop}>
          <span
            className={styles.method}
            style={{ color: METHOD_COLORS[entry.request.method] ?? '#999' }}
          >
            {entry.request.method}
          </span>
          <span className={styles.status}>{entry.response.status}</span>
          <span className={styles.time}>{formatTime(entry.timestamp)}</span>
        </div>
        <div className={styles.url}>{entry.request.url}</div>
      </button>
    </div>
  );
}

/**
 * History sidebar: fetches entries, clear control, and a virtualized list when there are many items.
 */
export function HistoryList() {
  const entries = useHistoryStore((s) => s.entries);
  const fetchHistory = useHistoryStore((s) => s.fetchHistory);
  const clearHistory = useHistoryStore((s) => s.clearHistory);
  const loading = useHistoryStore((s) => s.loading);
  const openTab = useRequestStore((s) => s.openTab);

  useEffect(() => {
    fetchHistory();
  }, []);

  const handleOpen = useMemo(
    () => (entry: HistoryEntry) =>
      openTab({
        name: entry.request.url,
        method: entry.request.method as any,
        url: entry.request.url,
      }),
    [openTab],
  );

  const rowProps = useMemo(
    () => ({ entries: entries as HistoryEntry[], onOpen: handleOpen }),
    [entries, handleOpen],
  );

  const useVirtual = entries.length > VIRTUALIZATION_THRESHOLD;

  return (
    <div className={styles.list}>
      <div className={styles.header}>
        <span className={styles.headerTitle}>History</span>
        <button className={styles.clearBtn} onClick={() => clearHistory()} aria-label="Clear history">
          Clear
        </button>
      </div>
      {loading && <div className={styles.loading}>Loading history...</div>}
      {!loading && entries.length === 0 && (
        <div className={styles.empty}>No request history yet.</div>
      )}
      <div className={styles.entriesContainer}>
        {useVirtual ? (
          <List<HistoryRowCustomProps>
            rowComponent={HistoryRow}
            rowCount={entries.length}
            rowHeight={ITEM_HEIGHT}
            rowProps={rowProps}
            overscanCount={15}
          />
        ) : (
          (entries as HistoryEntry[]).map((entry) => (
            <button
              key={entry.id}
              className={styles.entry}
              onClick={() => handleOpen(entry)}
            >
              <div className={styles.entryTop}>
                <span
                  className={styles.method}
                  style={{ color: METHOD_COLORS[entry.request.method] ?? '#999' }}
                >
                  {entry.request.method}
                </span>
                <span className={styles.status}>{entry.response.status}</span>
                <span className={styles.time}>{formatTime(entry.timestamp)}</span>
              </div>
              <div className={styles.url}>{entry.request.url}</div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
