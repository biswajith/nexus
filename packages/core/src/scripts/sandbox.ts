import * as vm from 'node:vm';
import type { Variable } from '../types/index.js';
import { AssertionEngine } from './assertions.js';
import type { TestResult } from './assertions.js';
import { VariableStore } from '../environments/variable-store.js';

export interface ScriptContext {
  environment: VariableScopeAccess;
  collectionVariables: VariableScopeAccess;
  globals: VariableScopeAccess;
  request: MutableRequest;
  response?: ResponseAccess;
  visualizer?: { set: (template: string, data: unknown) => void };
}

export interface VariableScopeAccess {
  get(key: string): string | undefined;
  set(key: string, value: string): void;
  unset(key: string): void;
  toObject(): Record<string, string>;
}

export interface MutableRequest {
  url: string;
  method: string;
  headers: Record<string, string>;
  body?: string | null;
}

export interface ResponseAccess {
  code: number;
  status: string;
  headers: Record<string, string | string[]>;
  responseTime: number;
  body: string;
  json(): unknown;
  text(): string;
}

export interface ExecutionControl {
  nextRequest: string | null | undefined;
  skipped: boolean;
}

export interface LogEntry {
  level: 'log' | 'warn' | 'error' | 'info';
  args: unknown[];
  timestamp: number;
}

export interface ScriptResult {
  logs: LogEntry[];
  testResults: TestResult[];
  variableChanges: VariableChange[];
  executionControl: ExecutionControl;
  error?: string;
  requestMutations?: {
    url?: string;
    method?: string;
    headers?: Record<string, string>;
    body?: string | null;
  };
}

export interface VariableChange {
  scope: string;
  action: 'set' | 'unset';
  key: string;
  value?: string;
}

/**
 * Variable scope that records set/unset operations for merging into {@link ScriptResult.variableChanges}.
 */
class TrackedScope implements VariableScopeAccess {
  private data: Map<string, string>;
  changes: VariableChange[] = [];

  /**
   * Seeds the scope from enabled variables only.
   * @param scopeName - Label stored on each recorded change.
   * @param variables - Initial variable definitions.
   */
  constructor(
    private scopeName: string,
    variables: Variable[],
  ) {
    this.data = new Map(
      variables.filter((v) => v.enabled).map((v) => [v.key, v.value]),
    );
  }

  /**
   * Reads a variable from this scope.
   * @param key - Variable name.
   * @returns The value, or undefined if absent.
   */
  get(key: string): string | undefined {
    return this.data.get(key);
  }

  /**
   * Sets a variable and records a `set` change.
   * @param key - Variable name.
   * @param value - New string value.
   */
  set(key: string, value: string): void {
    this.data.set(key, value);
    this.changes.push({ scope: this.scopeName, action: 'set', key, value });
  }

  /**
   * Deletes a variable and records an `unset` change.
   * @param key - Variable name.
   */
  unset(key: string): void {
    this.data.delete(key);
    this.changes.push({ scope: this.scopeName, action: 'unset', key });
  }

  /**
   * Snapshot of current variables as a plain object.
   * @returns Copy of all key/value pairs in this scope.
   */
  toObject(): Record<string, string> {
    const obj: Record<string, string> = {};
    for (const [k, v] of this.data) {
      obj[k] = v;
    }
    return obj;
  }
}

/**
 * Runs Nexus scripts in an isolated Node `vm` context with a constrained global and `nx` API surface.
 */
export class ScriptSandbox {
  private timeoutMs: number;

  /**
   * @param timeoutMs - Max milliseconds for `vm.Script` execution (default 30_000).
   */
  constructor(timeoutMs = 30_000) {
    this.timeoutMs = timeoutMs;
  }

