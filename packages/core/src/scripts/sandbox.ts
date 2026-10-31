import * as vm from 'node:vm';
import { randomUUID } from 'node:crypto';
import { types } from 'node:util';
import type { Variable } from '../types/index.js';
import { AssertionEngine } from './assertions.js';
import type { TestResult } from './assertions.js';
import { VariableStore } from '../environments/variable-store.js';
import { sandboxRuntime } from './sandbox-runtime.js';

interface RuntimeHandle {
  run: unknown;
  collect: unknown;
}

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
 * Reads an error message without running script code: proxies and accessors are never touched,
 * because a thrown value may come from the script's realm.
 * @param err - Value caught from the vm.
 * @returns The message, or a generic fallback.
 */
function safeErrorMessage(err: unknown): string {
  if (typeof err === 'string') return err;
  if (types.isNativeError(err)) {
    const desc = Object.getOwnPropertyDescriptor(err, 'message');
    if (desc && typeof desc.value === 'string') return desc.value;
  }
  return 'Script error';
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
 * Runs Nexus scripts in a Node `vm` context. The `nx` API and `console` are built inside the
 * context by {@link sandboxRuntime}; the host talks to them only through JSON strings.
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
    const testResults: TestResult[] = [];
    const executionControl: ExecutionControl = {
      nextRequest: undefined,
      skipped: false,
    };
    const result: ScriptResult = {
      logs,
      testResults,
      variableChanges: [],
      executionControl,
    };

    const envScope = context.environment as TrackedScope;
    const collScope = context.collectionVariables as TrackedScope;
    const globalScope = context.globals as TrackedScope;
    const scopes: Record<string, VariableScopeAccess> = {
      environment: envScope,
      collectionVariables: collScope,
      globals: globalScope,
    };
    const ajv = new AssertionEngine();

    // Every argument and return value crossing into the context is a JSON string,
    // so scripts never hold a host-realm object (see sandbox-runtime.ts).
    const handle = (op: string, args: unknown[]): unknown => {
      const str = (i: number): string => {
        const v = args[i];
        if (typeof v !== 'string') throw new Error(`invalid argument to ${op}`);
        return v;
      };
      const [scopeName, method] = op.split('.');
      const scope = scopeName !== undefined ? scopes[scopeName] : undefined;
      if (scope && method) {
        switch (method) {
          case 'get': return scope.get(str(0));
          case 'set': scope.set(str(0), str(1)); return undefined;
          case 'unset': scope.unset(str(0)); return undefined;
          case 'toObject': return JSON.stringify(scope.toObject());
        }
      }
      switch (op) {
        case 'variables.get': {
          const key = str(0);
          return envScope.get(key) ?? collScope.get(key) ?? globalScope.get(key);
        }
        case 'replaceIn': return this.replaceIn(str(0), envScope, collScope, globalScope);
        case 'setNextRequest':
          executionControl.nextRequest = args[0] === null ? null : str(0);
          return undefined;
        case 'skipRequest':
          executionControl.skipped = true;
          return undefined;
        case 'log': {
          const level = str(0);
          if (level !== 'log' && level !== 'warn' && level !== 'error' && level !== 'info') {
            throw new Error('invalid log level');
          }
          const encoded = JSON.parse(str(1)) as Array<{ j?: string; s?: string; u?: 1 }>;
          logs.push({
            level,
            args: encoded.map((a) => (a.u ? undefined : a.j !== undefined ? JSON.parse(a.j) : String(a.s))),
            timestamp: typeof args[2] === 'number' ? args[2] : Date.now(),
          });
          return undefined;
        }
        case 'test': {
          const error = args[2];
          testResults.push({
            name: str(0),
            passed: args[1] === true,
            ...(typeof error === 'string' ? { error } : {}),
            duration: typeof args[3] === 'number' ? args[3] : 0,
          });
          return undefined;
        }
        case 'matchSchema': {
          const { v } = JSON.parse(str(0)) as { v: unknown };
          const schema = JSON.parse(str(1)) as Record<string, unknown>;
          try {
            ajv.expect(v).matchSchema(schema);
            return { pass: true, errors: '' };
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            return { pass: false, errors: message.replace(/^schema validation failed: /, '') };
          }
        }
        case 'visualizer': {
          if (!context.visualizer) throw new Error('visualizer unavailable');
          const data = args[1] === null ? undefined : JSON.parse(str(1));
          context.visualizer.set(str(0), data);
          return undefined;
        }
        case 'error':
          result.error = str(0);
          return undefined;
      }
      throw new Error(`unknown operation ${op}`);
    };

    const host = (op: string, argsJson: string): string => {
      try {
        if (typeof op !== 'string' || typeof argsJson !== 'string') throw new Error('invalid host call');
        const args = JSON.parse(argsJson) as unknown;
        if (!Array.isArray(args)) throw new Error('invalid host call');
        return JSON.stringify({ ok: true, v: handle(op, args) });
      } catch (err) {
        return JSON.stringify({ ok: false, e: err instanceof Error ? err.message : String(err) });
      }
    };

    const resp = context.response;
    let responsePayload: Record<string, unknown> | undefined;
    if (resp) {
      let json: { ok: boolean; v?: unknown; e?: string };
      try {
        json = { ok: true, v: resp.json() };
      } catch (err) {
        json = { ok: false, e: err instanceof Error ? err.message : String(err) };
      }
      responsePayload = {
        code: resp.code,
        status: resp.status,
        headers: resp.headers,
        responseTime: resp.responseTime,
        text: resp.text(),
        json,
      };
    }

    const payload = JSON.stringify({
      request: {
        url: context.request.url,
        method: context.request.method,
        headers: context.request.headers,
        body: context.request.body ?? null,
        bodyUndefined: context.request.body === undefined,
      },
      response: responsePayload,
      visualizer: Boolean(context.visualizer),
    });

    // A null-prototype sandbox keeps `this.constructor` lookups inside the context realm.
    const vmContext = vm.createContext(Object.create(null) as object, {
      codeGeneration: { strings: false, wasm: false },
      microtaskMode: 'afterEvaluate',
    });

    let finalRequest: MutableRequest | undefined;
    try {
      const install = new vm.Script(`(${sandboxRuntime.toString()}).bind(this)`, {
        filename: 'nexus-runtime.js',
      }).runInContext(vmContext) as (h: typeof host, p: string) => RuntimeHandle;
      const runtime = install(host, payload);

      let userFn: unknown;
      try {
        userFn = vm.compileFunction(script, [], {
          filename: 'nexus-script.js',
          parsingContext: vmContext,
        });
      } catch (err) {
        result.error = safeErrorMessage(err);
      }

      if (userFn !== undefined) {
        this.runTimed(vmContext, runtime.run, userFn);
      }
      const collected = this.runTimed(vmContext, runtime.collect);
      if (typeof collected === 'string') {
        const parsed = JSON.parse(collected) as MutableRequest & { bodyUndefined: boolean };
        finalRequest = {
          url: parsed.url,
          method: parsed.method,
          headers: parsed.headers,
          body: parsed.bodyUndefined ? undefined : parsed.body,
        };
      }
    } catch (err) {
      // Only engine errors (timeout, SIGINT, stack limits) arrive here.
      result.error ??= safeErrorMessage(err);
    }

    result.variableChanges = [
      ...envScope.changes,
      ...collScope.changes,
      ...globalScope.changes,
    ];

    if (
      finalRequest &&
      (finalRequest.url !== context.request.url ||
        finalRequest.method !== context.request.method ||
        JSON.stringify(finalRequest.headers) !== JSON.stringify(context.request.headers) ||
        finalRequest.body !== context.request.body)
    ) {
      result.requestMutations = { ...finalRequest };
    }

    return result;
  }

  /**
   * Calls an in-realm function under the execution timeout. The function and its
   * argument are parked on the context global under a random key only for the call.
   * @param vmContext - Context created by {@link execute}.
   * @param fn - In-realm function to call.
   * @param arg - Optional in-realm argument.
   * @returns The function's return value (only primitives are trusted by callers).
   */
  private runTimed(vmContext: vm.Context, fn: unknown, arg?: unknown): unknown {
    // Two separate properties: packing them into a host array would hand scripts a host object.
    const key = `__nx_${randomUUID().replace(/-/g, '')}`;
    Object.defineProperty(vmContext, `${key}_f`, { value: fn, configurable: true });
    Object.defineProperty(vmContext, `${key}_a`, { value: arg, configurable: true });
    return new vm.Script(
      `(() => { const f = this.${key}_f, a = this.${key}_a; delete this.${key}_f; delete this.${key}_a; return f(a); })()`,
      { filename: 'nexus-runner.js' },
    ).runInContext(vmContext, {
      timeout: this.timeoutMs,
      breakOnSigint: true,
    });
  }

  /**
   * Resolves `{{var}}` and `{{$dynamic}}` placeholders for `nx.variables.replaceIn`.
   * @returns The template with known placeholders replaced.
   */
  private replaceIn(
    template: string,
    envScope: VariableScopeAccess,
    collScope: VariableScopeAccess,
    globalScope: VariableScopeAccess,
  ): string {
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
