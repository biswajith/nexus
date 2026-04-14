import { useUiStore } from '../../stores/ui-store.js';
import { CollectionTree } from './CollectionTree.js';
import { HistoryList } from './HistoryList.js';
import { EnvironmentManager } from './EnvironmentManager.js';
import styles from './Sidebar.module.css';

export function Sidebar() {
  const sidebarTab = useUiStore((s) => s.sidebarTab);
  const setSidebarTab = useUiStore((s) => s.setSidebarTab);

  return (
    <div className={styles.sidebar}>
      <div className={styles.tabRow}>
        {(['collections', 'environments', 'history'] as const).map((tab) => (
          <button
            key={tab}
            className={`${styles.tabBtn} ${sidebarTab === tab ? styles.activeTab : ''}`}
            onClick={() => setSidebarTab(tab)}
          >
            {tab === 'environments' ? 'Envs' : tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>
      <div className={styles.content}>
        {sidebarTab === 'collections' && <CollectionTree />}
        {sidebarTab === 'environments' && <EnvironmentManager />}
        {sidebarTab === 'history' && <HistoryList />}
      </div>
    </div>
  );
}
