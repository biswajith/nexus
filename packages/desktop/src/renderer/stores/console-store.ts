import { create } from 'zustand';
import type { ConsoleEntry, LogLevel, LogSource } from '@nexus/core';

interface ConsoleState {
  entries: ConsoleEntry[];
  filterLevel: LogLevel | null;
  filterSource: LogSource | null;
  searchTerm: string;
  maxEntries: number;

  addEntry: (entry: ConsoleEntry) => void;
  clear: () => void;
  setFilterLevel: (level: LogLevel | null) => void;
  setFilterSource: (source: LogSource | null) => void;
  setSearchTerm: (term: string) => void;
  getFilteredEntries: () => ConsoleEntry[];
}

export const useConsoleStore = create<ConsoleState>((set, get) => ({
  entries: [],
  filterLevel: null,
  filterSource: null,
  searchTerm: '',
  maxEntries: 5000,

  addEntry: (entry) =>
    set((state) => {
      const entries = [...state.entries, entry];
      if (entries.length > state.maxEntries) {
        return { entries: entries.slice(-state.maxEntries) };
      }
      return { entries };
    }),

  clear: () => set({ entries: [] }),

  setFilterLevel: (level) => set({ filterLevel: level }),
  setFilterSource: (source) => set({ filterSource: source }),
  setSearchTerm: (term) => set({ searchTerm: term }),

  getFilteredEntries: () => {
    const { entries, filterLevel, filterSource, searchTerm } = get();
    return entries.filter((e) => {
      if (filterLevel && e.level !== filterLevel) return false;
      if (filterSource && e.source !== filterSource) return false;
      if (searchTerm && !e.message.toLowerCase().includes(searchTerm.toLowerCase())) return false;
      return true;
    });
  },
}));
