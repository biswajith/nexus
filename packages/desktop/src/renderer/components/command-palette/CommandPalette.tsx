import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import type { KeyboardEvent } from 'react';
import { useUiStore } from '../../stores/ui-store.js';
import { useRequestStore } from '../../stores/request-store.js';
import { useCollectionStore } from '../../stores/collection-store.js';
import styles from './CommandPalette.module.css';

interface CommandItem {
  id: string;
  label: string;
  icon: string;
  category: string;
  shortcut?: string;
  action: () => void;
}

interface Props {
  open: boolean;
  onClose: () => void;
}

const isMac = navigator.platform.toUpperCase().includes('MAC');
const mod = isMac ? '⌘' : 'Ctrl+';

export function CommandPalette({ open, onClose }: Props) {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);

  const setActivePanel = useUiStore((s) => s.setActivePanel);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const toggleConsolePanel = useUiStore((s) => s.toggleConsolePanel);
  const setColorScheme = useUiStore((s) => s.setColorScheme);
  const openTab = useRequestStore((s) => s.openTab);
  const activeTabId = useRequestStore((s) => s.activeTabId);
  const sendRequest = useRequestStore((s) => s.sendRequest);
  const collections = useCollectionStore((s) => s.collections);

  const commands = useMemo<CommandItem[]>(() => [
    { id: 'new-request', label: 'New Request', icon: '+', category: 'Request', shortcut: `${mod}N`, action: () => openTab() },
    { id: 'send-request', label: 'Send Request', icon: '▶', category: 'Request', shortcut: `${mod}Enter`, action: () => { if (activeTabId) void sendRequest(activeTabId); } },
    { id: 'panel-http', label: 'Switch to HTTP', icon: '⇄', category: 'Navigation', shortcut: `${mod}1`, action: () => setActivePanel('http') },
    { id: 'panel-graphql', label: 'Switch to GraphQL', icon: '◇', category: 'Navigation', shortcut: `${mod}2`, action: () => setActivePanel('graphql') },
    { id: 'panel-ws', label: 'Switch to WebSocket', icon: '⇌', category: 'Navigation', shortcut: `${mod}3`, action: () => setActivePanel('websocket') },
    { id: 'panel-mcp', label: 'Switch to MCP Tester', icon: '⚡', category: 'Navigation', shortcut: `${mod}4`, action: () => setActivePanel('mcp') },
    { id: 'panel-runner', label: 'Switch to Runner', icon: '▷', category: 'Navigation', shortcut: `${mod}5`, action: () => setActivePanel('runner') },
    { id: 'toggle-sidebar', label: 'Toggle Sidebar', icon: '☰', category: 'View', shortcut: `${mod}B`, action: () => toggleSidebar() },
    { id: 'toggle-console', label: 'Toggle Console', icon: '⌥', category: 'View', shortcut: `${mod}J`, action: () => toggleConsolePanel() },
    { id: 'theme-dark', label: 'Theme: Dark', icon: '◐', category: 'Settings', action: () => setColorScheme('dark') },
    { id: 'theme-light', label: 'Theme: Light', icon: '☀', category: 'Settings', action: () => setColorScheme('light') },
    { id: 'theme-system', label: 'Theme: System', icon: '⚙', category: 'Settings', action: () => setColorScheme('system') },
    ...collections.map((c) => ({
      id: `col-${c.id}`,
      label: c.name,
      icon: '📁',
      category: 'Collection',
      action: () => { setActivePanel('http'); },
    })),
  ], [openTab, activeTabId, sendRequest, setActivePanel, toggleSidebar, toggleConsolePanel, setColorScheme, collections]);

  const filtered = useMemo(() => {
    if (!query.trim()) return commands;
    const q = query.toLowerCase();
    return commands.filter((c) =>
      c.label.toLowerCase().includes(q) || c.category.toLowerCase().includes(q)
    );
  }, [query, commands]);

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const executeItem = useCallback((item: CommandItem) => {
    item.action();
    onClose();
  }, [onClose]);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === 'Enter' && filtered[selectedIndex]) {
      e.preventDefault();
      executeItem(filtered[selectedIndex]);
    } else if (e.key === 'Escape') {
      onClose();
    }
  }, [filtered, selectedIndex, executeItem, onClose]);

  useEffect(() => {
    const active = resultsRef.current?.children[selectedIndex] as HTMLElement | undefined;
    active?.scrollIntoView({ block: 'nearest' });
  }, [selectedIndex]);

  if (!open) return null;

  let lastCategory = '';

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.palette} onClick={(e) => e.stopPropagation()}>
        <div className={styles.searchRow}>
          <span className={styles.searchIcon}>⌘</span>
          <input
            ref={inputRef}
            className={styles.searchInput}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a command..."
          />
        </div>
        <div className={styles.results} ref={resultsRef}>
          {filtered.length > 0 ? (
            filtered.map((item, i) => {
              const showSection = item.category !== lastCategory;
              lastCategory = item.category;
              return (
                <div key={item.id}>
                  {showSection && <div className={styles.sectionLabel}>{item.category}</div>}
                  <div
                    className={`${styles.resultItem} ${i === selectedIndex ? styles.resultItemActive : ''}`}
                    onClick={() => executeItem(item)}
                    onMouseEnter={() => setSelectedIndex(i)}
                  >
                    <span className={styles.resultIcon}>{item.icon}</span>
                    <span className={styles.resultLabel}>{item.label}</span>
                    {item.shortcut && <span className={styles.resultShortcut}>{item.shortcut}</span>}
                  </div>
                </div>
              );
            })
          ) : (
            <div className={styles.empty}>No matching commands</div>
          )}
        </div>
      </div>
    </div>
  );
}
