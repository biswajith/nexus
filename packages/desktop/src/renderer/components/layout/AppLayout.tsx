import { useEffect, useState, useCallback } from 'react';
import { useRequestStore } from '../../stores/request-store.js';
import { useUiStore } from '../../stores/ui-store.js';
import { useEnvironmentStore } from '../../stores/environment-store.js';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts.js';
import { Sidebar } from '../sidebar/Sidebar.js';
import { RequestPanel } from '../request/RequestPanel.js';
import { ConsolePanel } from '../console/ConsolePanel.js';
import { RunnerPanel } from '../runner/RunnerPanel.js';
import { WebSocketPanel } from '../websocket/WebSocketPanel.js';
import { McpPanel } from '../mcp/McpPanel.js';
import { GraphQLPanel } from '../graphql/GraphQLPanel.js';
import { Toolbar } from './Toolbar.js';
import { CommandPalette } from '../command-palette/CommandPalette.js';
import styles from './AppLayout.module.css';

export function AppLayout() {
  const tabs = useRequestStore((s) => s.tabs);
  const openTab = useRequestStore((s) => s.openTab);
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const consolePanelCollapsed = useUiStore((s) => s.consolePanelCollapsed);
  const sidebarWidth = useUiStore((s) => s.sidebarWidth);
  const consolePanelHeight = useUiStore((s) => s.consolePanelHeight);
  const activePanel = useUiStore((s) => s.activePanel);
  const [paletteOpen, setPaletteOpen] = useState(false);

  const handleOpenPalette = useCallback(() => setPaletteOpen(true), []);
  const handleClosePalette = useCallback(() => setPaletteOpen(false), []);

  const handleFocusUrlBar = useCallback(() => {
    document.querySelector<HTMLInputElement>('[aria-label="Request URL"]')?.focus();
  }, []);

  const handleEscape = useCallback(() => {
    if (paletteOpen) {
      setPaletteOpen(false);
    }
  }, [paletteOpen]);

  useKeyboardShortcuts({
    onCommandPalette: handleOpenPalette,
    onFocusUrlBar: handleFocusUrlBar,
    onEscape: handleEscape,
  });

  const fetchEnvironments = useEnvironmentStore((s) => s.fetchEnvironments);

  useEffect(() => {
    if (tabs.length === 0) {
      openTab();
    }
    fetchEnvironments();
  }, []);

  const renderPanel = () => {
    switch (activePanel) {
      case 'runner': return <RunnerPanel />;
      case 'websocket': return <WebSocketPanel />;
      case 'mcp': return <McpPanel />;
      case 'graphql': return <GraphQLPanel />;
      default: return <RequestPanel />;
    }
  };

  return (
    <div className={styles.root}>
      <div className={styles.titleBar}>
        <span className={styles.titleText}>Nexus</span>
      </div>
      <div className={styles.workspace}>
        {!sidebarCollapsed && (
          <div className={styles.sidebar} style={{ width: sidebarWidth }}>
            <Sidebar />
          </div>
        )}
        <div className={styles.mainArea}>
          <Toolbar />
          <div className={styles.editorArea}>
            {renderPanel()}
          </div>
          {!consolePanelCollapsed && (
            <div className={styles.consoleArea} style={{ height: consolePanelHeight }}>
              <ConsolePanel />
            </div>
          )}
        </div>
      </div>
      <CommandPalette open={paletteOpen} onClose={handleClosePalette} />
    </div>
  );
}
