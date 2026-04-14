import { useEffect, useRef } from 'react';
import { useRequestStore } from '../stores/request-store.js';
import { useUiStore } from '../stores/ui-store.js';
import type { ActivePanel } from '../stores/ui-store.js';

interface ShortcutActions {
  onCommandPalette?: () => void;
  onFocusUrlBar?: () => void;
  onEscape?: () => void;
  /** Focus or open the environment selector; defaults to focusing the toolbar select */
  onFocusEnvironmentSelector?: () => void;
  /** Open settings (placeholder until settings UI exists) */
  onOpenSettings?: () => void;
}

const isMac = navigator.platform.toUpperCase().includes('MAC');

function isModKey(e: KeyboardEvent): boolean {
  return isMac ? e.metaKey : e.ctrlKey;
}

function focusEnvironmentSelectorEl(): void {
  document.querySelector<HTMLElement>('[aria-label="Select environment"]')?.focus();
}

const PANEL_KEYS: Record<string, ActivePanel> = {
  '1': 'http',
  '2': 'graphql',
  '3': 'websocket',
  '4': 'mcp',
  '5': 'runner',
};

export function useKeyboardShortcuts(actions: ShortcutActions = {}) {
  const actionsRef = useRef(actions);
  actionsRef.current = actions;

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const mod = isModKey(e);

      if (e.key === 'Escape') {
        actionsRef.current.onEscape?.();
        return;
      }

      if (!mod) return;

      if (e.key === ',') {
        e.preventDefault();
        actionsRef.current.onOpenSettings?.();
        return;
      }

      const key = e.key.toLowerCase();

      if (key === 'e' && e.shiftKey) {
        e.preventDefault();
        const fn = actionsRef.current.onFocusEnvironmentSelector;
        if (fn) {
          fn();
        } else {
          focusEnvironmentSelectorEl();
        }
        return;
      }

      if (key === 'n' && !e.shiftKey) {
        e.preventDefault();
        useRequestStore.getState().openTab();
        return;
      }

      if (key === 'w') {
        e.preventDefault();
        const { tabs, activeTabId, closeTab } = useRequestStore.getState();
        if (activeTabId && tabs.length > 0) {
          closeTab(activeTabId);
        }
        return;
      }

      if (key === 'enter') {
        e.preventDefault();
        const { activeTabId, sendRequest } = useRequestStore.getState();
        if (activeTabId) {
          void sendRequest(activeTabId);
        }
        return;
      }

      if (key === 'k') {
        e.preventDefault();
        actionsRef.current.onCommandPalette?.();
        return;
      }

      if (key === 'b') {
        e.preventDefault();
        useUiStore.getState().toggleSidebar();
        return;
      }

      if (key === 'j') {
        e.preventDefault();
        useUiStore.getState().toggleConsolePanel();
        return;
      }

      if (key === 'l') {
        e.preventDefault();
        actionsRef.current.onFocusUrlBar?.();
        return;
      }

      if (!e.shiftKey && PANEL_KEYS[key]) {
        e.preventDefault();
        useUiStore.getState().setActivePanel(PANEL_KEYS[key]!);
        return;
      }
    }

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);
}

export function formatShortcut(key: string, shift = false): string {
  const mod = isMac ? '⌘' : 'Ctrl';
  const parts = [mod];
  if (shift) parts.push('Shift');
  parts.push(key.toUpperCase());
  return parts.join(isMac ? '' : '+');
}
