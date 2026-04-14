import { describe, it, expect } from 'vitest';
import { ScriptSandbox } from './sandbox.js';
import type { ScriptContext, MutableRequest, ResponseAccess } from './sandbox.js';
import type { Variable } from '../types/index.js';

function makeVars(entries: Record<string, string>): Variable[] {
  return Object.entries(entries).map(([key, value]) => ({
    key,
    value,
    type: 'string' as const,
    enabled: true,
  }));
}

function makeContext(overrides?: Partial<ScriptContext>): ScriptContext {
  const sandbox = new ScriptSandbox();
  return {
    environment: sandbox.createTrackedScope('environment', []),
    collectionVariables: sandbox.createTrackedScope('collection', []),
    globals: sandbox.createTrackedScope('global', []),
    request: {
      url: 'https://api.example.com/test',
      method: 'GET',
      headers: { 'Content-Type': 'application/json' },
      body: null,
    },
    ...overrides,
  };
}

function makeResponse(overrides?: Partial<ResponseAccess>): ResponseAccess {
  const bodyText = '{"status":"ok"}';
  return {
    code: 200,
    status: 'OK',
    headers: { 'content-type': 'application/json' },
    responseTime: 42,
    body: bodyText,
    json: () => JSON.parse(bodyText),
    text: () => bodyText,
    ...overrides,
  };
}

