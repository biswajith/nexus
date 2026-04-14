import { useConsoleStore } from '../../stores/console-store.js';
import styles from './ConsolePanel.module.css';

const LEVEL_STYLES: Record<string, { color: string; label: string }> = {
  info: { color: '#3b82f6', label: 'INFO' },
  warn: { color: '#f59e0b', label: 'WARN' },
  error: { color: '#ef4444', label: 'ERR' },
  debug: { color: '#8b5cf6', label: 'DBG' },
};

const SOURCE_LABELS: Record<string, string> = {
  script: 'Script',
  http: 'HTTP',
  variable: 'Var',
  system: 'Sys',
};

/**
 * Console drawer UI: filterable log stream with level chips, search, and clear.
 */
export function ConsolePanel() {
  const getFilteredEntries = useConsoleStore((s) => s.getFilteredEntries);
  const searchTerm = useConsoleStore((s) => s.searchTerm);
  const setSearchTerm = useConsoleStore((s) => s.setSearchTerm);
  const filterLevel = useConsoleStore((s) => s.filterLevel);
  const setFilterLevel = useConsoleStore((s) => s.setFilterLevel);
  const clear = useConsoleStore((s) => s.clear);
  const entries = getFilteredEntries();

  const formatTime = (ts: number) =>
    new Date(ts).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      fractionalSecondDigits: 3,
    });

  return (
    <div className={styles.console}>
      <div className={styles.toolbar}>
        <div className={styles.filters}>
          {(['info', 'warn', 'error', 'debug'] as const).map((level) => (
            <button
              key={level}
              className={`${styles.filterBtn} ${filterLevel === level ? styles.activeFilter : ''}`}
              onClick={() => setFilterLevel(filterLevel === level ? null : level)}
              style={{ color: LEVEL_STYLES[level]?.color }}
            >
              {LEVEL_STYLES[level]?.label}
            </button>
          ))}
        </div>
        <input
          className={styles.searchInput}
          type="text"
          placeholder="Filter console..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          spellCheck={false}
        />
        <button className={styles.clearBtn} onClick={clear} aria-label="Clear console">
          Clear
        </button>
      </div>
      <div className={styles.entries}>
        {entries.length === 0 && (
          <div className={styles.empty}>Console is empty</div>
        )}
        {entries.map((entry) => {
          const levelStyle = (LEVEL_STYLES[entry.level] ?? LEVEL_STYLES['info'])!;
          return (
            <div key={entry.id} className={styles.entry}>
              <span className={styles.timestamp}>{formatTime(entry.timestamp)}</span>
              <span className={styles.level} style={{ color: levelStyle.color }}>
                {levelStyle.label}
              </span>
              <span className={styles.source}>{SOURCE_LABELS[entry.source] ?? entry.source}</span>
              <span className={styles.message}>{entry.message}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
