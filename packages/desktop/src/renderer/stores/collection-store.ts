import { create } from 'zustand';
import type { NexusCollection, NexusRequest } from '@nexus/core';

interface CollectionMeta {
  id: string;
  name: string;
  description?: string;
  path: string;
}

interface CollectionState {
  collections: CollectionMeta[];
  loadedCollections: Map<string, NexusCollection>;
  selectedItemId: string | null;
  expandedFolders: Set<string>;
  loading: boolean;
  error: string | null;

  fetchCollections: () => Promise<void>;
  loadCollection: (dirName: string) => Promise<void>;
  createCollection: (collection: NexusCollection) => Promise<void>;
  renameCollection: (dirName: string, newName: string) => Promise<void>;
  deleteCollection: (dirName: string) => Promise<void>;
  addRequest: (dirName: string, request: NexusRequest) => Promise<void>;
  updateRequestInCollection: (dirName: string, requestId: string, request: NexusRequest) => Promise<void>;
  deleteRequest: (dirName: string, requestId: string) => Promise<void>;
  selectItem: (id: string | null) => void;
  toggleFolder: (id: string) => void;
}

function dirNameFromPath(p: string): string {
  return p.split('/').pop() ?? p;
}

export const useCollectionStore = create<CollectionState>((set, get) => ({
  collections: [],
  loadedCollections: new Map(),
  selectedItemId: null,
  expandedFolders: new Set(),
  loading: false,
  error: null,

  fetchCollections: async () => {
    set({ loading: true, error: null });
    try {
      const collections = await window.nexus.collections.list();
      set({ collections, loading: false });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to load collections', loading: false });
    }
  },

  loadCollection: async (dirName) => {
    try {
      const collection = await window.nexus.collections.load(dirName);
      set((state) => {
        const loaded = new Map(state.loadedCollections);
        loaded.set(dirName, collection);
        return { loadedCollections: loaded };
      });
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to load collection' });
    }
  },

  createCollection: async (collection) => {
    try {
      await window.nexus.collections.create(collection);
      await get().fetchCollections();
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to create collection' });
    }
  },

  renameCollection: async (dirName, newName) => {
    try {
      await window.nexus.collections.rename(dirName, newName);
      set((state) => ({
        collections: state.collections.map((c) =>
          dirNameFromPath(c.path) === dirName ? { ...c, name: newName } : c,
        ),
      }));
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to rename collection' });
    }
  },

  deleteCollection: async (dirName) => {
    try {
      await window.nexus.collections.delete(dirName);
      set((state) => {
        const loaded = new Map(state.loadedCollections);
        loaded.delete(dirName);
        return { loadedCollections: loaded };
      });
      await get().fetchCollections();
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to delete collection' });
    }
  },

  addRequest: async (dirName, request) => {
    try {
      await window.nexus.collections.addRequest(dirName, request);
      await get().loadCollection(dirName);
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to add request' });
    }
  },

  updateRequestInCollection: async (dirName, requestId, request) => {
    try {
      await window.nexus.collections.updateRequest(dirName, requestId, request);
      await get().loadCollection(dirName);
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to update request' });
    }
  },

  deleteRequest: async (dirName, requestId) => {
    try {
      await window.nexus.collections.deleteRequest(dirName, requestId);
      await get().loadCollection(dirName);
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Failed to delete request' });
    }
  },

  selectItem: (id) => set({ selectedItemId: id }),

  toggleFolder: (id) =>
    set((state) => {
      const expanded = new Set(state.expandedFolders);
      if (expanded.has(id)) {
        expanded.delete(id);
      } else {
        expanded.add(id);
      }
      return { expandedFolders: expanded };
    }),
}));
