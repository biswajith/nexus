import { EventEmitter } from 'node:events';
import type {
  NexusCollection, NexusRequest, NexusFolder,
  Variable, HttpMethod,
} from '../types/index.js';
import { isFolder, isRequest } from '../types/index.js';
import { sendHttpRequest } from '../http/request-sender.js';
import type { SendOptions } from '../http/request-sender.js';
import { VariableStore } from '../environments/variable-store.js';
import { ScriptSandbox } from '../scripts/sandbox.js';
import { resolveAuth } from '../auth/auth-handler.js';
import type { TestResult } from '../scripts/assertions.js';

export interface RunConfig {
  collection: NexusCollection;
  environmentVars?: Variable[];
  globalVars?: Variable[];
  requestIds?: string[];
  iterations: number;
  delayMs: number;
  dataRows?: Record<string, string>[];
  stopOnError: boolean;
  persistVariables: boolean;
}

export interface RunEvent {
  type: 'iteration-start' | 'iteration-complete' | 'request-start' | 'request-complete'
    | 'test-result' | 'run-complete' | 'error' | 'console-log';
  data: unknown;
}

export interface RequestRunResult {
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
  scriptLogs: Array<{ level: string; args: unknown[]; timestamp: number }>;
}

export interface RunSummary {
  totalRequests: number;
  totalIterations: number;
  totalTests: number;
  passedTests: number;
  failedTests: number;
  totalTime: number;
  results: RequestRunResult[];
  aborted: boolean;
}

/**
 * Executes a Nexus collection's HTTP requests in order with variable resolution, scripts, auth, and lifecycle events via `EventEmitter`.
 */
export class CollectionRunner extends EventEmitter {
  private sandbox: ScriptSandbox;
  private aborted = false;

  /**
   * Creates a runner with an isolated script sandbox for pre/post-request code.
   * @param scriptTimeout - Maximum time in milliseconds a single script may run.
   */
  constructor(scriptTimeout = 30_000) {
    super();
    this.sandbox = new ScriptSandbox(scriptTimeout);
  }

  /**
   * Runs the collection for the configured iterations and options, emitting progress events and returning an aggregate summary.
   * @param config - Collection, variables, filters, timing, and run behavior (e.g. stop on error).
   * @returns Summary counts, timing, per-request results, and whether the run was aborted.
   */
  async run(config: RunConfig): Promise<RunSummary> {
    this.aborted = false;
    const startTime = Date.now();
    const allResults: RequestRunResult[] = [];
    const dataRows = config.dataRows ?? [{}];

    const collVars = [...(config.collection.variables ?? [])];
    const envVars = [...(config.environmentVars ?? [])];
    const globalVars = [...(config.globalVars ?? [])];

    for (let iteration = 0; iteration < config.iterations; iteration++) {
      if (this.aborted) break;

      this.emit('event', { type: 'iteration-start', data: { iteration, total: config.iterations } } satisfies RunEvent);

      const rowData = dataRows[iteration % dataRows.length] ?? {};
      const dataVars: Variable[] = Object.entries(rowData).map(([key, value]) => ({
        key, value, type: 'string' as const, enabled: true,
      }));

      const requestQueue = this.buildRequestQueue(config.collection, config.requestIds);
      let currentIndex = 0;
      let nextRequestName: string | null | undefined;

      while (currentIndex < requestQueue.length) {
        if (this.aborted) break;

        const req = requestQueue[currentIndex]!;
        this.emit('event', { type: 'request-start', data: { requestId: req.id, name: req.name, iteration } } satisfies RunEvent);

        const result = await this.executeRequest(
          req, config.collection, envVars, collVars, globalVars, dataVars,
        );
        allResults.push(result);

        this.emit('event', { type: 'request-complete', data: result } satisfies RunEvent);

        for (const tr of result.testResults) {
          this.emit('event', { type: 'test-result', data: tr } satisfies RunEvent);
        }

        if (result.error && config.stopOnError) {
          this.emit('event', { type: 'error', data: { message: result.error, requestId: req.id } } satisfies RunEvent);
          break;
        }

        const hasFailedTests = result.testResults.some((t) => !t.passed);
        if (hasFailedTests && config.stopOnError) break;

        if (nextRequestName !== undefined) {
          if (nextRequestName === null) break;
          const idx = requestQueue.findIndex((r) => r.name === nextRequestName);
          currentIndex = idx >= 0 ? idx : requestQueue.length;
          nextRequestName = undefined;
        } else {
          currentIndex++;
        }

        if (config.delayMs > 0 && currentIndex < requestQueue.length) {
          await this.delay(config.delayMs);
        }
      }

      this.emit('event', { type: 'iteration-complete', data: { iteration } } satisfies RunEvent);
    }

    const summary: RunSummary = {
      totalRequests: allResults.length,
      totalIterations: config.iterations,
      totalTests: allResults.reduce((sum, r) => sum + r.testResults.length, 0),
      passedTests: allResults.reduce((sum, r) => sum + r.testResults.filter((t) => t.passed).length, 0),
      failedTests: allResults.reduce((sum, r) => sum + r.testResults.filter((t) => !t.passed).length, 0),
      totalTime: Date.now() - startTime,
      results: allResults,
      aborted: this.aborted,
    };

    this.emit('event', { type: 'run-complete', data: summary } satisfies RunEvent);
    return summary;
  }

