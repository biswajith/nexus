import { describe, it, expect, beforeEach } from 'vitest';
import { NexusExporter } from './nexus-exporter.js';
import type { NexusCollection, NexusRequest, NexusFolder, NexusEnvironment } from '../types/index.js';

function makeRequest(overrides: Partial<NexusRequest> = {}): NexusRequest {
  return {
    id: 'req_abc',
    name: 'Test Request',
    method: 'GET',
    url: 'https://api.example.com/users',
    headers: [],
    params: [],
    body: { mode: 'none' },
    auth: { type: 'inherit' },
    settings: {},
    ...overrides,
  };
}

function makeFolder(overrides: Partial<NexusFolder> = {}): NexusFolder {
  return {
    id: 'folder_abc',
    name: 'Test Folder',
    items: [],
    ...overrides,
  };
}

function makeCollection(overrides: Partial<NexusCollection> = {}): NexusCollection {
  return {
    id: 'col_abc',
    name: 'Test Collection',
    description: 'A collection',
    variables: [],
    items: [],
    ...overrides,
  };
}

describe('NexusExporter', () => {
  let exporter: NexusExporter;

  beforeEach(() => {
    exporter = new NexusExporter();
  });

  describe('exportAsNexusJson', () => {
    it('returns valid pretty-printed JSON', () => {
      const collection = makeCollection();
      const json = exporter.exportAsNexusJson(collection);
      const parsed = JSON.parse(json);
      expect(parsed.name).toBe('Test Collection');
      expect(parsed.id).toBe('col_abc');
    });

    it('includes all collection fields', () => {
      const collection = makeCollection({
        variables: [{ key: 'host', value: 'example.com', type: 'string', enabled: true }],
        items: [makeRequest()],
      });
      const parsed = JSON.parse(exporter.exportAsNexusJson(collection));
      expect(parsed.variables).toHaveLength(1);
      expect(parsed.items).toHaveLength(1);
    });

    it('produces indented output', () => {
      const json = exporter.exportAsNexusJson(makeCollection());
      expect(json).toContain('\n');
      expect(json).toContain('  ');
    });
  });

  describe('exportAsPostmanV21', () => {
    it('produces valid Postman v2.1 structure', () => {
      const json = exporter.exportAsPostmanV21(makeCollection());
      const parsed = JSON.parse(json);
      expect(parsed.info.name).toBe('Test Collection');
      expect(parsed.info.schema).toBe('https://schema.getpostman.com/json/collection/v2.1.0/collection.json');
      expect(parsed.info._postman_id).toBe('col_abc');
      expect(parsed.item).toEqual([]);
    });

    it('exports collection description', () => {
      const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({ description: 'My API' })));
      expect(parsed.info.description).toBe('My API');
    });

    it('exports collection variables', () => {
      const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
        variables: [
          { key: 'base_url', value: 'https://api.com', type: 'string', enabled: true, description: 'Base URL' },
          { key: 'token', value: 'hidden', type: 'secret', enabled: false },
        ],
      })));
      expect(parsed.variable).toHaveLength(2);
      expect(parsed.variable[0]).toEqual({ key: 'base_url', value: 'https://api.com', disabled: false, description: 'Base URL', type: 'string' });
      expect(parsed.variable[1].disabled).toBe(true);
      expect(parsed.variable[1].type).toBe('secret');
    });

    it('exports collection-level auth (bearer)', () => {
      const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
        auth: { type: 'bearer', token: 'abc' },
      })));
      expect(parsed.auth.type).toBe('bearer');
      expect(parsed.auth.bearer).toEqual([{ key: 'token', value: 'abc' }]);
    });

    it('omits auth when type is none', () => {
      const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
        auth: { type: 'none' },
      })));
      expect(parsed.auth).toBeUndefined();
    });

    it('omits auth when type is inherit', () => {
      const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
        auth: { type: 'inherit' },
      })));
      expect(parsed.auth).toBeUndefined();
    });

    it('exports collection-level scripts as events', () => {
      const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
        preRequestScript: 'console.log("pre");',
        postResponseScript: 'console.log("test");',
      })));
      expect(parsed.event).toHaveLength(2);
      expect(parsed.event[0].listen).toBe('prerequest');
      expect(parsed.event[1].listen).toBe('test');
    });

    it('omits events when no scripts', () => {
      const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection()));
      expect(parsed.event).toBeUndefined();
    });

    describe('request conversion', () => {
      it('exports a basic request', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ name: 'Get Users', method: 'GET', url: 'https://api.com/users' })],
        })));
        const item = parsed.item[0];
        expect(item.name).toBe('Get Users');
        expect(item.request.method).toBe('GET');
        expect(item.request.url.raw).toBe('https://api.com/users');
      });

      it('exports headers with disabled flag', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({
            headers: [
              { key: 'Accept', value: 'application/json', enabled: true },
              { key: 'X-Debug', value: 'true', enabled: false, description: 'debug' },
            ],
          })],
        })));
        const headers = parsed.item[0].request.header;
        expect(headers).toHaveLength(2);
        expect(headers[0].disabled).toBe(false);
        expect(headers[1].disabled).toBe(true);
        expect(headers[1].description).toBe('debug');
      });

      it('exports query params in URL and as query array', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({
            url: 'https://api.com/search',
            params: [
              { key: 'q', value: 'test', enabled: true },
              { key: 'page', value: '1', enabled: false },
            ],
          })],
        })));
        const req = parsed.item[0].request;
        expect(req.url.raw).toBe('https://api.com/search?q=test');
        expect(req.url.query).toHaveLength(2);
        expect(req.url.query[0].disabled).toBe(false);
        expect(req.url.query[1].disabled).toBe(true);
      });

      it('builds empty query string for no enabled params', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({
            url: 'https://api.com/test',
            params: [{ key: 'x', value: 'y', enabled: false }],
          })],
        })));
        expect(parsed.item[0].request.url.raw).toBe('https://api.com/test');
      });

      it('exports JSON body as raw with json language', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ body: { mode: 'json', raw: '{"key":"val"}' } })],
        })));
        const body = parsed.item[0].request.body;
        expect(body.mode).toBe('raw');
        expect(body.raw).toBe('{"key":"val"}');
        expect(body.options.raw.language).toBe('json');
      });

      it('exports XML body', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ body: { mode: 'xml', raw: '<root/>' } })],
        })));
        expect(parsed.item[0].request.body.options.raw.language).toBe('xml');
      });

      it('exports HTML body', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ body: { mode: 'html', raw: '<p>hi</p>' } })],
        })));
        expect(parsed.item[0].request.body.options.raw.language).toBe('html');
      });

      it('exports text body', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ body: { mode: 'text', raw: 'hello' } })],
        })));
        expect(parsed.item[0].request.body.options.raw.language).toBe('text');
      });

      it('exports urlencoded body', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({
            body: {
              mode: 'x-www-form-urlencoded',
              urlencoded: [
                { key: 'user', value: 'alice', enabled: true },
                { key: 'pass', value: 'secret', enabled: false, description: 'pw' },
              ],
            },
          })],
        })));
        const body = parsed.item[0].request.body;
        expect(body.mode).toBe('urlencoded');
        expect(body.urlencoded).toHaveLength(2);
        expect(body.urlencoded[0].disabled).toBe(false);
        expect(body.urlencoded[1].disabled).toBe(true);
      });

      it('exports form-data body', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({
            body: {
              mode: 'form-data',
              formData: [
                { key: 'file', value: '/path', type: 'file', enabled: true },
                { key: 'name', value: 'doc', type: 'text', enabled: true },
              ],
            },
          })],
        })));
        const body = parsed.item[0].request.body;
        expect(body.mode).toBe('formdata');
        expect(body.formdata).toHaveLength(2);
        expect(body.formdata[0].type).toBe('file');
      });

      it('exports graphql body', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({
            body: { mode: 'graphql', graphql: { query: '{ users { id } }', variables: '{}' } },
          })],
        })));
        const body = parsed.item[0].request.body;
        expect(body.mode).toBe('graphql');
        expect(body.graphql.query).toBe('{ users { id } }');
      });

      it('omits body for none mode', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest()],
        })));
        expect(parsed.item[0].request.body).toBeUndefined();
      });

      it('exports request-level auth (basic)', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ auth: { type: 'basic', username: 'admin', password: 'pass' } })],
        })));
        const auth = parsed.item[0].request.auth;
        expect(auth.type).toBe('basic');
        expect(auth.basic).toEqual([{ key: 'username', value: 'admin' }, { key: 'password', value: 'pass' }]);
      });

      it('omits request auth when type is inherit', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ auth: { type: 'inherit' } })],
        })));
        expect(parsed.item[0].request.auth).toBeUndefined();
      });

      it('omits request auth when type is none', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({ auth: { type: 'none' } })],
        })));
        expect(parsed.item[0].request.auth).toBeUndefined();
      });

      it('exports request scripts as events', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeRequest({
            preRequestScript: 'nx.environment.set("k", "v");',
            postResponseScript: 'nx.test("ok", () => {});',
          })],
        })));
        const events = parsed.item[0].event;
        expect(events).toHaveLength(2);
        expect(events[0].listen).toBe('prerequest');
        expect(events[0].script.type).toBe('text/javascript');
        expect(events[0].script.exec.join('\n')).toContain('pm.environment.set');
        expect(events[1].listen).toBe('test');
        expect(events[1].script.exec.join('\n')).toContain('pm.test');
      });
    });

    describe('folder conversion', () => {
      it('exports a folder with nested items', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeFolder({
            name: 'Users',
            items: [makeRequest({ name: 'Get Users' })],
          })],
        })));
        const folder = parsed.item[0];
        expect(folder.name).toBe('Users');
        expect(folder.item).toHaveLength(1);
        expect(folder.item[0].name).toBe('Get Users');
      });

      it('exports folder auth when not inherit', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeFolder({ auth: { type: 'bearer', token: 'tok' } })],
        })));
        expect(parsed.item[0].auth.type).toBe('bearer');
      });

      it('omits folder auth when inherit', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeFolder({ auth: { type: 'inherit' } })],
        })));
        expect(parsed.item[0].auth).toBeUndefined();
      });

      it('exports folder scripts', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeFolder({
            preRequestScript: 'console.log("pre");',
            postResponseScript: 'console.log("test");',
          })],
        })));
        expect(parsed.item[0].event).toHaveLength(2);
      });

      it('handles nested folders', () => {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({
          items: [makeFolder({
            name: 'API',
            items: [makeFolder({
              name: 'v1',
              items: [makeRequest({ name: 'Ping' })],
            })],
          })],
        })));
        expect(parsed.item[0].item[0].name).toBe('v1');
        expect(parsed.item[0].item[0].item[0].name).toBe('Ping');
      });
    });

    describe('auth type conversion', () => {
      function exportAuth(auth: NexusCollection['auth']) {
        const parsed = JSON.parse(exporter.exportAsPostmanV21(makeCollection({ auth })));
        return parsed.auth;
      }

      it('converts api-key auth', () => {
        const auth = exportAuth({ type: 'api-key', key: 'X-Key', value: 'secret', addTo: 'header' });
        expect(auth.type).toBe('apikey');
        expect(auth.apikey).toEqual([
          { key: 'key', value: 'X-Key' },
          { key: 'value', value: 'secret' },
          { key: 'in', value: 'header' },
        ]);
      });

      it('converts api-key auth with query location', () => {
        const auth = exportAuth({ type: 'api-key', key: 'api_key', value: 's', addTo: 'query' });
        expect(auth.apikey.find((kv: { key: string }) => kv.key === 'in').value).toBe('query');
      });

      it('converts oauth2 auth', () => {
        const auth = exportAuth({ type: 'oauth2', accessToken: 'tok', tokenType: 'Bearer', addTo: 'header' });
        expect(auth.type).toBe('oauth2');
        expect(auth.oauth2.find((kv: { key: string }) => kv.key === 'accessToken').value).toBe('tok');
      });

      it('converts oauth2 auth with query addTo', () => {
        const auth = exportAuth({ type: 'oauth2', accessToken: 't', addTo: 'query' });
        expect(auth.oauth2.find((kv: { key: string }) => kv.key === 'addTokenTo').value).toBe('queryParams');
      });

      it('converts digest auth', () => {
        const auth = exportAuth({ type: 'digest', username: 'u', password: 'p', realm: 'r', nonce: 'n' });
        expect(auth.type).toBe('digest');
        expect(auth.digest.find((kv: { key: string }) => kv.key === 'username').value).toBe('u');
      });

      it('converts aws-sig-v4 auth', () => {
        const auth = exportAuth({ type: 'aws-sig-v4', accessKey: 'AK', secretKey: 'SK', region: 'us-east-1', service: 's3' });
        expect(auth.type).toBe('awsv4');
        expect(auth.awsv4.find((kv: { key: string }) => kv.key === 'region').value).toBe('us-east-1');
      });

      it('defaults unknown types to noauth', () => {
        const auth = exportAuth({ type: 'hawk' as any });
        expect(auth.type).toBe('noauth');
      });
    });
  });

  describe('exportEnvironmentAsPostman', () => {
    it('exports a basic environment', () => {
      const env: NexusEnvironment = {
        id: 'env_1',
        name: 'Production',
        variables: [
          { key: 'host', value: 'api.prod.com', type: 'string', enabled: true },
          { key: 'token', value: 'secret-val', type: 'secret', enabled: true },
        ],
      };
      const parsed = JSON.parse(exporter.exportEnvironmentAsPostman(env));
      expect(parsed.name).toBe('Production');
      expect(parsed._postman_variable_scope).toBe('environment');
      expect(parsed.values).toHaveLength(2);
      expect(parsed.values[0]).toEqual({ key: 'host', value: 'api.prod.com', enabled: true, type: 'default' });
    });

    it('clears secret values in export', () => {
      const env: NexusEnvironment = {
        id: 'env_1',
        name: 'Env',
        variables: [{ key: 'secret', value: 'hidden', type: 'secret', enabled: true }],
      };
      const parsed = JSON.parse(exporter.exportEnvironmentAsPostman(env));
      expect(parsed.values[0].value).toBe('');
      expect(parsed.values[0].type).toBe('secret');
    });

    it('exports disabled variables', () => {
      const env: NexusEnvironment = {
        id: 'env_1',
        name: 'Env',
        variables: [{ key: 'off', value: 'val', type: 'string', enabled: false }],
      };
      const parsed = JSON.parse(exporter.exportEnvironmentAsPostman(env));
      expect(parsed.values[0].enabled).toBe(false);
    });

    it('handles empty variables', () => {
      const env: NexusEnvironment = { id: 'env_1', name: 'Empty', variables: [] };
      const parsed = JSON.parse(exporter.exportEnvironmentAsPostman(env));
      expect(parsed.values).toHaveLength(0);
    });
  });
});
