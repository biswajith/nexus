import { useRequestStore } from '../../stores/request-store.js';
import styles from './TabBar.module.css';

const METHOD_COLORS: Record<string, string> = {
  GET: 'var(--nx-method-get)',
  POST: 'var(--nx-method-post)',
  PUT: 'var(--nx-method-put)',
  PATCH: 'var(--nx-method-patch)',
  DELETE: 'var(--nx-method-delete)',
  HEAD: 'var(--nx-method-head)',
  OPTIONS: 'var(--nx-method-options)',
};

/** Horizontal strip of request tabs with close/new controls, driven by the request store. */
export function TabBar() {
  const tabs = useRequestStore((s) => s.tabs);
  const activeTabId = useRequestStore((s) => s.activeTabId);
  const setActiveTab = useRequestStore((s) => s.setActiveTab);
  const closeTab = useRequestStore((s) => s.closeTab);
  const openTab = useRequestStore((s) => s.openTab);

  return (
    <div className={styles.tabBar}>
      <div className={styles.tabs}>
        {tabs.map((tab) => {
          const label = tab.request.name !== 'New Request' ? tab.request.name : (tab.request.url || tab.request.name);
          return (
            <div
              key={tab.id}
              role="tab"
              tabIndex={0}
              className={`${styles.tab} ${tab.id === activeTabId ? styles.active : ''}`}
              onClick={() => setActiveTab(tab.id)}
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') setActiveTab(tab.id); }}
              title={label}
            >
              <span
                className={styles.method}
                style={{ color: METHOD_COLORS[tab.request.method] ?? '#999' }}
              >
                {tab.request.method}
              </span>
              <span className={styles.name}>{label}</span>
              {tab.dirty && <span className={styles.dirty}>●</span>}
              <button
                className={styles.closeBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(tab.id);
                }}
                aria-label={`Close ${tab.request.name}`}
              >
                ×
              </button>
            </div>
          );
        })}
      </div>
      <button className={styles.addBtn} onClick={() => openTab()} aria-label="New request">
        +
      </button>
    </div>
  );
}
