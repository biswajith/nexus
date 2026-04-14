import { describe, it, expect, beforeEach } from 'vitest';
import { PostmanImporter } from './postman-importer.js';

function makeCollection(overrides: Record<string, unknown> = {}) {
  return {
    info: { name: 'Test Collection', description: 'A test', schema: 'https://schema.getpostman.com/json/collection/v2.1.0/collection.json' },
    item: [],
    ...overrides,
  };
}

function makeRequestItem(overrides: Record<string, unknown> = {}) {
  return {
    name: 'My Request',
    request: {
      method: 'GET',
      url: 'https://api.example.com/users',
      header: [],
      ...((overrides.request as Record<string, unknown>) ?? {}),
    },
    ...Object.fromEntries(Object.entries(overrides).filter(([k]) => k !== 'request')),
  };
}

describe('PostmanImporter', () => {
  let importer: PostmanImporter;

  beforeEach(() => {
    importer = new PostmanImporter();
  });

  describe('importCollection', () => {
    it('returns one collection with correct name and description', () => {
      const result = importer.importCollection(makeCollection());
      expect(result.collections).toHaveLength(1);
      expect(result.collections[0]!.name).toBe('Test Collection');
      expect(result.collections[0]!.description).toBe('A test');
      expect(result.environments).toHaveLength(0);
    });

    it('generates a collection id starting with col_', () => {
      const result = importer.importCollection(makeCollection());
      expect(result.collections[0]!.id).toMatch(/^col_[a-f0-9]{8}$/);
    });

    it('imports an empty item list', () => {
      const result = importer.importCollection(makeCollection({ item: [] }));
      expect(result.collections[0]!.items).toHaveLength(0);
    });

    it('imports collection-level variables', () => {
      const result = importer.importCollection(makeCollection({
        variable: [
          { key: 'base_url', value: 'https://api.example.com' },
          { key: 'token', value: 'abc123', type: 'secret', disabled: true, description: 'API token' },
        ],
      }));
      const vars = result.collections[0]!.variables;
      expect(vars).toHaveLength(2);
      expect(vars[0]).toEqual({ key: 'base_url', value: 'https://api.example.com', type: 'string', enabled: true, description: undefined });
      expect(vars[1]).toEqual({ key: 'token', value: 'abc123', type: 'secret', enabled: false, description: 'API token' });
    });

    it('imports collection-level auth', () => {
      const result = importer.importCollection(makeCollection({
        auth: { type: 'bearer', bearer: [{ key: 'token', value: 'xyz' }] },
      }));
      expect(result.collections[0]!.auth).toEqual({ type: 'bearer', token: 'xyz' });
    });

    it('imports collection-level pre-request and test scripts', () => {
      const result = importer.importCollection(makeCollection({
        event: [
          { listen: 'prerequest', script: { exec: ['pm.environment.set("foo", "bar");'] } },
          { listen: 'test', script: { exec: ['pm.test("ok", function() {});'] } },
        ],
      }));
      const col = result.collections[0]!;
      expect(col.preRequestScript).toContain('nx.environment.set');
      expect(col.postResponseScript).toContain('nx.test');
    });
  });

  describe('folders', () => {
    it('converts a Postman folder to a NexusFolder', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'Users',
          item: [makeRequestItem()],
        }],
      }));
      const folder = result.collections[0]!.items[0]!;
      expect('items' in folder).toBe(true);
      if ('items' in folder) {
        expect(folder.name).toBe('Users');
        expect(folder.id).toMatch(/^folder_/);
        expect(folder.items).toHaveLength(1);
      }
    });

    it('handles nested folders recursively', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'API',
          item: [{
            name: 'v1',
            item: [makeRequestItem()],
          }],
        }],
      }));
      const outer = result.collections[0]!.items[0] as { items: unknown[] };
      expect(outer.items).toHaveLength(1);
      const inner = outer.items[0] as { name: string; items: unknown[] };
      expect(inner.name).toBe('v1');
      expect(inner.items).toHaveLength(1);
    });

    it('imports folder-level auth and scripts', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'Secured',
          item: [],
          auth: { type: 'basic', basic: [{ key: 'username', value: 'admin' }, { key: 'password', value: 'pass' }] },
          event: [{ listen: 'prerequest', script: { exec: ['console.log("hi");'] } }],
        }],
      }));
      const folder = result.collections[0]!.items[0] as { auth?: { type: string }; preRequestScript?: string };
      expect(folder.auth).toEqual({ type: 'basic', username: 'admin', password: 'pass' });
      expect(folder.preRequestScript).toContain('console.log');
    });
  });

  describe('requests', () => {
    it('converts a basic GET request', () => {
      const result = importer.importCollection(makeCollection({
        item: [makeRequestItem()],
      }));
      const req = result.collections[0]!.items[0] as { method: string; url: string; name: string; id: string };
      expect(req.method).toBe('GET');
      expect(req.url).toBe('https://api.example.com/users');
      expect(req.name).toBe('My Request');
      expect(req.id).toMatch(/^req_/);
    });

    it('defaults method to GET when missing', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'No Method',
          request: { url: 'https://example.com' },
        }],
      }));
      const req = result.collections[0]!.items[0] as { method: string };
      expect(req.method).toBe('GET');
    });

    it('uppercases the method', () => {
      const result = importer.importCollection(makeCollection({
        item: [{ name: 'Post', request: { method: 'post', url: 'https://example.com' } }],
      }));
      const req = result.collections[0]!.items[0] as { method: string };
      expect(req.method).toBe('POST');
    });

    it('handles a URL object with raw field', () => {
      const result = importer.importCollection(makeCollection({
        item: [{ name: 'R', request: { method: 'GET', url: { raw: 'https://api.example.com/test', host: ['api', 'example', 'com'], path: ['test'] } } }],
      }));
      const req = result.collections[0]!.items[0] as { url: string };
      expect(req.url).toBe('https://api.example.com/test');
    });

    it('returns empty string for missing URL', () => {
      const result = importer.importCollection(makeCollection({
        item: [{ name: 'R', request: { method: 'GET' } }],
      }));
      const req = result.collections[0]!.items[0] as { url: string };
      expect(req.url).toBe('');
    });

    it('imports headers with enabled/disabled state', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'GET',
            url: 'https://example.com',
            header: [
              { key: 'Accept', value: 'application/json', disabled: false },
              { key: 'X-Debug', value: 'true', disabled: true, description: 'debug header' },
            ],
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { headers: Array<{ key: string; value: string; enabled: boolean; description?: string }> };
      expect(req.headers).toHaveLength(2);
      expect(req.headers[0]).toEqual({ key: 'Accept', value: 'application/json', enabled: true, description: undefined });
      expect(req.headers[1]).toEqual({ key: 'X-Debug', value: 'true', enabled: false, description: 'debug header' });
    });

    it('extracts query params from URL string', () => {
      const result = importer.importCollection(makeCollection({
        item: [{ name: 'R', request: { method: 'GET', url: 'https://example.com/search?q=hello&page=1' } }],
      }));
      const req = result.collections[0]!.items[0] as { params: Array<{ key: string; value: string }> };
      expect(req.params).toHaveLength(2);
      expect(req.params[0]!.key).toBe('q');
      expect(req.params[0]!.value).toBe('hello');
      expect(req.params[1]!.key).toBe('page');
      expect(req.params[1]!.value).toBe('1');
    });

    it('extracts query params from URL object', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'GET',
            url: {
              raw: 'https://example.com/search?q=hello',
              query: [
                { key: 'q', value: 'hello', disabled: false },
                { key: 'limit', value: '10', disabled: true, description: 'page size' },
              ],
            },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { params: Array<{ key: string; value: string; enabled: boolean; description?: string }> };
      expect(req.params).toHaveLength(2);
      expect(req.params[0]).toEqual({ key: 'q', value: 'hello', enabled: true, description: undefined });
      expect(req.params[1]).toEqual({ key: 'limit', value: '10', enabled: false, description: 'page size' });
    });

    it('returns empty params for URL with no query', () => {
      const result = importer.importCollection(makeCollection({
        item: [{ name: 'R', request: { method: 'GET', url: 'https://example.com' } }],
      }));
      const req = result.collections[0]!.items[0] as { params: unknown[] };
      expect(req.params).toHaveLength(0);
    });

    it('defaults auth to inherit when request has no auth', () => {
      const result = importer.importCollection(makeCollection({
        item: [makeRequestItem()],
      }));
      const req = result.collections[0]!.items[0] as { auth: { type: string } };
      expect(req.auth).toEqual({ type: 'inherit' });
    });

    it('imports request-level auth', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'GET',
            url: 'https://example.com',
            auth: { type: 'bearer', bearer: [{ key: 'token', value: 'my-token' }] },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { auth: { type: string; token: string } };
      expect(req.auth).toEqual({ type: 'bearer', token: 'my-token' });
    });

    it('imports request pre-request and test scripts', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: { method: 'GET', url: 'https://example.com' },
          event: [
            { listen: 'prerequest', script: { exec: ['pm.environment.set("key", "val");'] } },
            { listen: 'test', script: { exec: ['pm.test("status", function() {', '  pm.response.to.have.status(200);', '});'] } },
          ],
        }],
      }));
      const req = result.collections[0]!.items[0] as { preRequestScript: string; postResponseScript: string };
      expect(req.preRequestScript).toContain('nx.environment.set');
      expect(req.postResponseScript).toContain('nx.test');
    });

    it('returns empty script when event has no exec', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: { method: 'GET', url: 'https://example.com' },
          event: [{ listen: 'prerequest', script: {} }],
        }],
      }));
      const req = result.collections[0]!.items[0] as { preRequestScript: string };
      expect(req.preRequestScript).toBe('');
    });

    it('returns empty script when exec lines are all empty', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: { method: 'GET', url: 'https://example.com' },
          event: [{ listen: 'prerequest', script: { exec: ['', '  '] } }],
        }],
      }));
      const req = result.collections[0]!.items[0] as { preRequestScript: string };
      expect(req.preRequestScript).toBe('');
    });
  });

  describe('body types', () => {
    it('returns none for missing body', () => {
      const result = importer.importCollection(makeCollection({
        item: [makeRequestItem()],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string } };
      expect(req.body.mode).toBe('none');
    });

    it('returns none for body with no mode', () => {
      const result = importer.importCollection(makeCollection({
        item: [{ name: 'R', request: { method: 'POST', url: 'https://e.com', body: {} } }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string } };
      expect(req.body.mode).toBe('none');
    });

    it('converts raw JSON body', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: { mode: 'raw', raw: '{"key":"value"}', options: { raw: { language: 'json' } } },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string; raw: string } };
      expect(req.body.mode).toBe('json');
      expect(req.body.raw).toBe('{"key":"value"}');
    });

    it('detects JSON mode from raw content starting with {', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: { mode: 'raw', raw: '{"auto": true}' },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string } };
      expect(req.body.mode).toBe('json');
    });

    it('converts raw XML body', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: { mode: 'raw', raw: '<root/>', options: { raw: { language: 'xml' } } },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string } };
      expect(req.body.mode).toBe('xml');
    });

    it('converts raw HTML body', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: { mode: 'raw', raw: '<html></html>', options: { raw: { language: 'html' } } },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string } };
      expect(req.body.mode).toBe('html');
    });

    it('defaults raw body to text mode', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: { mode: 'raw', raw: 'plain text' },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string; raw: string } };
      expect(req.body.mode).toBe('text');
      expect(req.body.raw).toBe('plain text');
    });

    it('converts urlencoded body', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: {
              mode: 'urlencoded',
              urlencoded: [
                { key: 'user', value: 'alice', disabled: false },
                { key: 'pass', value: 'secret', disabled: true, description: 'password' },
              ],
            },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string; urlencoded: Array<{ key: string; value: string; enabled: boolean }> } };
      expect(req.body.mode).toBe('x-www-form-urlencoded');
      expect(req.body.urlencoded).toHaveLength(2);
      expect(req.body.urlencoded[0]!.enabled).toBe(true);
      expect(req.body.urlencoded[1]!.enabled).toBe(false);
    });

    it('converts formdata body', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: {
              mode: 'formdata',
              formdata: [
                { key: 'file', src: '/path/to/file', type: 'file' },
                { key: 'name', value: 'doc', type: 'text' },
              ],
            },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string; formData: Array<{ key: string; value: string; type: string }> } };
      expect(req.body.mode).toBe('form-data');
      expect(req.body.formData).toHaveLength(2);
      expect(req.body.formData[0]!.value).toBe('/path/to/file');
      expect(req.body.formData[0]!.type).toBe('file');
      expect(req.body.formData[1]!.type).toBe('text');
    });

    it('converts graphql body', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: {
              mode: 'graphql',
              graphql: { query: '{ users { id } }', variables: '{"limit":10}' },
            },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string; graphql: { query: string; variables: string } } };
      expect(req.body.mode).toBe('graphql');
      expect(req.body.graphql.query).toBe('{ users { id } }');
      expect(req.body.graphql.variables).toBe('{"limit":10}');
    });

    it('converts file body to binary mode', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: { mode: 'file' },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string } };
      expect(req.body.mode).toBe('binary');
    });

    it('returns none for unknown body mode', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: {
            method: 'POST',
            url: 'https://e.com',
            body: { mode: 'something-new' },
          },
        }],
      }));
      const req = result.collections[0]!.items[0] as { body: { mode: string } };
      expect(req.body.mode).toBe('none');
    });
  });

  describe('auth types', () => {
    function importWithAuth(auth: Record<string, unknown>) {
      const result = importer.importCollection(makeCollection({
        auth,
      }));
      return result.collections[0]!.auth;
    }

    it('converts bearer auth', () => {
      expect(importWithAuth({ type: 'bearer', bearer: [{ key: 'token', value: 'abc' }] }))
        .toEqual({ type: 'bearer', token: 'abc' });
    });

    it('converts basic auth', () => {
      expect(importWithAuth({
        type: 'basic',
        basic: [{ key: 'username', value: 'user' }, { key: 'password', value: 'pass' }],
      })).toEqual({ type: 'basic', username: 'user', password: 'pass' });
    });

    it('converts apikey auth', () => {
      expect(importWithAuth({
        type: 'apikey',
        apikey: [{ key: 'key', value: 'X-API-Key' }, { key: 'value', value: 'secret' }, { key: 'in', value: 'header' }],
      })).toEqual({ type: 'api-key', key: 'X-API-Key', value: 'secret', addTo: 'header' });
    });

    it('converts apikey auth with query location', () => {
      expect(importWithAuth({
        type: 'apikey',
        apikey: [{ key: 'key', value: 'api_key' }, { key: 'value', value: 's' }, { key: 'in', value: 'query' }],
      })).toEqual({ type: 'api-key', key: 'api_key', value: 's', addTo: 'query' });
    });

    it('converts oauth2 auth', () => {
      const auth = importWithAuth({
        type: 'oauth2',
        oauth2: [
          { key: 'accessToken', value: 'tok' },
          { key: 'tokenType', value: 'Bearer' },
          { key: 'addTokenTo', value: 'header' },
          { key: 'authUrl', value: 'https://auth.example.com' },
          { key: 'accessTokenUrl', value: 'https://token.example.com' },
          { key: 'clientId', value: 'cid' },
          { key: 'clientSecret', value: 'cs' },
          { key: 'scope', value: 'read' },
          { key: 'redirect_uri', value: 'https://cb.example.com' },
        ],
      });
      expect(auth!.type).toBe('oauth2');
      expect((auth as Record<string, unknown>).accessToken).toBe('tok');
      expect((auth as Record<string, unknown>).clientId).toBe('cid');
    });

    it('converts digest auth', () => {
      const auth = importWithAuth({
        type: 'digest',
        digest: [
          { key: 'username', value: 'u' },
          { key: 'password', value: 'p' },
          { key: 'realm', value: 'r' },
          { key: 'nonce', value: 'n' },
          { key: 'algorithm', value: 'SHA-256' },
          { key: 'qop', value: 'auth' },
          { key: 'opaque', value: 'o' },
        ],
      });
      expect(auth).toEqual({
        type: 'digest', username: 'u', password: 'p', realm: 'r',
        nonce: 'n', algorithm: 'SHA-256', qop: 'auth', opaque: 'o',
      });
    });

    it('uses MD5 as default digest algorithm', () => {
      const auth = importWithAuth({ type: 'digest', digest: [{ key: 'username', value: 'u' }] });
      expect((auth as Record<string, unknown>).algorithm).toBe('MD5');
    });

    it('converts awsv4 auth', () => {
      const auth = importWithAuth({
        type: 'awsv4',
        awsv4: [
          { key: 'accessKey', value: 'AK' },
          { key: 'secretKey', value: 'SK' },
          { key: 'region', value: 'us-east-1' },
          { key: 'service', value: 's3' },
          { key: 'sessionToken', value: 'ST' },
        ],
      });
      expect(auth).toEqual({ type: 'aws-sig-v4', accessKey: 'AK', secretKey: 'SK', region: 'us-east-1', service: 's3', sessionToken: 'ST' });
    });

    it('converts hawk auth to none with warning', () => {
      const result = importer.importCollection(makeCollection({
        auth: { type: 'hawk', hawk: [] },
      }));
      expect(result.collections[0]!.auth).toEqual({ type: 'none' });
      expect(result.warnings).toContain('Hawk auth imported but not fully supported in Nexus.');
    });

    it('converts noauth to none', () => {
      expect(importWithAuth({ type: 'noauth' })).toEqual({ type: 'none' });
    });

    it('defaults unknown auth to none with warning', () => {
      const result = importer.importCollection(makeCollection({
        auth: { type: 'ntlm' },
      }));
      expect(result.collections[0]!.auth).toEqual({ type: 'none' });
      expect(result.warnings.some((w) => w.includes('Unknown auth type "ntlm"'))).toBe(true);
    });
  });

  describe('importEnvironment', () => {
    it('converts a Postman environment into a Nexus environment', () => {
      const result = importer.importEnvironment({
        name: 'Production',
        values: [
          { key: 'host', value: 'api.prod.com' },
          { key: 'secret', value: 'hidden', type: 'secret', disabled: true },
        ],
      });
      expect(result.environments).toHaveLength(1);
      expect(result.collections).toHaveLength(0);
      expect(result.warnings).toHaveLength(0);

      const env = result.environments[0]!;
      expect(env.name).toBe('Production');
      expect(env.variables).toHaveLength(2);
      expect(env.variables[0]).toEqual({ key: 'host', value: 'api.prod.com', type: 'string', enabled: true, description: undefined });
      expect(env.variables[1]).toEqual({ key: 'secret', value: 'hidden', type: 'secret', enabled: false, description: undefined });
    });

    it('handles an environment with no values', () => {
      const result = importer.importEnvironment({ name: 'Empty', values: [] });
      expect(result.environments[0]!.variables).toHaveLength(0);
    });
  });

  describe('warnings', () => {
    it('resets warnings between imports', () => {
      importer.importCollection(makeCollection({
        auth: { type: 'hawk', hawk: [] },
      }));
      const result2 = importer.importCollection(makeCollection());
      expect(result2.warnings).toHaveLength(0);
    });

    it('produces script warnings for unsupported APIs', () => {
      const result = importer.importCollection(makeCollection({
        item: [{
          name: 'R',
          request: { method: 'GET', url: 'https://e.com' },
          event: [{ listen: 'test', script: { exec: ['pm.sendRequest("https://x.com", function() {});'] } }],
        }],
      }));
      expect(result.warnings.some((w) => w.includes('[Script]') && w.includes('pm.sendRequest'))).toBe(true);
    });
  });
});
