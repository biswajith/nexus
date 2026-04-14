import { describe, it, expect } from 'vitest';
import { generateCurl } from './curl-generator.js';
import type { NexusRequest } from '../types/index.js';

function makeRequest(overrides?: Partial<NexusRequest>): NexusRequest {
  return {
    id: 'req-1',
    name: 'Test Request',
    method: 'GET',
    url: 'https://api.example.com/data',
    headers: [],
    params: [],
    body: { mode: 'none' },
    auth: { type: 'none' },
    settings: {},
    ...overrides,
  };
}

describe('generateCurl', () => {
  describe('method and URL', () => {
    it('generates a basic GET request without -X flag', () => {
      const result = generateCurl(makeRequest());
      expect(result).toContain("curl");
      expect(result).toContain("'https://api.example.com/data'");
      expect(result).not.toContain('-X');
    });

    it('includes -X for non-GET methods', () => {
      const result = generateCurl(makeRequest({ method: 'POST' }));
      expect(result).toContain('-X POST');
    });

    it('includes -X for DELETE method', () => {
      const result = generateCurl(makeRequest({ method: 'DELETE' }));
      expect(result).toContain('-X DELETE');
    });
  });

  describe('query parameters', () => {
    it('appends enabled params to URL', () => {
      const result = generateCurl(makeRequest({
        params: [
          { key: 'page', value: '1', enabled: true },
          { key: 'limit', value: '10', enabled: true },
        ],
      }));
      expect(result).toContain('page=1');
      expect(result).toContain('limit=10');
    });

    it('skips disabled params', () => {
      const result = generateCurl(makeRequest({
        params: [
          { key: 'page', value: '1', enabled: true },
          { key: 'secret', value: 'hidden', enabled: false },
        ],
      }));
      expect(result).toContain('page=1');
      expect(result).not.toContain('secret');
    });

    it('skips params with empty keys', () => {
      const result = generateCurl(makeRequest({
        params: [
          { key: '', value: 'val', enabled: true },
        ],
      }));
      expect(result).not.toContain('=val');
    });

    it('appends with & when URL already has query params', () => {
      const result = generateCurl(makeRequest({
        url: 'https://api.example.com/data?existing=1',
        params: [{ key: 'extra', value: '2', enabled: true }],
      }));
      expect(result).toContain('existing=1&extra=2');
    });

    it('URL-encodes param keys and values', () => {
      const result = generateCurl(makeRequest({
        params: [{ key: 'q', value: 'hello world', enabled: true }],
      }));
      expect(result).toContain('hello%20world');
    });
  });

  describe('headers', () => {
    it('adds enabled headers', () => {
      const result = generateCurl(makeRequest({
        headers: [
          { key: 'Accept', value: 'application/json', enabled: true },
        ],
      }));
      expect(result).toContain("-H 'Accept: application/json'");
    });

    it('skips disabled headers', () => {
      const result = generateCurl(makeRequest({
        headers: [
          { key: 'Accept', value: 'application/json', enabled: false },
        ],
      }));
      expect(result).not.toContain('Accept');
    });

    it('skips headers with empty keys', () => {
      const result = generateCurl(makeRequest({
        headers: [
          { key: '', value: 'val', enabled: true },
        ],
      }));
      expect(result).not.toContain("-H '");
    });
  });

  describe('authentication', () => {
    it('adds Bearer auth header', () => {
      const result = generateCurl(makeRequest({
        auth: { type: 'bearer', token: 'my-token' },
      }));
      expect(result).toContain("-H 'Authorization: Bearer my-token'");
    });

    it('does not add bearer header when token is empty', () => {
      const result = generateCurl(makeRequest({
        auth: { type: 'bearer', token: '' },
      }));
      expect(result).not.toContain('Authorization');
    });

    it('adds basic auth with -u flag', () => {
      const result = generateCurl(makeRequest({
        auth: { type: 'basic', username: 'user', password: 'pass' },
      }));
      expect(result).toContain("-u 'user:pass'");
    });

    it('adds api-key header when addTo is "header"', () => {
      const result = generateCurl(makeRequest({
        auth: { type: 'api-key', key: 'X-Api-Key', value: 'abc123', addTo: 'header' },
      }));
      expect(result).toContain("-H 'X-Api-Key: abc123'");
    });

    it('does not add api-key header when key or value is empty', () => {
      const result = generateCurl(makeRequest({
        auth: { type: 'api-key', key: '', value: 'abc123', addTo: 'header' },
      }));
      expect(result).not.toContain('X-Api-Key');
    });

    it('does not add api-key when addTo is not "header"', () => {
      const result = generateCurl(makeRequest({
        auth: { type: 'api-key', key: 'X-Api-Key', value: 'abc', addTo: 'query' },
      }));
      expect(result).not.toContain("-H 'X-Api-Key");
    });
  });

  describe('body modes', () => {
    it('sends JSON body with Content-Type', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'json', raw: '{"key":"value"}' },
      }));
      expect(result).toContain("-H 'Content-Type: application/json'");
      expect(result).toContain("-d '{\"key\":\"value\"}'");
    });

    it('sends XML body with Content-Type', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'xml', raw: '<root/>' },
      }));
      expect(result).toContain("-H 'Content-Type: application/xml'");
      expect(result).toContain("-d '<root/>'");
    });

    it('sends HTML body with Content-Type', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'html', raw: '<h1>Hello</h1>' },
      }));
      expect(result).toContain("-H 'Content-Type: text/html'");
    });

    it('sends text body with Content-Type', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'text', raw: 'hello' },
      }));
      expect(result).toContain("-H 'Content-Type: text/plain'");
      expect(result).toContain("-d 'hello'");
    });

    it('does not add Content-Type if already present in headers', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        headers: [{ key: 'Content-Type', value: 'application/json; charset=utf-8', enabled: true }],
        body: { mode: 'json', raw: '{}' },
      }));
      const matches = result.match(/Content-Type/g);
      expect(matches).toHaveLength(1);
    });

    it('does not send body when raw is empty', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'json', raw: '' },
      }));
      expect(result).not.toContain("-d ");
    });

    it('escapes single quotes in body', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'json', raw: "it's a test" },
      }));
      expect(result).toContain("it'\\''s a test");
    });

    it('sends x-www-form-urlencoded entries', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: {
          mode: 'x-www-form-urlencoded',
          urlencoded: [
            { key: 'name', value: 'John', enabled: true },
            { key: 'disabled', value: 'skip', enabled: false },
          ],
        },
      }));
      expect(result).toContain("--data-urlencode 'name=John'");
      expect(result).not.toContain('disabled');
    });

    it('sends form-data text entries', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: {
          mode: 'form-data',
          formData: [
            { key: 'name', value: 'John', type: 'text', enabled: true },
          ],
        },
      }));
      expect(result).toContain("-F 'name=John'");
    });

    it('sends form-data file entries with @ prefix', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: {
          mode: 'form-data',
          formData: [
            { key: 'avatar', value: '/path/to/file.png', type: 'file', enabled: true },
          ],
        },
      }));
      expect(result).toContain("-F 'avatar=@/path/to/file.png'");
    });

    it('skips disabled form-data entries', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: {
          mode: 'form-data',
          formData: [
            { key: 'skip', value: 'me', type: 'text', enabled: false },
          ],
        },
      }));
      expect(result).not.toContain('-F');
    });

    it('sends binary body with --data-binary', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'binary', binary: { path: '/tmp/data.bin' } },
      }));
      expect(result).toContain("--data-binary '@/tmp/data.bin'");
    });

    it('does not send binary when path is missing', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'binary' },
      }));
      expect(result).not.toContain('--data-binary');
    });

    it('sends graphql body as JSON', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: {
          mode: 'graphql',
          graphql: { query: '{ users { id name } }' },
        },
      }));
      expect(result).toContain("-H 'Content-Type: application/json'");
      expect(result).toContain("-d ");
      expect(result).toContain('users');
    });

    it('includes graphql variables when provided', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: {
          mode: 'graphql',
          graphql: {
            query: 'query($id: ID!) { user(id: $id) { name } }',
            variables: '{"id":"123"}',
          },
        },
      }));
      expect(result).toContain('"variables"');
      expect(result).toContain('"id"');
    });

    it('does not send graphql body when query is empty', () => {
      const result = generateCurl(makeRequest({
        method: 'POST',
        body: { mode: 'graphql', graphql: { query: '' } },
      }));
      expect(result).not.toContain("-d ");
    });
  });

  describe('settings', () => {
    it('adds -L for follow redirects (default)', () => {
      const result = generateCurl(makeRequest());
      expect(result).toContain('-L');
    });

    it('omits -L when followRedirects is false', () => {
      const result = generateCurl(makeRequest({
        settings: { followRedirects: false },
      }));
      expect(result).not.toContain('-L');
    });

    it('adds -k when rejectUnauthorized is false', () => {
      const result = generateCurl(makeRequest({
        settings: { rejectUnauthorized: false },
      }));
      expect(result).toContain('-k');
    });

    it('does not add -k when rejectUnauthorized is not false', () => {
      const result = generateCurl(makeRequest());
      expect(result).not.toContain('-k');
    });

    it('adds --max-time converted from ms to seconds', () => {
      const result = generateCurl(makeRequest({
        settings: { timeout: 5000 },
      }));
      expect(result).toContain('--max-time 5');
    });

    it('rounds up timeout to next second', () => {
      const result = generateCurl(makeRequest({
        settings: { timeout: 1500 },
      }));
      expect(result).toContain('--max-time 2');
    });

    it('adds --max-redirs when set', () => {
      const result = generateCurl(makeRequest({
        settings: { maxRedirects: 3 },
      }));
      expect(result).toContain('--max-redirs 3');
    });
  });

  describe('output format', () => {
    it('joins parts with line continuation', () => {
      const result = generateCurl(makeRequest({ method: 'POST' }));
      expect(result).toContain(' \\\n  ');
    });
  });
});
