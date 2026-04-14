import { create } from 'zustand';
import type { HistoryEntry, HistoryFilter } from '@nexus/core';

interface HistoryState {
  entries: HistoryEntry[];
  loading: boolean;
  filter: HistoryFilter;

  fetchHistory: (filter?: HistoryFilter) => Promise<void>;
  logEntry: (entry: HistoryEntry) => Promise<void>;
  clearHistory: (before?: Date) => Promise<void>;
  setFilter: (filter: Partial<HistoryFilter>) => void;
}

export const useHistoryStore = create<HistoryState>((set, get) => ({
  entries: [],
  loading: false,
  filter: { limit: 100 },

  fetchHistory: async (filter) => {
    set({ loading: true });
    try {
      const entries = await window.nexus.history.query(filter ?? get().filter);
      set({ entries, loading: false });
    } catch {
      set({ loading: false });
    }
  },

  logEntry: async (entry) => {
    await window.nexus.history.log(entry);
    set((state) => ({
      entries: [entry, ...state.entries].slice(0, get().filter.limit ?? 100),
    }));
  },

  clearHistory: async (before) => {
    await window.nexus.history.clear(before?.toISOString());
    set({ entries: [] });
  },

  setFilter: (filter) =>
    set((state) => ({ filter: { ...state.filter, ...filter } })),
}));