  /**
   * Signals the current run to stop cooperatively at the next iteration or request boundary.
   * @returns void
   */
  cancel(): void {
    this.aborted = true;
  }

  /**
   * Resolves variables, runs pre/post scripts, applies auth, sends the HTTP request, and collects tests and logs.
   * @param req - Request to execute.
   * @param collection - Parent collection for inherited auth and shared scripts.
   * @param envVars - Environment variables for resolution and sandbox scopes.
   * @param collVars - Collection variables for resolution and sandbox scopes.
   * @param globalVars - Global variables for resolution and sandbox scopes.
   * @param dataVars - Data-row variables merged into the environment for this iteration.
   * @returns Per-request result including status, timing, tests, script logs, and optional error.
   */
  private async executeRequest(
    req: NexusRequest,
    collection: NexusCollection,
    envVars: Variable[],
    collVars: Variable[],
    globalVars: Variable[],
    dataVars: Variable[],
  ): Promise<RequestRunResult> {
    const scriptLogs: Array<{ level: string; args: unknown[]; timestamp: number }> = [];

    try {
      const variableStore = new VariableStore({
        global: globalVars,
        collection: collVars,
        environment: [...dataVars, ...envVars],
      });

      let requestUrl = variableStore.resolve(req.url);
      let requestHeaders: Record<string, string> = {};
      for (const h of req.headers.filter((h) => h.enabled)) {
        requestHeaders[h.key] = variableStore.resolve(h.value);
      }
      let requestBody: string | null = null;
      if (req.body.mode !== 'none' && req.body.raw) {
        requestBody = variableStore.resolve(req.body.raw);
      }
      let requestMethod = req.method;

      // Pre-request scripts: collection → request
      const preScripts = [collection.preRequestScript, req.preRequestScript].filter(Boolean) as string[];
      for (const script of preScripts) {
        const envScope = this.sandbox.createTrackedScope('environment', envVars);
        const collScope = this.sandbox.createTrackedScope('collection', collVars);
        const globalScope = this.sandbox.createTrackedScope('global', globalVars);

        const result = this.sandbox.execute(script, {
          environment: envScope,
          collectionVariables: collScope,
          globals: globalScope,
          request: { url: requestUrl, method: requestMethod, headers: requestHeaders, body: requestBody },
        });
        scriptLogs.push(...result.logs);

        if (result.requestMutations) {
          if (result.requestMutations.url) requestUrl = result.requestMutations.url;
          if (result.requestMutations.method) requestMethod = result.requestMutations.method as HttpMethod;
          if (result.requestMutations.headers) requestHeaders = result.requestMutations.headers;
          if (result.requestMutations.body !== undefined) requestBody = result.requestMutations.body;
        }

        if (result.error) {
          return {
            requestId: req.id, requestName: req.name, method: requestMethod, url: requestUrl,
            status: 0, statusText: 'Pre-request script error', responseTime: 0, size: 0,
            testResults: [], error: result.error, scriptLogs,
          };
        }
      }

      // Auth
      const auth = req.auth?.type === 'inherit' ? collection.auth : req.auth;
      if (auth && auth.type !== 'none' && auth.type !== 'inherit') {
        const prepared = await resolveAuth(auth, {
          method: requestMethod, url: requestUrl, headers: requestHeaders, body: requestBody,
        });
        requestUrl = prepared.url;
        requestHeaders = prepared.headers;
      }

      // Send
      const sendOpts: SendOptions = {
        method: requestMethod as SendOptions['method'],
        url: requestUrl,
        headers: requestHeaders,
        body: requestBody,
        settings: req.settings,
      };

      const response = await sendHttpRequest(sendOpts);

      // Post-response scripts: request → collection
      let testResults: TestResult[] = [];
      const postScripts = [req.postResponseScript, collection.postResponseScript].filter(Boolean) as string[];
      for (const script of postScripts) {
        const envScope = this.sandbox.createTrackedScope('environment', envVars);
        const collScope = this.sandbox.createTrackedScope('collection', collVars);
        const globalScope = this.sandbox.createTrackedScope('global', globalVars);

        const parsedHeaders: Record<string, string | string[]> = {};
        for (const [k, v] of Object.entries(response.headers)) {
          if (v !== undefined) parsedHeaders[k] = v;
        }

        const result = this.sandbox.execute(script, {
          environment: envScope,
          collectionVariables: collScope,
          globals: globalScope,
          request: { url: requestUrl, method: requestMethod, headers: requestHeaders, body: requestBody },
          response: {
            code: response.status,
            status: response.statusText,
            headers: parsedHeaders,
            responseTime: response.timing.total,
            body: response.bodyText,
            json: () => { try { return JSON.parse(response.bodyText); } catch { return null; } },
            text: () => response.bodyText,
          },
        });

        scriptLogs.push(...result.logs);
        testResults = [...testResults, ...result.testResults];
      }

      return {
        requestId: req.id, requestName: req.name, method: requestMethod, url: requestUrl,
        status: response.status, statusText: response.statusText,
        responseTime: response.timing.total, size: response.size,
        testResults, scriptLogs,
      };
    } catch (err) {
      return {
        requestId: req.id, requestName: req.name, method: req.method, url: req.url,
        status: 0, statusText: 'Request failed',
        responseTime: 0, size: 0, testResults: [],
        error: err instanceof Error ? err.message : String(err), scriptLogs,
      };
    }
  }