describe('ScriptSandbox', () => {
  const sandbox = new ScriptSandbox();

  describe('execute – console logging', () => {
    it('captures console.log calls', () => {
      const result = sandbox.execute('console.log("hello", 42);', makeContext());
      expect(result.logs).toHaveLength(1);
      expect(result.logs[0].level).toBe('log');
      expect(result.logs[0].args).toEqual(['hello', 42]);
    });

    it('captures console.warn, console.error, console.info', () => {
      const script = `
        console.warn("warning");
        console.error("error");
        console.info("info");
      `;
      const result = sandbox.execute(script, makeContext());
      expect(result.logs).toHaveLength(3);
      expect(result.logs[0].level).toBe('warn');
      expect(result.logs[1].level).toBe('error');
      expect(result.logs[2].level).toBe('info');
    });

    it('captures multiple arguments per log call', () => {
      const result = sandbox.execute('console.log("a", "b", { c: 1 });', makeContext());
      expect(result.logs[0].args).toEqual(['a', 'b', { c: 1 }]);
    });

    it('captures log timestamps', () => {
      const before = Date.now();
      const result = sandbox.execute('console.log("test");', makeContext());
      const after = Date.now();
      expect(result.logs[0].timestamp).toBeGreaterThanOrEqual(before);
      expect(result.logs[0].timestamp).toBeLessThanOrEqual(after);
    });
  });

  describe('execute – nx.test and assertions', () => {
    it('records passing tests', () => {
      const script = `
        nx.test("passes", () => {
          nx.expect(200).to.equal(200);
        });
      `;
      const result = sandbox.execute(script, makeContext());
      expect(result.testResults).toHaveLength(1);
      expect(result.testResults[0].passed).toBe(true);
      expect(result.testResults[0].name).toBe('passes');
    });

    it('records failing tests without halting execution', () => {
      const script = `
        nx.test("fails", () => {
          nx.expect(200).to.equal(404);
        });
        nx.test("still runs", () => {
          nx.expect(true).to.be.true;
        });
      `;
      const result = sandbox.execute(script, makeContext());
      expect(result.testResults).toHaveLength(2);
      expect(result.testResults[0].passed).toBe(false);
      expect(result.testResults[0].error).toBeDefined();
      expect(result.testResults[1].passed).toBe(true);
    });

    it('supports chained assertion methods', () => {
      const ctx = makeContext({ response: makeResponse() });
      const script = `
        nx.test("status is 200", () => {
          nx.expect(nx.response.code).to.equal(200);
        });
        nx.test("response time below 100", () => {
          nx.expect(nx.response.responseTime).to.be.below(100);
        });
      `;
      const result = sandbox.execute(script, ctx);
      expect(result.testResults.every((t) => t.passed)).toBe(true);
    });

    it('records test duration', () => {
      const script = `nx.test("quick", () => { nx.expect(1).to.equal(1); });`;
      const result = sandbox.execute(script, makeContext());
      expect(result.testResults[0].duration).toBeGreaterThanOrEqual(0);
    });
  });

  describe('execute – variable access', () => {
    it('reads environment variables via nx.environment.get', () => {
      const s = new ScriptSandbox();
      const envScope = s.createTrackedScope('environment', makeVars({ host: 'localhost' }));
      const ctx = makeContext({ environment: envScope });
      const script = `console.log(nx.environment.get("host"));`;
      const result = s.execute(script, ctx);
      expect(result.logs[0].args).toEqual(['localhost']);
    });

    it('sets and reads environment variables', () => {
      const s = new ScriptSandbox();
      const envScope = s.createTrackedScope('environment', []);
      const ctx = makeContext({ environment: envScope });
      const script = `
        nx.environment.set("token", "abc123");
        console.log(nx.environment.get("token"));
      `;
      const result = s.execute(script, ctx);
      expect(result.logs[0].args).toEqual(['abc123']);
      expect(result.variableChanges).toContainEqual({
        scope: 'environment',
        action: 'set',
        key: 'token',
        value: 'abc123',
      });
    });

    it('unsets variables and records changes', () => {
      const s = new ScriptSandbox();
      const envScope = s.createTrackedScope('environment', makeVars({ temp: 'val' }));
      const ctx = makeContext({ environment: envScope });
      const script = `nx.environment.unset("temp");`;
      const result = s.execute(script, ctx);
      expect(result.variableChanges).toContainEqual({
        scope: 'environment',
        action: 'unset',
        key: 'temp',
      });
    });

    it('reads collection variables', () => {
      const s = new ScriptSandbox();
      const collScope = s.createTrackedScope('collection', makeVars({ baseUrl: 'https://api.com' }));
      const ctx = makeContext({ collectionVariables: collScope });
      const script = `console.log(nx.collectionVariables.get("baseUrl"));`;
      const result = s.execute(script, ctx);
      expect(result.logs[0].args).toEqual(['https://api.com']);
    });

    it('reads global variables', () => {
      const s = new ScriptSandbox();
      const globalScope = s.createTrackedScope('global', makeVars({ apiKey: 'key123' }));
      const ctx = makeContext({ globals: globalScope });
      const script = `console.log(nx.globals.get("apiKey"));`;
      const result = s.execute(script, ctx);
      expect(result.logs[0].args).toEqual(['key123']);
    });

    it('nx.variables.get resolves from env > collection > global', () => {
      const s = new ScriptSandbox();
      const envScope = s.createTrackedScope('environment', makeVars({ shared: 'from-env' }));
      const collScope = s.createTrackedScope('collection', makeVars({ shared: 'from-coll', extra: 'coll-only' }));
      const globalScope = s.createTrackedScope('global', makeVars({ fallback: 'global-val' }));
      const ctx = makeContext({ environment: envScope, collectionVariables: collScope, globals: globalScope });
      const script = `
        console.log(nx.variables.get("shared"));
        console.log(nx.variables.get("extra"));
        console.log(nx.variables.get("fallback"));
      `;
      const result = s.execute(script, ctx);
      expect(result.logs[0].args).toEqual(['from-env']);
      expect(result.logs[1].args).toEqual(['coll-only']);
      expect(result.logs[2].args).toEqual(['global-val']);
    });

    it('nx.environment.toObject returns current variables', () => {
      const s = new ScriptSandbox();
      const envScope = s.createTrackedScope('environment', makeVars({ a: '1', b: '2' }));
      const ctx = makeContext({ environment: envScope });
      const script = `console.log(JSON.stringify(nx.environment.toObject()));`;
      const result = s.execute(script, ctx);
      expect(JSON.parse(result.logs[0].args[0] as string)).toEqual({ a: '1', b: '2' });
    });
  });

  describe('execute – request mutation', () => {
    it('mutates request URL', () => {
      const script = `nx.request.url = "https://new-host.com/v2";`;
      const result = sandbox.execute(script, makeContext());
      expect(result.requestMutations).toBeDefined();
      expect(result.requestMutations!.url).toBe('https://new-host.com/v2');
    });

    it('mutates request method', () => {
      const script = `nx.request.method = "POST";`;
      const result = sandbox.execute(script, makeContext());
      expect(result.requestMutations!.method).toBe('POST');
    });

    it('mutates request headers', () => {
      const script = `nx.request.headers = { ...nx.request.headers, "X-Custom": "value" };`;
      const result = sandbox.execute(script, makeContext());
      expect(result.requestMutations!.headers!['X-Custom']).toBe('value');
    });

    it('mutates request body', () => {
      const script = `nx.request.body = '{"new":true}';`;
      const result = sandbox.execute(script, makeContext());
      expect(result.requestMutations!.body).toBe('{"new":true}');
    });

    it('does not set requestMutations when request is unchanged', () => {
      const result = sandbox.execute('console.log("no change");', makeContext());
      expect(result.requestMutations).toBeUndefined();
    });
  });

  describe('execute – response access', () => {
    it('reads response code and status', () => {
      const ctx = makeContext({ response: makeResponse({ code: 201, status: 'Created' }) });
      const script = `
        console.log(nx.response.code);
        console.log(nx.response.status);
      `;
      const result = sandbox.execute(script, ctx);
      expect(result.logs[0].args).toEqual([201]);
      expect(result.logs[1].args).toEqual(['Created']);
    });

    it('reads response headers', () => {
      const ctx = makeContext({
        response: makeResponse({ headers: { 'x-request-id': 'abc' } }),
      });
      const script = `console.log(nx.response.headers["x-request-id"]);`;
      const result = sandbox.execute(script, ctx);
      expect(result.logs[0].args).toEqual(['abc']);
    });

    it('parses response JSON', () => {
      const body = '{"items":[1,2,3]}';
      const ctx = makeContext({
        response: makeResponse({
          body,
          json: () => JSON.parse(body),
          text: () => body,
        }),
      });
      const script = `
        const data = nx.response.json();
        console.log(data.items.length);
      `;
      const result = sandbox.execute(script, ctx);
      expect(result.logs[0].args).toEqual([3]);
    });

    it('reads response text', () => {
      const text = 'plain text';
      const ctx = makeContext({
        response: makeResponse({ body: text, text: () => text }),
      });
      const script = `console.log(nx.response.text());`;
      const result = sandbox.execute(script, ctx);
      expect(result.logs[0].args).toEqual(['plain text']);
    });

    it('reads response time', () => {
      const ctx = makeContext({ response: makeResponse({ responseTime: 150 }) });
      const script = `console.log(nx.response.responseTime);`;
      const result = sandbox.execute(script, ctx);
      expect(result.logs[0].args).toEqual([150]);
    });
  });

  describe('execute – execution control', () => {
    it('sets next request via nx.execution.setNextRequest', () => {
      const script = `nx.execution.setNextRequest("Login");`;
      const result = sandbox.execute(script, makeContext());
      expect(result.executionControl.nextRequest).toBe('Login');
    });

    it('sets next request to null to stop', () => {
      const script = `nx.execution.setNextRequest(null);`;
      const result = sandbox.execute(script, makeContext());
      expect(result.executionControl.nextRequest).toBeNull();
    });

    it('skips request via nx.execution.skipRequest', () => {
      const script = `nx.execution.skipRequest();`;
      const result = sandbox.execute(script, makeContext());
      expect(result.executionControl.skipped).toBe(true);
    });

    it('defaults to no skip and undefined nextRequest', () => {
      const result = sandbox.execute('// noop', makeContext());
      expect(result.executionControl.skipped).toBe(false);
      expect(result.executionControl.nextRequest).toBeUndefined();
    });
  });

  describe('execute – error handling', () => {
    it('captures syntax errors', () => {
      const result = sandbox.execute('function {', makeContext());
      expect(result.error).toBeDefined();
      expect(result.error).toContain('Unexpected token');
    });

    it('captures runtime errors', () => {
      const result = sandbox.execute('undefinedFunction();', makeContext());
      expect(result.error).toBeDefined();
    });

    it('captures thrown strings', () => {
      const result = sandbox.execute('throw "custom error";', makeContext());
      expect(result.error).toBe('custom error');
    });

    it('still returns logs and test results on error', () => {
      const script = `
        console.log("before");
        nx.test("early test", () => { nx.expect(1).to.equal(1); });
        throw new Error("boom");
      `;
      const result = sandbox.execute(script, makeContext());
      expect(result.error).toContain('boom');
      expect(result.logs).toHaveLength(1);
      expect(result.testResults).toHaveLength(1);
      expect(result.testResults[0].passed).toBe(true);
    });
  });

  describe('execute – sandbox restrictions', () => {
    it('prevents access to require', () => {
      const result = sandbox.execute('const fs = require("fs");', makeContext());
      expect(result.error).toBeDefined();
    });

    it('prevents access to process', () => {
      const result = sandbox.execute('console.log(process.env);', makeContext());
      expect(result.error).toBeDefined();
    });

    it('prevents access to global/globalThis', () => {
      const result = sandbox.execute('console.log(globalThis.constructor);', makeContext());
      expect(result.error).toBeDefined();
    });
  });

  describe('execute – timeout', () => {
    it('times out on infinite loops', () => {
      const fast = new ScriptSandbox(50);
      const result = fast.execute('while(true) {}', makeContext());
      expect(result.error).toBeDefined();
      expect(result.error).toContain('timed out');
    });
  });

  describe('execute – visualizer', () => {
    it('calls visualizer.set when provided', () => {
      let capturedTemplate = '';
      let capturedData: unknown;
      const ctx = makeContext({
        visualizer: {
          set: (t: string, d: unknown) => { capturedTemplate = t; capturedData = d; },
        },
      });
      const script = `nx.visualizer.set("<h1>{{title}}</h1>", { title: "Test" });`;
      sandbox.execute(script, ctx);
      expect(capturedTemplate).toBe('<h1>{{title}}</h1>');
      expect(capturedData).toEqual({ title: 'Test' });
    });
  });

  describe('createTrackedScope', () => {
    it('creates a scope with enabled variables only', () => {
      const vars: Variable[] = [
        { key: 'a', value: '1', type: 'string', enabled: true },
        { key: 'b', value: '2', type: 'string', enabled: false },
        { key: 'c', value: '3', type: 'string', enabled: true },
      ];
      const scope = sandbox.createTrackedScope('test', vars);
      expect(scope.get('a')).toBe('1');
      expect(scope.get('b')).toBeUndefined();
      expect(scope.get('c')).toBe('3');
    });

    it('tracks set operations', () => {
      const scope = sandbox.createTrackedScope('env', []);
      scope.set('key', 'value');
      expect(scope.get('key')).toBe('value');
      expect(scope.changes).toHaveLength(1);
      expect(scope.changes[0]).toEqual({ scope: 'env', action: 'set', key: 'key', value: 'value' });
    });

    it('tracks unset operations', () => {
      const scope = sandbox.createTrackedScope('env', makeVars({ x: '1' }));
      scope.unset('x');
      expect(scope.get('x')).toBeUndefined();
      expect(scope.changes).toHaveLength(1);
      expect(scope.changes[0]).toEqual({ scope: 'env', action: 'unset', key: 'x' });
    });

    it('returns a plain object from toObject', () => {
      const scope = sandbox.createTrackedScope('env', makeVars({ a: '1', b: '2' }));
      expect(scope.toObject()).toEqual({ a: '1', b: '2' });
    });
  });

  describe('execute – empty script', () => {
    it('returns clean result for empty script', () => {
      const result = sandbox.execute('', makeContext());
      expect(result.error).toBeUndefined();
      expect(result.logs).toEqual([]);
      expect(result.testResults).toEqual([]);
      expect(result.variableChanges).toEqual([]);
    });
  });
});
