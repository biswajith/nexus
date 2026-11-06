// In-realm runtime for `ScriptSandbox` (port of `sandbox-runtime.ts`).
//
// Evaluated as a function expression inside a fresh QuickJS context and called as
// `runtime.call(globalObject, host, payloadJson)`. Every object a script can reach (`nx`,
// `console`, expectations, errors) is created here, by the context's own intrinsics. Rust never
// hands a host object to scripts: the only link is `host(op, argsJson)`, which takes and returns
// JSON strings, and it is captured in the closure below (never stored on the global).
//
// Rules for this file (enforced by source-scan tests in `sandbox_tests.rs`):
// - No imports / `require`, no reference to module scope: it is a single self-contained expression.
// - It must not use any host, file or network API: there is nothing to reach.
(function (host, payloadJson) {
  'use strict';

  // Capture intrinsics before script code can replace them.
  const g = this;
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
  const FunctionCtor = Function;

  function call(op, ...args) {
    let raw;
    try {
      raw = host(op, jsonStringify(args));
    } catch {
      throw new ErrorCtor(`host call failed: ${op}`);
    }
    if (typeof raw !== 'string') throw new ErrorCtor(`host call failed: ${op}`);
    const reply = jsonParse(raw);
    if (!reply.ok) throw new ErrorCtor(toStr(reply.e));
    return reply.v;
  }

  function describe(err) {
    try {
      if (err instanceof ErrorCtor) return toStr(err.message);
      return toStr(err);
    } catch {
      return 'Script error';
    }
  }

  function encodeArg(arg) {
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

  const payload = jsonParse(payloadJson);

  /*__ASSERTIONS__*/

  function scope(name) {
    return freeze({
      get: (key) => call(`${name}.get`, toStr(key)),
      set: (key, value) => { call(`${name}.set`, toStr(key), toStr(value)); },
      unset: (key) => { call(`${name}.unset`, toStr(key)); },
      toObject: () => jsonParse(call(`${name}.toObject`)),
    });
  }

  const request = {
    url: payload.request.url,
    method: payload.request.method,
    headers: payload.request.headers,
    body: payload.request.bodyUndefined ? undefined : payload.request.body,
  };

  function log(level) {
    return (...args) => {
      call('log', level, jsonStringify(args.map(encodeArg)), now());
    };
  }

  const nx = {
    environment: scope('environment'),
    collectionVariables: scope('collectionVariables'),
    globals: scope('globals'),
    variables: freeze({
      get: (key) => call('variables.get', toStr(key)),
      set: (key, value) => { call('environment.set', toStr(key), toStr(value)); },
      replaceIn: (template) => call('replaceIn', toStr(template)),
    }),
    request: {
      get url() { return request.url; },
      set url(v) { request.url = v; },
      get method() { return request.method; },
      set method(v) { request.method = v; },
      get headers() { return request.headers; },
      set headers(v) { request.headers = v; },
      get body() { return request.body; },
      set body(v) { request.body = v; },
    },
    execution: freeze({
      setNextRequest: (name) => { call('setNextRequest', name === null ? null : toStr(name)); },
      skipRequest: () => { call('skipRequest'); },
    }),
    test: (name, fn) => {
      const label = toStr(name);
      const start = call('clock');
      let error = null;
      try {
        fn();
      } catch (err) {
        error = describe(err);
      }
      call('test', label, error === null, error, call('clock') - start);
    },
    expect: (value) => new Expectation(value),
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
      set: (template, data) => {
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
  ]) {
    defineProperty(g, name, { value, writable: false, configurable: false, enumerable: false });
  }

  return freeze({
    // Compiles `source` as a function body (like `vm.compileFunction`) and runs it; script errors
    // are reported through the host, never thrown to Rust.
    run: (source) => {
      try {
        const fn = new FunctionCtor(source);
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
      const headers = {};
      const h = request.headers;
      if (h && typeof h === 'object') {
        for (const k of objectKeys(h)) headers[toStr(k)] = toStr(h[k]);
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
})
