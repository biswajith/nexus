import { create } from 'zustand';

type SidebarTab = 'collections' | 'history' | 'search';
type ResponseTab = 'body' | 'headers' | 'cookies' | 'tests' | 'timing' | 'visualize';
type BodyViewMode = 'pretty' | 'raw' | 'preview';
type ColorScheme = 'light' | 'dark' | 'system';
export type ActivePanel = 'http' | 'runner' | 'websocket' | 'mcp' | 'graphql';

interface UiState {
  sidebarTab: SidebarTab;
  sidebarWidth: number;
  sidebarCollapsed: boolean;
  consolePanelHeight: number;
  consolePanelCollapsed: boolean;
  responseTab: ResponseTab;
  bodyViewMode: BodyViewMode;
  colorScheme: ColorScheme;
  activePanel: ActivePanel;

  setSidebarTab: (tab: SidebarTab) => void;
  setSidebarWidth: (width: number) => void;
  toggleSidebar: () => void;
  setConsolePanelHeight: (height: number) => void;
  toggleConsolePanel: () => void;
  setResponseTab: (tab: ResponseTab) => void;
  setBodyViewMode: (mode: BodyViewMode) => void;
  setColorScheme: (scheme: ColorScheme) => void;
  setActivePanel: (panel: ActivePanel) => void;
}

export const useUiStore = create<UiState>((set) => ({
  sidebarTab: 'collections',
  sidebarWidth: 300,
  sidebarCollapsed: false,
  consolePanelHeight: 200,
  consolePanelCollapsed: true,
  responseTab: 'body',
  bodyViewMode: 'pretty',
  colorScheme: 'dark',
  activePanel: 'http',

  setSidebarTab: (tab) => set({ sidebarTab: tab }),
  setSidebarWidth: (width) => set({ sidebarWidth: Math.max(200, Math.min(600, width)) }),
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  setConsolePanelHeight: (height) => set({ consolePanelHeight: Math.max(100, Math.min(500, height)) }),
  toggleConsolePanel: () => set((state) => ({ consolePanelCollapsed: !state.consolePanelCollapsed })),
  setResponseTab: (tab) => set({ responseTab: tab }),
  setBodyViewMode: (mode) => set({ bodyViewMode: mode }),
  setColorScheme: (scheme) => set({ colorScheme: scheme }),
  setActivePanel: (panel) => set({ activePanel: panel }),
}));
