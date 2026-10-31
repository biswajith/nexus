/**
 * In-realm runtime for {@link ScriptSandbox}.
 *
 * This function is never called directly. The sandbox injects its source text
 * (`sandboxRuntime.toString()`) into a fresh `node:vm` context, so every object
 * and function a script can reach (`nx`, `console`, expectations, errors) is
 * created by that context's own intrinsics. Host-realm objects are never
 * handed to scripts: a host function's `constructor` is the host `Function`,
 * which evaluates code with full main-process authority.
 *
 * Rules for this file (enforced by `src/architecture.test.ts`):
 * - The function must be self-contained: no imports, no references to module scope.
 * - The only link to the host is `host(op, argsJson)`, which takes and returns
 *   JSON strings. Values cross the boundary as strings, never as objects.
 *
 * @param this - The context's global object (bound by the sandbox).
 * @param host - Host bridge; returns `{"ok":true,"v":...}` or `{"ok":false,"e":"..."}` as JSON.
 * @param payloadJson - JSON with the request snapshot, optional response, and feature flags.
 * @returns In-realm helpers the host invokes under the vm timeout.
 */
export function sandboxRuntime(
  this: unknown,
  host: (op: string, argsJson: string) => string,
  payloadJson: string,
): { run: (fn: () => void) => void; collect: () => string } {
  'use strict';

  // Capture intrinsics before script code can replace them.
  const g = this as unknown as Record<string, unknown>;
  const jsonParse = JSON.parse;
  const jsonStringify = JSON.stringify;
  const toStr = String;
  const isArray = Array.isArray;
  const objectKeys = Object.keys;
  const objectEntries = Object.entries;
  const defineProperty = Object.defineProperty;
  const freeze = Object.freeze;
  const now = Date.now;
  const ErrorCtor = Error;

  class AssertionError extends ErrorCtor {
    constructor(message: string) {
      super(message);
      this.name = 'AssertionError';
    }
  }

  function call(op: string, ...args: Array<string | number | boolean | null>): unknown {
    let raw: unknown;
    try {
      raw = host(op, jsonStringify(args));
    } catch {
      // Never let a host-realm error object reach script code.
      throw new ErrorCtor(`host call failed: ${op}`);
    }
    if (typeof raw !== 'string') throw new ErrorCtor(`host call failed: ${op}`);
    const reply = jsonParse(raw) as { ok: boolean; v?: unknown; e?: string };
    if (!reply.ok) throw new ErrorCtor(toStr(reply.e));
    return reply.v;
  }

  function describe(err: unknown): string {
    try {
      if (err instanceof ErrorCtor) return toStr(err.message);
      return toStr(err);
    } catch {
      return 'Script error';
    }
  }

  function encodeArg(arg: unknown): { j?: string; s?: string; u?: 1 } {
    if (arg === undefined) return { u: 1 };
    try {
      const j = jsonStringify(arg);
      if (typeof j === 'string') return { j };
    } catch {
      // fall through to String()
    }
    try {
      return { s: toStr(arg) };
    } catch {
      return { s: '[unprintable]' };
    }
  }

  const payload = jsonParse(payloadJson) as {
    request: { url: string; method: string; headers: Record<string, string>; body: string | null; bodyUndefined: boolean };
    response?: {
      code: number;
      status: string;
      headers: Record<string, string | string[]>;
      responseTime: number;
      text: string;
      json: { ok: boolean; v?: unknown; e?: string };
    };
    visualizer: boolean;
  };

  function scope(name: string) {
    return freeze({
      get: (key: string) => call(`${name}.get`, toStr(key)) as string | undefined,
      set: (key: string, value: string) => { call(`${name}.set`, toStr(key), toStr(value)); },
      unset: (key: string) => { call(`${name}.unset`, toStr(key)); },
      toObject: () => jsonParse(call(`${name}.toObject`) as string) as Record<string, string>,
    });
  }

  const request = {
    url: payload.request.url,
    method: payload.request.method,
    headers: payload.request.headers,
    body: payload.request.bodyUndefined ? undefined : payload.request.body as string | null | undefined,
  };

  class TypeChecker {
    constructor(private value: unknown, private negated: boolean) {}

    check(typeName: string): void {
      let pass: boolean;
      const v = this.value;
      switch (toStr(typeName).toLowerCase()) {
        case 'string': pass = typeof v === 'string'; break;
        case 'number': pass = typeof v === 'number'; break;
        case 'boolean': pass = typeof v === 'boolean'; break;
        case 'object': pass = typeof v === 'object' && v !== null && !isArray(v); break;
        case 'array': pass = isArray(v); break;
        case 'null': pass = v === null; break;
        case 'undefined': pass = v === undefined; break;
        case 'function': pass = typeof v === 'function'; break;
        default: pass = typeof v === typeName; break;
      }
      if (this.negated ? pass : !pass) {
        throw new AssertionError(
          this.negated
            ? `expected value to not be a ${typeName}`
            : `expected a ${typeName}, got ${typeof v}`,
        );
      }
    }
  }

  // Mirrors the chain API of `AssertionEngine` in assertions.ts.
  class Expectation {
    private negated = false;

    constructor(private value: unknown) {}

    get to(): this { return this; }
    get be(): this { return this; }
    get have(): this { return this; }
    get not(): this { this.negated = !this.negated; return this; }
    get a(): (typeName: string) => void {
      const checker = new TypeChecker(this.value, this.negated);
      return (typeName: string) => checker.check(typeName);
    }
    get an(): (typeName: string) => void { return this.a; }
    get true(): void { this.assert(this.value === true, `expected true, got ${this.value}`, 'expected value to not be true'); return; }
    get false(): void { this.assert(this.value === false, `expected false, got ${this.value}`, 'expected value to not be false'); return; }
    get null(): void { this.assert(this.value === null, `expected null, got ${this.value}`, 'expected value to not be null'); return; }
    get undefined(): void { this.assert(this.value === undefined, `expected undefined, got ${this.value}`, 'expected value to not be undefined'); return; }
    get empty(): void { this.assert(checkEmpty(this.value), 'expected value to be empty', 'expected value not to be empty'); return; }

    equal(expected: unknown): void {
      this.assert(
        deepEqual(this.value, expected),
        `expected ${jsonStringify(this.value)} to equal ${jsonStringify(expected)}`,
        `expected ${jsonStringify(this.value)} to not equal ${jsonStringify(expected)}`,
      );
    }

    eql(expected: unknown): void { this.equal(expected); }

    include(item: unknown): void {
      const v = this.value;
      let pass = false;
      if (typeof v === 'string' && typeof item === 'string') {
        pass = v.includes(item);
      } else if (isArray(v)) {
        pass = v.includes(item);
      } else if (v && typeof v === 'object') {
        pass = item !== null && typeof item === 'object'
          ? objectEntries(item as Record<string, unknown>).every(([k, x]) => (v as Record<string, unknown>)[k] === x)
          : false;
      }
      this.assert(
        pass,
        `expected ${jsonStringify(v)} to include ${jsonStringify(item)}`,
        `expected ${jsonStringify(v)} to not include ${jsonStringify(item)}`,
      );
    }

    contain(item: unknown): void { this.include(item); }

    property(name: string, val?: unknown): void {
      const obj = this.value as Record<string, unknown> | null | undefined;
      const has = obj != null && typeof obj === 'object' && name in obj;
      if (val !== undefined && has) {
        this.assert(
          obj![name] === val,
          `expected property "${name}" to equal ${jsonStringify(val)}, got ${jsonStringify(obj![name])}`,
          `expected property "${name}" to not equal ${jsonStringify(val)}`,
        );
      } else {
        this.assert(has, `expected object to have property "${name}"`, `expected object to not have property "${name}"`);
      }
    }

    length(expected: number): void {
      const actual = (this.value as { length?: number })?.length;
      this.assert(actual === expected, `expected length ${actual} to equal ${expected}`, `expected length ${actual} to not equal ${expected}`);
    }

    above(n: number): void {
      this.assert(typeof this.value === 'number' && this.value > n, `expected ${this.value} to be above ${n}`, `expected ${this.value} to not be above ${n}`);
    }

    below(n: number): void {
      this.assert(typeof this.value === 'number' && this.value < n, `expected ${this.value} to be below ${n}`, `expected ${this.value} to not be below ${n}`);
    }

    least(n: number): void {
      this.assert(typeof this.value === 'number' && this.value >= n, `expected ${this.value} to be at least ${n}`, `expected ${this.value} to not be at least ${n}`);
    }

    most(n: number): void {
      this.assert(typeof this.value === 'number' && this.value <= n, `expected ${this.value} to be at most ${n}`, `expected ${this.value} to not be at most ${n}`);
    }

    match(regex: RegExp): void {
      this.assert(typeof this.value === 'string' && regex.test(this.value), `expected "${this.value}" to match ${regex}`, `expected "${this.value}" to not match ${regex}`);
    }

    oneOf(list: unknown[]): void {
      this.assert(
        list.includes(this.value),
        `expected ${jsonStringify(this.value)} to be one of ${jsonStringify(list)}`,
        `expected ${jsonStringify(this.value)} to not be one of ${jsonStringify(list)}`,
      );
    }

    matchSchema(schema: Record<string, unknown>): void {
      const r = call('matchSchema', jsonStringify({ v: this.value }), jsonStringify(schema)) as { pass: boolean; errors: string };
      this.assert(r.pass, `schema validation failed: ${r.errors}`, 'expected value to not match schema');
    }

    ok(): void {
      this.assert(!!this.value, 'expected value to be truthy', 'expected value to be falsy');
    }

    private assert(condition: boolean, failMessage: string, negatedFailMessage: string): void {
      if (this.negated ? condition : !condition) {
        throw new AssertionError(this.negated ? negatedFailMessage : failMessage);
      }
    }
  }

  function deepEqual(a: unknown, b: unknown): boolean {
    if (a === b) return true;
    if (a === null || b === null) return false;
    if (typeof a !== typeof b) return false;
    if (typeof a !== 'object') return false;
    const aObj = a as Record<string, unknown>;
    const bObj = b as Record<string, unknown>;
    const aKeys = objectKeys(aObj);
    if (aKeys.length !== objectKeys(bObj).length) return false;
    return aKeys.every((key) => deepEqual(aObj[key], bObj[key]));
  }

  function checkEmpty(val: unknown): boolean {
    if (val === null || val === undefined) return true;
    if (typeof val === 'string') return val.length === 0;
    if (isArray(val)) return val.length === 0;
    if (typeof val === 'object') return objectKeys(val).length === 0;
    return false;
  }

  function log(level: string) {
    return (...args: unknown[]) => {
      call('log', level, jsonStringify(args.map(encodeArg)), now());
    };
  }

  const nx: Record<string, unknown> = {
    environment: scope('environment'),
    collectionVariables: scope('collectionVariables'),
    globals: scope('globals'),
    variables: freeze({
      get: (key: string) => call('variables.get', toStr(key)) as string | undefined,
      set: (key: string, value: string) => { call('environment.set', toStr(key), toStr(value)); },
      replaceIn: (template: string) => call('replaceIn', toStr(template)) as string,
    }),
    request: {
      get url() { return request.url; },
      set url(v: string) { request.url = v; },
      get method() { return request.method; },
      set method(v: string) { request.method = v; },
      get headers() { return request.headers; },
      set headers(v: Record<string, string>) { request.headers = v; },
      get body() { return request.body; },
      set body(v: string | null | undefined) { request.body = v; },
    },
    execution: freeze({
      setNextRequest: (name: string | null) => { call('setNextRequest', name === null ? null : toStr(name)); },
      skipRequest: () => { call('skipRequest'); },
    }),
    test: (name: string, fn: () => void) => {
      const label = toStr(name);
      const start = now();
      let error: string | null = null;
      try {
        fn();
      } catch (err) {
        error = describe(err);
      }
      call('test', label, error === null, error, now() - start);
    },
    expect: (value: unknown) => new Expectation(value),
  };

  const resp = payload.response;
  if (resp) {
    nx.response = freeze({
      get code() { return resp.code; },
      get status() { return resp.status; },
      get headers() { return resp.headers; },
      get responseTime() { return resp.responseTime; },
      json: () => {
        if (!resp.json.ok) throw new ErrorCtor(toStr(resp.json.e));
        return resp.json.v === undefined ? undefined : jsonParse(jsonStringify(resp.json.v));
      },
      text: () => resp.text,
    });
  }

  if (payload.visualizer) {
    nx.visualizer = freeze({
      set: (template: string, data: unknown) => {
        const encoded = jsonStringify(data);
        call('visualizer', toStr(template), encoded === undefined ? null : encoded);
      },
    });
  }

  for (const [name, value] of [
    ['nx', nx],
    ['console', freeze({ log: log('log'), warn: log('warn'), error: log('error'), info: log('info') })],
    ['global', undefined],
    ['globalThis', undefined],
  ] as Array<[string, unknown]>) {
    defineProperty(g, name, { value, writable: false, configurable: false, enumerable: false });
  }

  return freeze({
    run: (fn: () => void) => {
      try {
        fn();
      } catch (err) {
        try {
          call('error', describe(err));
        } catch {
          // The host records a generic error when this fails.
        }
      }
    },
    collect: () => {
      const headers: Record<string, string> = {};
      const h = request.headers;
      if (h && typeof h === 'object') {
        for (const k of objectKeys(h)) headers[toStr(k)] = toStr((h as Record<string, unknown>)[k]);
      }
      const body = request.body;
      return jsonStringify({
        url: toStr(request.url),
        method: toStr(request.method),
        headers,
        body: body === undefined || body === null ? null : toStr(body),
        bodyUndefined: body === undefined,
      });
    },
  });
}
