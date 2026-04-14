import { create } from 'zustand';

interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
  duration: number;
}

interface RequestRunResult {
  requestId: string;
  requestName: string;
  method: string;
  url: string;
  status: number;
  statusText: string;
  responseTime: number;
  size: number;
  testResults: TestResult[];
  error?: string;
}

interface RunSummary {
  totalRequests: number;
  totalIterations: number;
  totalTests: number;
  passedTests: number;
  failedTests: number;
  totalTime: number;
  results: RequestRunResult[];
  aborted: boolean;
}

type RunStatus = 'idle' | 'running' | 'completed' | 'cancelled' | 'error';

interface RunnerState {
  status: RunStatus;
  currentIteration: number;
  totalIterations: number;
  results: RequestRunResult[];
  summary: RunSummary | null;
  error: string | null;

  startRun: (config: unknown) => Promise<void>;
  cancelRun: () => Promise<void>;
  reset: () => void;
}

/** Collection runner status, iteration progress, per-request results, and run summary/error. */
export const useRunnerStore = create<RunnerState>((set) => ({
  status: 'idle',
  currentIteration: 0,
  totalIterations: 0,
  results: [],
  summary: null,
  error: null,

  startRun: async (config) => {
    set({ status: 'running', results: [], summary: null, error: null, currentIteration: 0 });

    const cleanup = window.nexus.runner.onEvent((event: unknown) => {
      const evt = event as { type: string; data: unknown };
      switch (evt.type) {
        case 'iteration-start': {
          const d = evt.data as { iteration: number; total: number };
          set({ currentIteration: d.iteration + 1, totalIterations: d.total });
          break;
        }
        case 'request-complete': {
          const result = evt.data as RequestRunResult;
          set((state) => ({ results: [...state.results, result] }));
          break;
        }
        case 'error': {
          const d = evt.data as { message: string };
          set({ error: d.message });
          break;
        }
      }
    });

    try {
      const summary = await window.nexus.runner.start(config) as RunSummary;
      set({ status: summary.aborted ? 'cancelled' : 'completed', summary });
    } catch (err) {
      set({ status: 'error', error: err instanceof Error ? err.message : String(err) });
    } finally {
      cleanup();
    }
  },

  cancelRun: async () => {
    await window.nexus.runner.cancel();
    set({ status: 'cancelled' });
  },

  reset: () => set({
    status: 'idle', currentIteration: 0, totalIterations: 0,
    results: [], summary: null, error: null,
  }),
}));
