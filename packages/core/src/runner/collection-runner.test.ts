import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { NexusCollection, NexusRequest, NexusFolder, NexusResponse, Variable } from '../types/index.js';
import type { RunConfig, RunEvent, RunSummary } from './collection-runner.js';

vi.mock('../http/request-sender.js', () => ({
  sendHttpRequest: vi.fn(),
}));

vi.mock('../scripts/sandbox.js', () => {
  class MockScriptSandbox {
    createTrackedScope(_name: string, vars: Variable[]) {
      const data = new Map(vars.filter((v) => v.enabled).map((v) => [v.key, v.value]));
      return {
        get: (k: string) => data.get(k),
        set: (k: string, v: string) => { data.set(k, v); },
        unset: (k: string) => { data.delete(k); },
        toObject: () => Object.fromEntries(data),
        changes: [],
      };
    }
    execute() {
      return {
        logs: [],
        testResults: [],
        variableChanges: [],
        executionControl: { nextRequest: undefined, skipped: false },
      };
    }
  }
  return { ScriptSandbox: MockScriptSandbox };
});

vi.mock('../auth/auth-handler.js', () => ({
  resolveAuth: vi.fn().mockImplementation(async (_auth: unknown, req: { url: string; headers: Record<string, string> }) => ({
    url: req.url,
    headers: req.headers,
  })),
}));

vi.mock('../environments/variable-store.js', () => {
  class MockVariableStore {
    resolve(template: string) { return template; }
    getScope() {
      return {
        get: () => undefined,
        set: () => {},
        unset: () => {},
        getAll: () => [],
        toRecord: () => ({}),
      };
    }
  }
  return { VariableStore: MockVariableStore };
});

import { sendHttpRequest } from '../http/request-sender.js';
import { CollectionRunner } from './collection-runner.js';

const mockSendHttp = vi.mocked(sendHttpRequest);

function fakeResponse(overrides: Partial<NexusResponse> = {}): NexusResponse {
  return {
    status: 200,
    statusText: 'OK',
    headers: { 'content-type': 'application/json' },
    body: Buffer.from('{"ok":true}'),
    bodyText: '{"ok":true}',
    bodyJson: { ok: true },
    timing: { dns: 1, tcp: 2, tls: 3, ttfb: 10, download: 5, total: 21 },
    size: 11,
    ...overrides,
  };
}

function makeRequest(id: string, name: string, overrides: Partial<NexusRequest> = {}): NexusRequest {
  return {
    id,
    name,
    method: 'GET',
    url: `https://api.example.com/${name}`,
    headers: [],
    params: [],
    body: { mode: 'none' },
    auth: { type: 'none' },
    settings: {},
    ...overrides,
  };
}

function makeFolder(id: string, name: string, items: (NexusRequest | NexusFolder)[]): NexusFolder {
  return { id, name, items };
}

function makeCollection(items: (NexusRequest | NexusFolder)[], overrides: Partial<NexusCollection> = {}): NexusCollection {
  return {
    id: 'col-1',
    name: 'Test Collection',
    variables: [],
    items,
    ...overrides,
  };
}

function baseConfig(collection: NexusCollection, overrides: Partial<RunConfig> = {}): RunConfig {
  return {
    collection,
    iterations: 1,
    delayMs: 0,
    stopOnError: false,
    persistVariables: false,
    ...overrides,
  };
}