  /**
   * Executes script text, capturing logs, tests, variable deltas, and optional request mutations.
   * @param script - JavaScript source to run.
   * @param context - Scopes, request, and optional response/visualizer exposed as `nx`.
   * @returns Aggregated script outcome; `error` is set when execution throws.
   */
  execute(script: string, context: ScriptContext): ScriptResult {
    const logs: LogEntry[] = [];
    const assertionEngine = new AssertionEngine();
    const executionControl: ExecutionControl = {
      nextRequest: undefined,
      skipped: false,
    };

    const envScope = context.environment as TrackedScope;
    const collScope = context.collectionVariables as TrackedScope;
    const globalScope = context.globals as TrackedScope;

    const mutableRequest = { ...context.request };

    const nx: Record<string, unknown> = {
      environment: {
        get: (key: string) => envScope.get(key),
        set: (key: string, value: string) => envScope.set(key, value),
        unset: (key: string) => envScope.unset(key),
        toObject: () => envScope.toObject(),
      },
      collectionVariables: {
        get: (key: string) => collScope.get(key),
        set: (key: string, value: string) => collScope.set(key, value),
        unset: (key: string) => collScope.unset(key),
        toObject: () => collScope.toObject(),
      },
      globals: {
        get: (key: string) => globalScope.get(key),
        set: (key: string, value: string) => globalScope.set(key, value),
        unset: (key: string) => globalScope.unset(key),
        toObject: () => globalScope.toObject(),
      },
      variables: {
        get: (key: string) => {
          return envScope.get(key) ?? collScope.get(key) ?? globalScope.get(key);
        },
        set: (key: string, value: string) => {
          envScope.set(key, value);
        },
        replaceIn: (template: string) => {
          const dynamicResolver = new VariableStore();
          return template.replace(/\{\{(.+?)\}\}/g, (match, key: string) => {
            const trimmed = key.trim();
            if (trimmed.startsWith('$')) {
              const resolved = dynamicResolver.resolve(`{{${trimmed}}}`);
              return resolved === `{{${trimmed}}}` ? match : resolved;
            }
            return envScope.get(trimmed)
              ?? collScope.get(trimmed)
              ?? globalScope.get(trimmed)
              ?? match;
          });
        },
      },
      request: {
        get url() { return mutableRequest.url; },
        set url(v: string) { mutableRequest.url = v; },
        get method() { return mutableRequest.method; },
        set method(v: string) { mutableRequest.method = v; },
        get headers() { return mutableRequest.headers; },
        set headers(v: Record<string, string>) { mutableRequest.headers = v; },
        get body() { return mutableRequest.body; },
        set body(v: string | null | undefined) { mutableRequest.body = v; },
      },
      execution: {
        setNextRequest: (name: string | null) => {
          executionControl.nextRequest = name;
        },
        skipRequest: () => {
          executionControl.skipped = true;
        },
      },
      test: (name: string, fn: () => void) => assertionEngine.test(name, fn),
      expect: (value: unknown) => assertionEngine.expect(value),
    };

    if (context.response) {
      const resp = context.response;
      nx.response = {
        get code() { return resp.code; },
        get status() { return resp.status; },
        get headers() { return resp.headers; },
        get responseTime() { return resp.responseTime; },
        json: () => resp.json(),
        text: () => resp.text(),
      };
    }

    if (context.visualizer) {
      nx.visualizer = {
        set: (template: string, data: unknown) => context.visualizer!.set(template, data),
      };
    }

    const consoleMethods = {
      log: (...args: unknown[]) => logs.push({ level: 'log', args, timestamp: Date.now() }),
      warn: (...args: unknown[]) => logs.push({ level: 'warn', args, timestamp: Date.now() }),
      error: (...args: unknown[]) => logs.push({ level: 'error', args, timestamp: Date.now() }),
      info: (...args: unknown[]) => logs.push({ level: 'info', args, timestamp: Date.now() }),
    };

    const sandbox = {
      nx,
      console: consoleMethods,
      setTimeout: undefined,
      setInterval: undefined,
      setImmediate: undefined,
      process: undefined,
      require: undefined,
      __filename: undefined,
      __dirname: undefined,
      global: undefined,
      globalThis: undefined,
    };

    const vmContext = vm.createContext(sandbox, {
      codeGeneration: { strings: false, wasm: false },
    });

    const result: ScriptResult = {
      logs,
      testResults: [],
      variableChanges: [],
      executionControl,
    };

    try {
      const compiledScript = new vm.Script(script, {
        filename: 'nexus-script.js',
      });
      compiledScript.runInContext(vmContext, {
        timeout: this.timeoutMs,
        breakOnSigint: true,
      });
    } catch (err) {
      result.error = err instanceof Error ? err.message : String(err);
    }

    result.testResults = assertionEngine.getResults();
    result.variableChanges = [
      ...envScope.changes,
      ...collScope.changes,
      ...globalScope.changes,
    ];

    if (
      mutableRequest.url !== context.request.url ||
      mutableRequest.method !== context.request.method ||
      mutableRequest.headers !== context.request.headers ||
      mutableRequest.body !== context.request.body
    ) {
      result.requestMutations = {
        url: mutableRequest.url,
        method: mutableRequest.method,
        headers: mutableRequest.headers,
        body: mutableRequest.body,
      };
    }

    return result;
  }

  /**
   * Factory for {@link TrackedScope} instances used to back `ScriptContext` variable scopes.
   * @param scopeName - Label recorded on each variable change from this scope.
   * @param variables - Initial variable definitions.
   * @returns A new tracked scope.
   */
  createTrackedScope(scopeName: string, variables: Variable[]): TrackedScope {
    return new TrackedScope(scopeName, variables);
  }
}
