import { describe, it, expect } from 'vitest';
import { ScriptRunner } from './script-runner.js';
import type { ResponseAccess } from './sandbox.js';
import type { NexusRequest, NexusFolder, NexusCollection, Variable } from '../types/index.js';

function makeVars(entries: Record<string, string>): Variable[] {
  return Object.entries(entries).map(([key, value]) => ({
    key,
    value,
    type: 'string' as const,
    enabled: true,
  }));
}

function makeRequest(overrides?: Partial<NexusRequest>): NexusRequest {
  return {
    id: 'req-1',
    name: 'Test Request',
    method: 'GET',
    url: 'https://api.example.com/data',
    headers: [{ key: 'Accept', value: 'application/json', enabled: true }],
    params: [],
    body: { mode: 'none' },
    auth: { type: 'none' },
    settings: {},
    ...overrides,
  };
}

function makeFolder(overrides?: Partial<NexusFolder>): NexusFolder {
  return {
    id: 'folder-1',
    name: 'Test Folder',
    items: [],
    ...overrides,
  };
}

function makeCollection(overrides?: Partial<NexusCollection>): NexusCollection {
  return {
    id: 'coll-1',
    name: 'Test Collection',
    variables: [],
    items: [],
    ...overrides,
  };
}

function makeResponse(overrides?: Partial<ResponseAccess>): ResponseAccess {
  const bodyText = '{"result":"ok"}';
  return {
    code: 200,
    status: 'OK',
    headers: { 'content-type': 'application/json' },
    responseTime: 50,
    body: bodyText,
    json: () => JSON.parse(bodyText),
    text: () => bodyText,
    ...overrides,
  };
}