  /**
   * Builds the ordered list of requests to run, optionally restricted to specific ids.
   * @param collection - Collection whose folder tree defines order.
   * @param requestIds - When non-empty, only requests whose id is listed are included.
   * @returns Flat request queue in traversal order.
   */
  private buildRequestQueue(collection: NexusCollection, requestIds?: string[]): NexusRequest[] {
    const allRequests = this.flattenRequests(collection.items);
    if (!requestIds || requestIds.length === 0) return allRequests;
    const idSet = new Set(requestIds);
    return allRequests.filter((r) => idSet.has(r.id));
  }

  /**
   * Depth-first flattens folders into a linear list of requests.
   * @param items - Requests and folders at the current tree level.
   * @returns All nested requests in encounter order.
   */
  private flattenRequests(items: (NexusRequest | NexusFolder)[]): NexusRequest[] {
    const result: NexusRequest[] = [];
    for (const item of items) {
      if (isRequest(item)) {
        result.push(item);
      } else if (isFolder(item)) {
        result.push(...this.flattenRequests(item.items));
      }
    }
    return result;
  }

  /**
   * Returns a promise that resolves after a fixed delay (used between requests).
   * @param ms - Milliseconds to wait.
   * @returns Promise that resolves when the delay elapses.
   */
  private delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}
