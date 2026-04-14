import { useState } from 'react';
import { useUiStore, type ActivePanel } from '../../stores/ui-store.js';
import { useEnvironmentStore } from '../../stores/environment-store.js';
import { ImportExportModal } from '../import-export/ImportExportModal.js';
import { DocsModal } from '../docs/DocsModal.js';
import styles from './Toolbar.module.css';

const panels: { key: ActivePanel; label: string }[] = [
  { key: 'http', label: 'HTTP' },
  { key: 'graphql', label: 'GraphQL' },
  { key: 'websocket', label: 'WS' },
  { key: 'mcp', label: 'MCP' },
  { key: 'runner', label: 'Runner' },
];

const THEME_CYCLE: Array<'dark' | 'light' | 'system'> = ['dark', 'light', 'system'];
const THEME_LABELS: Record<string, string> = { dark: 'Dark', light: 'Light', system: 'System' };

export function Toolbar() {
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const toggleConsolePanel = useUiStore((s) => s.toggleConsolePanel);
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const consolePanelCollapsed = useUiStore((s) => s.consolePanelCollapsed);
  const activePanel = useUiStore((s) => s.activePanel);
  const setActivePanel = useUiStore((s) => s.setActivePanel);
  const colorScheme = useUiStore((s) => s.colorScheme);
  const setColorScheme = useUiStore((s) => s.setColorScheme);
  const environments = useEnvironmentStore((s) => s.environments);
  const activeEnvironmentId = useEnvironmentStore((s) => s.activeEnvironmentId);
  const setActiveEnvironment = useEnvironmentStore((s) => s.setActiveEnvironment);
  const [importExportOpen, setImportExportOpen] = useState(false);
  const [docsOpen, setDocsOpen] = useState(false);

  const cycleTheme = () => {
    const idx = THEME_CYCLE.indexOf(colorScheme);
    setColorScheme(THEME_CYCLE[(idx + 1) % THEME_CYCLE.length]!);
  };

  return (
    <>
      <div className={styles.toolbar}>
        <div className={styles.left}>
          <button
            className={styles.iconBtn}
            onClick={toggleSidebar}
            aria-label="Toggle sidebar"
            title="Toggle sidebar"
          >
            {sidebarCollapsed ? '☰' : '✕'}
          </button>
          <div className={styles.panelTabs}>
            {panels.map((p) => (
              <button
                key={p.key}
                className={`${styles.panelBtn} ${activePanel === p.key ? styles.panelBtnActive : ''}`}
                onClick={() => setActivePanel(p.key)}
              >
                {p.label}
              </button>
            ))}
          </div>
          <div className={styles.divider} />
          <button className={styles.textBtn} onClick={() => setImportExportOpen(true)}>
            Import
          </button>
          <button className={styles.textBtn} onClick={() => setDocsOpen(true)}>
            Docs
          </button>
        </div>
        <div className={styles.center}>
          <select
            className={styles.envSelect}
            value={activeEnvironmentId ?? ''}
            onChange={(e) => setActiveEnvironment(e.target.value || null)}
            aria-label="Select environment"
          >
            <option value="">No Environment</option>
            {environments.map((env) => (
              <option key={env.id} value={env.id}>
                {env.name}
              </option>
            ))}
          </select>
        </div>
        <div className={styles.right}>
          <button className={styles.textBtn} onClick={cycleTheme} title={`Theme: ${colorScheme}`}>
            {THEME_LABELS[colorScheme]}
          </button>
          <button className={styles.textBtn} onClick={toggleConsolePanel}>
            {consolePanelCollapsed ? 'Console' : '✕ Console'}
          </button>
        </div>
      </div>
      <ImportExportModal open={importExportOpen} onClose={() => setImportExportOpen(false)} />
      <DocsModal open={docsOpen} onClose={() => setDocsOpen(false)} />
    </>
  );
}
