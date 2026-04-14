import { create } from 'zustand';
import type { NexusEnvironment, Variable } from '@nexus/core';

interface EnvironmentState {
  environments: NexusEnvironment[];
  activeEnvironmentId: string | null;
  globalVariables: Variable[];
  loading: boolean;

  fetchEnvironments: () => Promise<void>;
  addEnvironment: (env: NexusEnvironment) => Promise<void>;
  updateEnvironment: (id: string, updates: Partial<NexusEnvironment>) => Promise<void>;
  deleteEnvironment: (id: string) => Promise<void>;
  setActiveEnvironment: (id: string | null) => void;
  setGlobalVariables: (variables: Variable[]) => void;
  getActiveEnvironment: () => NexusEnvironment | undefined;
}

export const useEnvironmentStore = create<EnvironmentState>((set, get) => ({
  environments: [],
  activeEnvironmentId: null,
  globalVariables: [],
  loading: false,

  fetchEnvironments: async () => {
    set({ loading: true });
    try {
      const environments = await window.nexus.environments.list() as NexusEnvironment[];
      set({ environments, loading: false });
    } catch {
      set({ loading: false });
    }
  },

  addEnvironment: async (env) => {
    try {
      await window.nexus.environments.create(env);
      set((state) => ({ environments: [...state.environments, env] }));
    } catch {
      // ignore
    }
  },

  updateEnvironment: async (id, updates) => {
    const existing = get().environments.find((e) => e.id === id);
    if (!existing) return;
    const updated = { ...existing, ...updates };
    try {
      await window.nexus.environments.update(updated);
      set((state) => ({
        environments: state.environments.map((e) => (e.id === id ? updated : e)),
      }));
    } catch {
      // ignore
    }
  },

  deleteEnvironment: async (id) => {
    try {
      await window.nexus.environments.delete(id);
      set((state) => ({
        environments: state.environments.filter((e) => e.id !== id),
        activeEnvironmentId: state.activeEnvironmentId === id ? null : state.activeEnvironmentId,
      }));
    } catch {
      // ignore
    }
  },

  setActiveEnvironment: (id) => set({ activeEnvironmentId: id }),

  setGlobalVariables: (variables) => set({ globalVariables: variables }),

  getActiveEnvironment: () => {
    const { environments, activeEnvironmentId } = get();
    return environments.find((e) => e.id === activeEnvironmentId);
  },
}));