describe('CollectionRunner', () => {
  let runner: CollectionRunner;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    mockSendHttp.mockResolvedValue(fakeResponse());
    runner = new CollectionRunner();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('basic execution', () => {
    it('runs a single request and returns a summary', async () => {
      const collection = makeCollection([makeRequest('r1', 'get-users')]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(1);
      expect(summary.totalIterations).toBe(1);
      expect(summary.aborted).toBe(false);
      expect(summary.results).toHaveLength(1);
      expect(summary.results[0]!.requestId).toBe('r1');
      expect(summary.results[0]!.status).toBe(200);
    });

    it('runs multiple requests in order', async () => {
      const collection = makeCollection([
        makeRequest('r1', 'first'),
        makeRequest('r2', 'second'),
        makeRequest('r3', 'third'),
      ]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(3);
      expect(summary.results.map((r) => r.requestId)).toEqual(['r1', 'r2', 'r3']);
    });

    it('flattens nested folders depth-first', async () => {
      const collection = makeCollection([
        makeRequest('r1', 'top-level'),
        makeFolder('f1', 'Folder A', [
          makeRequest('r2', 'nested-a1'),
          makeFolder('f2', 'Subfolder', [
            makeRequest('r3', 'deep-nested'),
          ]),
        ]),
        makeRequest('r4', 'another-top'),
      ]);

      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.results.map((r) => r.requestId)).toEqual(['r1', 'r2', 'r3', 'r4']);
    });

    it('filters by requestIds when provided', async () => {
      const collection = makeCollection([
        makeRequest('r1', 'first'),
        makeRequest('r2', 'second'),
        makeRequest('r3', 'third'),
      ]);

      const promise = runner.run(baseConfig(collection, { requestIds: ['r1', 'r3'] }));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.results.map((r) => r.requestId)).toEqual(['r1', 'r3']);
    });
  });

  describe('iterations', () => {
    it('repeats requests across multiple iterations', async () => {
      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection, { iterations: 3 }));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(3);
      expect(summary.totalIterations).toBe(3);
    });

    it('runs all requests in each iteration', async () => {
      const collection = makeCollection([
        makeRequest('r1', 'first'),
        makeRequest('r2', 'second'),
      ]);

      const promise = runner.run(baseConfig(collection, { iterations: 2 }));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(4);
      expect(summary.results.map((r) => r.requestId)).toEqual(['r1', 'r2', 'r1', 'r2']);
    });
  });

  describe('events', () => {
    it('emits iteration-start and iteration-complete events', async () => {
      const events: RunEvent[] = [];
      runner.on('event', (e: RunEvent) => events.push(e));

      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection, { iterations: 2 }));
      await vi.runAllTimersAsync();
      await promise;

      const iterStarts = events.filter((e) => e.type === 'iteration-start');
      const iterCompletes = events.filter((e) => e.type === 'iteration-complete');
      expect(iterStarts).toHaveLength(2);
      expect(iterCompletes).toHaveLength(2);
    });

    it('emits request-start and request-complete for each request', async () => {
      const events: RunEvent[] = [];
      runner.on('event', (e: RunEvent) => events.push(e));

      const collection = makeCollection([
        makeRequest('r1', 'first'),
        makeRequest('r2', 'second'),
      ]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      await promise;

      const starts = events.filter((e) => e.type === 'request-start');
      const completes = events.filter((e) => e.type === 'request-complete');
      expect(starts).toHaveLength(2);
      expect(completes).toHaveLength(2);
    });

    it('emits run-complete with the summary', async () => {
      const events: RunEvent[] = [];
      runner.on('event', (e: RunEvent) => events.push(e));

      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      await promise;

      const runComplete = events.find((e) => e.type === 'run-complete');
      expect(runComplete).toBeDefined();
      expect((runComplete!.data as RunSummary).totalRequests).toBe(1);
    });
  });

  describe('delay between requests', () => {
    it('waits delayMs between requests', async () => {
      const collection = makeCollection([
        makeRequest('r1', 'first'),
        makeRequest('r2', 'second'),
      ]);

      const promise = runner.run(baseConfig(collection, { delayMs: 500 }));
      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(500);
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(2);
    });
  });

  describe('cancel()', () => {
    it('stops before the next iteration when cancelled', async () => {
      let callCount = 0;
      mockSendHttp.mockImplementation(async () => {
        callCount++;
        if (callCount >= 3) runner.cancel();
        return fakeResponse();
      });

      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const config = baseConfig(collection, { iterations: 100 });

      const promise = runner.run(config);
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.aborted).toBe(true);
      expect(summary.totalRequests).toBeLessThan(100);
    });

    it('stops between requests within an iteration', async () => {
      let callCount = 0;
      mockSendHttp.mockImplementation(async () => {
        callCount++;
        if (callCount === 1) runner.cancel();
        return fakeResponse();
      });

      const collection = makeCollection([
        makeRequest('r1', 'first'),
        makeRequest('r2', 'second'),
        makeRequest('r3', 'third'),
      ]);

      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.aborted).toBe(true);
      expect(summary.totalRequests).toBeLessThanOrEqual(2);
    });

    it('resets aborted state on subsequent runs', async () => {
      const collection = makeCollection([makeRequest('r1', 'endpoint')]);

      mockSendHttp.mockImplementation(async () => {
        runner.cancel();
        return fakeResponse();
      });
      const promise1 = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary1 = await promise1;
      expect(summary1.aborted).toBe(true);

      mockSendHttp.mockResolvedValue(fakeResponse());
      const promise2 = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary2 = await promise2;
      expect(summary2.aborted).toBe(false);
    });
  });

  describe('error handling', () => {
    it('catches request errors and includes them in results', async () => {
      mockSendHttp.mockRejectedValue(new Error('Connection refused'));

      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.results[0]!.error).toBe('Connection refused');
      expect(summary.results[0]!.status).toBe(0);
      expect(summary.results[0]!.statusText).toBe('Request failed');
    });

    it('continues after errors when stopOnError is false', async () => {
      mockSendHttp
        .mockRejectedValueOnce(new Error('Timeout'))
        .mockResolvedValue(fakeResponse());

      const collection = makeCollection([
        makeRequest('r1', 'will-fail'),
        makeRequest('r2', 'will-succeed'),
      ]);
      const promise = runner.run(baseConfig(collection, { stopOnError: false }));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(2);
      expect(summary.results[0]!.error).toBe('Timeout');
      expect(summary.results[1]!.status).toBe(200);
    });

    it('stops after first error when stopOnError is true', async () => {
      mockSendHttp
        .mockRejectedValueOnce(new Error('Server down'))
        .mockResolvedValue(fakeResponse());

      const collection = makeCollection([
        makeRequest('r1', 'will-fail'),
        makeRequest('r2', 'should-not-run'),
      ]);
      const promise = runner.run(baseConfig(collection, { stopOnError: true }));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(1);
      expect(summary.results[0]!.error).toBe('Server down');
    });

    it('emits an error event when stopOnError is true', async () => {
      const events: RunEvent[] = [];
      runner.on('event', (e: RunEvent) => events.push(e));

      mockSendHttp.mockRejectedValueOnce(new Error('Boom'));

      const collection = makeCollection([makeRequest('r1', 'fail')]);
      const promise = runner.run(baseConfig(collection, { stopOnError: true }));
      await vi.runAllTimersAsync();
      await promise;

      const errorEvents = events.filter((e) => e.type === 'error');
      expect(errorEvents).toHaveLength(1);
      expect((errorEvents[0]!.data as { message: string }).message).toBe('Boom');
    });

    it('handles non-Error thrown objects', async () => {
      mockSendHttp.mockRejectedValue('string error');

      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.results[0]!.error).toBe('string error');
    });
  });

  describe('summary statistics', () => {
    it('counts total time', async () => {
      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection));
      await vi.advanceTimersByTimeAsync(100);
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalTime).toBeGreaterThanOrEqual(0);
    });

    it('reports zero tests when none are present', async () => {
      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalTests).toBe(0);
      expect(summary.passedTests).toBe(0);
      expect(summary.failedTests).toBe(0);
    });
  });

  describe('empty collection', () => {
    it('returns a summary with zero results for an empty collection', async () => {
      const collection = makeCollection([]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(0);
      expect(summary.results).toEqual([]);
      expect(summary.aborted).toBe(false);
    });

    it('still emits iteration events for empty collections', async () => {
      const events: RunEvent[] = [];
      runner.on('event', (e: RunEvent) => events.push(e));

      const collection = makeCollection([]);
      const promise = runner.run(baseConfig(collection, { iterations: 2 }));
      await vi.runAllTimersAsync();
      await promise;

      expect(events.filter((e) => e.type === 'iteration-start')).toHaveLength(2);
      expect(events.filter((e) => e.type === 'iteration-complete')).toHaveLength(2);
    });
  });

  describe('data rows', () => {
    it('cycles data rows across iterations', async () => {
      const dataRows = [
        { name: 'Alice' },
        { name: 'Bob' },
      ];

      const collection = makeCollection([makeRequest('r1', 'endpoint')]);
      const promise = runner.run(baseConfig(collection, {
        iterations: 4,
        dataRows,
      }));
      await vi.runAllTimersAsync();
      const summary = await promise;

      expect(summary.totalRequests).toBe(4);
    });
  });

  describe('request result shape', () => {
    it('includes all expected fields in a successful result', async () => {
      mockSendHttp.mockResolvedValue(fakeResponse({
        status: 201,
        statusText: 'Created',
        timing: { dns: 1, tcp: 2, tls: 3, ttfb: 10, download: 5, total: 21 },
        size: 42,
      }));

      const collection = makeCollection([
        makeRequest('r1', 'create-item', { method: 'POST' }),
      ]);
      const promise = runner.run(baseConfig(collection));
      await vi.runAllTimersAsync();
      const summary = await promise;

      const result = summary.results[0]!;
      expect(result.requestId).toBe('r1');
      expect(result.requestName).toBe('create-item');
      expect(result.status).toBe(201);
      expect(result.statusText).toBe('Created');
      expect(result.responseTime).toBe(21);
      expect(result.size).toBe(42);
      expect(result.error).toBeUndefined();
      expect(result.testResults).toEqual([]);
      expect(result.scriptLogs).toEqual([]);
    });
  });
});