describe('ScriptRunner', () => {
  const runner = new ScriptRunner();

  describe('runPreRequestChain', () => {
    it('returns empty results when no scripts exist', () => {
      const coll = makeCollection();
      const req = makeRequest();
      const { results, request } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(results).toHaveLength(0);
      expect(request.url).toBe('https://api.example.com/data');
    });

    it('runs collection preRequestScript', () => {
      const coll = makeCollection({
        preRequestScript: 'console.log("collection pre");',
      });
      const req = makeRequest();
      const { results } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(results).toHaveLength(1);
      expect(results[0].logs[0].args).toEqual(['collection pre']);
    });

    it('runs scripts in order: collection → folder → request', () => {
      const coll = makeCollection({
        preRequestScript: 'console.log("coll");',
      });
      const folder = makeFolder({
        preRequestScript: 'console.log("folder");',
      });
      const req = makeRequest({
        preRequestScript: 'console.log("request");',
      });
      const { results } = runner.runPreRequestChain(coll, req, [folder], [], []);
      expect(results).toHaveLength(3);
      expect(results[0].logs[0].args).toEqual(['coll']);
      expect(results[1].logs[0].args).toEqual(['folder']);
      expect(results[2].logs[0].args).toEqual(['request']);
    });

    it('runs multiple folder scripts in nesting order', () => {
      const coll = makeCollection();
      const outerFolder = makeFolder({
        id: 'outer',
        name: 'Outer',
        preRequestScript: 'console.log("outer");',
      });
      const innerFolder = makeFolder({
        id: 'inner',
        name: 'Inner',
        preRequestScript: 'console.log("inner");',
      });
      const req = makeRequest({ preRequestScript: 'console.log("req");' });
      const { results } = runner.runPreRequestChain(coll, req, [outerFolder, innerFolder], [], []);
      expect(results).toHaveLength(3);
      expect(results[0].logs[0].args).toEqual(['outer']);
      expect(results[1].logs[0].args).toEqual(['inner']);
      expect(results[2].logs[0].args).toEqual(['req']);
    });

    it('applies request mutations from scripts', () => {
      const coll = makeCollection({
        preRequestScript: 'nx.request.url = "https://mutated.com/api";',
      });
      const req = makeRequest();
      const { request } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(request.url).toBe('https://mutated.com/api');
    });

    it('chains mutations across scripts', () => {
      const coll = makeCollection({
        preRequestScript: 'nx.request.url = nx.request.url + "/v2";',
      });
      const req = makeRequest({
        url: 'https://api.com',
        preRequestScript: 'nx.request.url = nx.request.url + "/final";',
      });
      const { request } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(request.url).toBe('https://api.com/v2/final');
    });

    it('stops chain when script sets execution to skipped', () => {
      const coll = makeCollection({
        preRequestScript: `
          console.log("coll runs");
          nx.execution.skipRequest();
        `,
      });
      const req = makeRequest({ preRequestScript: 'console.log("should not run");' });
      const { results } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(results).toHaveLength(1);
      expect(results[0].logs[0].args).toEqual(['coll runs']);
      expect(results[0].executionControl.skipped).toBe(true);
    });

    it('propagates variable changes across scripts in the chain', () => {
      const coll = makeCollection({
        preRequestScript: 'nx.environment.set("token", "abc");',
      });
      const req = makeRequest({
        preRequestScript: 'console.log(nx.environment.get("token"));',
      });
      const { results } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(results).toHaveLength(2);
      expect(results[1].logs[0].args).toEqual(['abc']);
    });

    it('populates mutableRequest headers from enabled headers only', () => {
      const req = makeRequest({
        headers: [
          { key: 'Enabled', value: 'yes', enabled: true },
          { key: 'Disabled', value: 'no', enabled: false },
        ],
        preRequestScript: `
          console.log(nx.request.headers["Enabled"]);
          console.log(nx.request.headers["Disabled"]);
        `,
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual(['yes']);
      expect(results[0].logs[1].args).toEqual([undefined]);
    });

    it('handles script errors without crashing the chain', () => {
      const coll = makeCollection({
        preRequestScript: 'throw new Error("coll error");',
      });
      const req = makeRequest({
        preRequestScript: 'console.log("still runs");',
      });
      const { results } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(results).toHaveLength(2);
      expect(results[0].error).toContain('coll error');
      expect(results[1].logs[0].args).toEqual(['still runs']);
    });

    it('uses collection variables for the collection scope', () => {
      const coll = makeCollection({
        variables: makeVars({ apiUrl: 'https://api.com' }),
        preRequestScript: 'console.log(nx.collectionVariables.get("apiUrl"));',
      });
      const { results } = runner.runPreRequestChain(coll, makeRequest(), [], [], []);
      expect(results[0].logs[0].args).toEqual(['https://api.com']);
    });

    it('sets a collection variable and reads it in a later script in the chain', () => {
      const coll = makeCollection({
        preRequestScript: 'nx.collectionVariables.set("step", "init");',
      });
      const req = makeRequest({
        preRequestScript: 'console.log(nx.collectionVariables.get("step"));',
      });
      const { results } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(results).toHaveLength(2);
      expect(results[1].logs[0].args).toEqual(['init']);
    });

    it('records collection variable changes in variableChanges', () => {
      const coll = makeCollection({
        preRequestScript: 'nx.collectionVariables.set("token", "coll-token");',
      });
      const { results } = runner.runPreRequestChain(coll, makeRequest(), [], [], []);
      expect(results[0].variableChanges).toContainEqual({
        scope: 'collection',
        action: 'set',
        key: 'token',
        value: 'coll-token',
      });
    });

    it('overwrites a collection variable across scripts in the chain', () => {
      const coll = makeCollection({
        preRequestScript: 'nx.collectionVariables.set("phase", "pre-coll");',
      });
      const req = makeRequest({
        preRequestScript: `
          nx.collectionVariables.set("phase", "pre-req");
          console.log(nx.collectionVariables.get("phase"));
        `,
      });
      const { results } = runner.runPreRequestChain(coll, req, [], [], []);
      expect(results[1].logs[0].args).toEqual(['pre-req']);
    });

    it('uses envVars for environment scope and globalVars for global scope', () => {
      const envVars = makeVars({ env_key: 'env_val' });
      const globalVars = makeVars({ global_key: 'global_val' });
      const coll = makeCollection({
        preRequestScript: `
          console.log(nx.environment.get("env_key"));
          console.log(nx.globals.get("global_key"));
        `,
      });
      const { results } = runner.runPreRequestChain(coll, makeRequest(), [], envVars, globalVars);
      expect(results[0].logs[0].args).toEqual(['env_val']);
      expect(results[0].logs[1].args).toEqual(['global_val']);
    });
  });

  describe('runPostResponseChain', () => {
    it('returns empty results when no scripts exist', () => {
      const results = runner.runPostResponseChain(
        makeCollection(), makeRequest(), [], [], [], makeResponse(),
      );
      expect(results).toHaveLength(0);
    });

    it('runs scripts in order: request → folders (inner-to-outer) → collection', () => {
      const coll = makeCollection({
        postResponseScript: 'console.log("coll");',
      });
      const outerFolder = makeFolder({
        id: 'outer',
        name: 'Outer',
        postResponseScript: 'console.log("outer");',
      });
      const innerFolder = makeFolder({
        id: 'inner',
        name: 'Inner',
        postResponseScript: 'console.log("inner");',
      });
      const req = makeRequest({
        postResponseScript: 'console.log("request");',
      });
      const results = runner.runPostResponseChain(
        coll, req, [outerFolder, innerFolder], [], [], makeResponse(),
      );
      expect(results).toHaveLength(4);
      expect(results[0].logs[0].args).toEqual(['request']);
      expect(results[1].logs[0].args).toEqual(['inner']);
      expect(results[2].logs[0].args).toEqual(['outer']);
      expect(results[3].logs[0].args).toEqual(['coll']);
    });

    it('provides response data to scripts', () => {
      const response = makeResponse({ code: 404, status: 'Not Found' });
      const req = makeRequest({
        postResponseScript: `
          console.log(nx.response.code);
          console.log(nx.response.status);
        `,
      });
      const results = runner.runPostResponseChain(
        makeCollection(), req, [], [], [], response,
      );
      expect(results[0].logs[0].args).toEqual([404]);
      expect(results[0].logs[1].args).toEqual(['Not Found']);
    });

    it('allows test assertions against response', () => {
      const response = makeResponse({ code: 200 });
      const req = makeRequest({
        postResponseScript: `
          nx.test("status is 200", () => {
            nx.expect(nx.response.code).to.equal(200);
          });
        `,
      });
      const results = runner.runPostResponseChain(
        makeCollection(), req, [], [], [], response,
      );
      expect(results[0].testResults).toHaveLength(1);
      expect(results[0].testResults[0].passed).toBe(true);
    });

    it('propagates variable changes across post-response scripts', () => {
      const req = makeRequest({
        postResponseScript: 'nx.environment.set("extracted", "fromResp");',
      });
      const coll = makeCollection({
        postResponseScript: 'console.log(nx.environment.get("extracted"));',
      });
      const results = runner.runPostResponseChain(
        coll, req, [], [], [], makeResponse(),
      );
      expect(results).toHaveLength(2);
      expect(results[1].logs[0].args).toEqual(['fromResp']);
    });

    it('sets a collection variable in post-response and reads it in a later post-response script', () => {
      const req = makeRequest({
        postResponseScript: 'nx.collectionVariables.set("lastStatus", String(nx.response.code));',
      });
      const coll = makeCollection({
        postResponseScript: 'console.log(nx.collectionVariables.get("lastStatus"));',
      });
      const results = runner.runPostResponseChain(
        coll, req, [], [], [], makeResponse({ code: 201 }),
      );
      expect(results).toHaveLength(2);
      expect(results[1].logs[0].args).toEqual(['201']);
    });

    it('parses response JSON in scripts', () => {
      const body = '{"items":[1,2,3]}';
      const response = makeResponse({
        body,
        json: () => JSON.parse(body),
      });
      const req = makeRequest({
        postResponseScript: `
          const data = nx.response.json();
          console.log(data.items.length);
        `,
      });
      const results = runner.runPostResponseChain(
        makeCollection(), req, [], [], [], response,
      );
      expect(results[0].logs[0].args).toEqual([3]);
    });

    it('handles script errors without crashing the chain', () => {
      const req = makeRequest({
        postResponseScript: 'throw new Error("req error");',
      });
      const coll = makeCollection({
        postResponseScript: 'console.log("coll still runs");',
      });
      const results = runner.runPostResponseChain(
        coll, req, [], [], [], makeResponse(),
      );
      expect(results).toHaveLength(2);
      expect(results[0].error).toContain('req error');
      expect(results[1].logs[0].args).toEqual(['coll still runs']);
    });
  });

  describe('findParentFolders', () => {
    it('returns empty for a top-level request', () => {
      const req = makeRequest({ id: 'r1' });
      const coll = makeCollection({ items: [req] });
      const folders = runner.findParentFolders(coll, 'r1');
      expect(folders).toEqual([]);
    });

    it('returns the parent folder for a nested request', () => {
      const req = makeRequest({ id: 'r1' });
      const folder = makeFolder({ id: 'f1', name: 'Users', items: [req] });
      const coll = makeCollection({ items: [folder] });
      const folders = runner.findParentFolders(coll, 'r1');
      expect(folders).toHaveLength(1);
      expect(folders[0].id).toBe('f1');
    });

    it('returns folders from root to immediate parent for deeply nested request', () => {
      const req = makeRequest({ id: 'r1' });
      const inner = makeFolder({ id: 'f2', name: 'Inner', items: [req] });
      const outer = makeFolder({ id: 'f1', name: 'Outer', items: [inner] });
      const coll = makeCollection({ items: [outer] });
      const folders = runner.findParentFolders(coll, 'r1');
      expect(folders).toHaveLength(2);
      expect(folders[0].id).toBe('f1');
      expect(folders[1].id).toBe('f2');
    });

    it('returns empty when request id is not found', () => {
      const coll = makeCollection({ items: [makeRequest({ id: 'other' })] });
      const folders = runner.findParentFolders(coll, 'nonexistent');
      expect(folders).toEqual([]);
    });

    it('finds request among siblings', () => {
      const req1 = makeRequest({ id: 'r1' });
      const req2 = makeRequest({ id: 'r2' });
      const folder = makeFolder({ id: 'f1', name: 'Group', items: [req1, req2] });
      const coll = makeCollection({ items: [folder] });
      expect(runner.findParentFolders(coll, 'r2')).toHaveLength(1);
      expect(runner.findParentFolders(coll, 'r2')[0].id).toBe('f1');
    });
  });

  describe('extractBody (via pre-request chain)', () => {
    it('extracts raw body for json mode', () => {
      const req = makeRequest({
        body: { mode: 'json', raw: '{"a":1}' },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual(['{"a":1}']);
    });

    it('extracts raw body for xml mode', () => {
      const req = makeRequest({
        body: { mode: 'xml', raw: '<root/>' },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual(['<root/>']);
    });

    it('extracts raw body for text mode', () => {
      const req = makeRequest({
        body: { mode: 'text', raw: 'plain text' },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual(['plain text']);
    });

    it('extracts raw body for html mode', () => {
      const req = makeRequest({
        body: { mode: 'html', raw: '<b>hi</b>' },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual(['<b>hi</b>']);
    });

    it('encodes x-www-form-urlencoded body', () => {
      const req = makeRequest({
        body: {
          mode: 'x-www-form-urlencoded',
          urlencoded: [
            { key: 'user', value: 'alice', enabled: true },
            { key: 'pass', value: 'a b', enabled: true },
            { key: 'disabled', value: 'nope', enabled: false },
          ],
        },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual(['user=alice&pass=a%20b']);
    });

    it('serializes graphql body with variables', () => {
      const req = makeRequest({
        body: {
          mode: 'graphql',
          graphql: { query: '{ users { id } }', variables: '{"limit":10}' },
        },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      const parsed = JSON.parse(results[0].logs[0].args[0] as string);
      expect(parsed.query).toBe('{ users { id } }');
      expect(parsed.variables).toEqual({ limit: 10 });
    });

    it('returns null for mode none', () => {
      const req = makeRequest({
        body: { mode: 'none' },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual([null]);
    });

    it('returns null for raw modes when raw is undefined', () => {
      const req = makeRequest({
        body: { mode: 'json' },
        preRequestScript: 'console.log(nx.request.body);',
      });
      const { results } = runner.runPreRequestChain(makeCollection(), req, [], [], []);
      expect(results[0].logs[0].args).toEqual([null]);
    });
  });
});
