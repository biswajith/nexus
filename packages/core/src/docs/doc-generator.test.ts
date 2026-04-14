import { describe, it, expect } from 'vitest';
import { DocGenerator } from './doc-generator.js';
import type { NexusCollection, NexusRequest, NexusFolder } from '../types/index.js';

function makeRequest(overrides?: Partial<NexusRequest>): NexusRequest {
  return {
    id: 'req-1',
    name: 'Get Users',
    method: 'GET',
    url: 'https://api.example.com/users',
    headers: [],
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
    name: 'Auth',
    items: [],
    ...overrides,
  };
}

function makeCollection(overrides?: Partial<NexusCollection>): NexusCollection {
  return {
    id: 'coll-1',
    name: 'My API',
    variables: [],
    items: [],
    ...overrides,
  };
}

describe('DocGenerator', () => {
  const gen = new DocGenerator();

  describe('generateMarkdown', () => {
    it('includes the collection name as the title', () => {
      const md = gen.generateMarkdown(makeCollection({ name: 'Pet Store API' }));
      expect(md).toContain('# Pet Store API');
    });

    it('includes the collection description', () => {
      const md = gen.generateMarkdown(makeCollection({ description: 'A sample API for pets' }));
      expect(md).toContain('A sample API for pets');
    });

    it('includes base_url from variables', () => {
      const md = gen.generateMarkdown(makeCollection({
        variables: [{ key: 'base_url', value: 'https://api.pets.com', type: 'string', enabled: true }],
      }));
      expect(md).toContain('**Base URL:** `https://api.pets.com`');
    });

    it('omits base_url section when not in variables', () => {
      const md = gen.generateMarkdown(makeCollection({ variables: [] }));
      expect(md).not.toContain('Base URL');
    });

    it('groups top-level requests under "Requests"', () => {
      const coll = makeCollection({
        items: [makeRequest({ name: 'Health Check' })],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('## Requests');
      expect(md).toContain('### GET Health Check');
    });

    it('groups folder items under the folder name', () => {
      const coll = makeCollection({
        items: [
          makeFolder({
            name: 'Users',
            description: 'User management',
            items: [makeRequest({ name: 'List Users', method: 'GET', url: '/users' })],
          }),
        ],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('## Users');
      expect(md).toContain('User management');
      expect(md).toContain('### GET List Users');
    });

    it('builds a table of contents', () => {
      const coll = makeCollection({
        items: [
          makeFolder({
            name: 'Auth',
            items: [makeRequest({ name: 'Login', method: 'POST', url: '/login' })],
          }),
        ],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('## Table of Contents');
      expect(md).toContain('- [Auth](#auth)');
      expect(md).toContain('- [POST Login](#post-login)');
    });

    it('renders headers table for enabled headers', () => {
      const coll = makeCollection({
        items: [
          makeRequest({
            headers: [
              { key: 'Authorization', value: 'Bearer token', enabled: true, description: 'Auth header' },
              { key: 'X-Disabled', value: 'nope', enabled: false },
            ],
          }),
        ],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('**Headers:**');
      expect(md).toContain('`Authorization`');
      expect(md).toContain('Auth header');
      expect(md).not.toContain('X-Disabled');
    });

    it('renders query parameters table for enabled params', () => {
      const coll = makeCollection({
        items: [
          makeRequest({
            params: [
              { key: 'page', value: '1', enabled: true, description: 'Page number' },
              { key: 'skip', value: '0', enabled: false },
            ],
          }),
        ],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('**Query Parameters:**');
      expect(md).toContain('`page`');
      expect(md).toContain('Page number');
      expect(md).not.toContain('skip');
    });

    it('renders body example for JSON body', () => {
      const coll = makeCollection({
        items: [
          makeRequest({
            method: 'POST',
            body: { mode: 'json', raw: '{"name":"Alice"}' },
          }),
        ],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('**Body** (`json`)');
      expect(md).toContain('{"name":"Alice"}');
    });

    it('omits body section for mode none', () => {
      const coll = makeCollection({
        items: [makeRequest({ body: { mode: 'none' } })],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).not.toContain('**Body**');
    });

    it('renders graphql query as body example', () => {
      const coll = makeCollection({
        items: [
          makeRequest({
            method: 'POST',
            body: { mode: 'graphql', graphql: { query: '{ users { id } }' } },
          }),
        ],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('{ users { id } }');
    });

    it('includes a cURL snippet', () => {
      const coll = makeCollection({
        items: [makeRequest({ method: 'GET', url: 'https://api.example.com/users' })],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('**cURL:**');
      expect(md).toContain('curl');
    });

    it('flattens nested folders into endpoint list', () => {
      const coll = makeCollection({
        items: [
          makeFolder({
            name: 'V2',
            items: [
              makeFolder({
                id: 'inner',
                name: 'Nested',
                items: [makeRequest({ name: 'Deep Endpoint', url: '/deep' })],
              }),
            ],
          }),
        ],
      });
      const md = gen.generateMarkdown(coll);
      expect(md).toContain('## V2');
      expect(md).toContain('### GET Deep Endpoint');
    });

    it('includes footer with collection name', () => {
      const md = gen.generateMarkdown(makeCollection({ name: 'Awesome API' }));
      expect(md).toContain('*Generated from "Awesome API" by Nexus*');
    });

    it('handles empty collection', () => {
      const md = gen.generateMarkdown(makeCollection());
      expect(md).toContain('# My API');
      expect(md).not.toContain('## Requests');
    });
  });

  describe('generateHtml', () => {
    it('wraps output in a full HTML document', () => {
      const html = gen.generateHtml(makeCollection());
      expect(html).toContain('<!DOCTYPE html>');
      expect(html).toContain('</html>');
    });

    it('includes collection name in title and h1', () => {
      const html = gen.generateHtml(makeCollection({ name: 'Pet Store' }));
      expect(html).toContain('<title>Pet Store — API Documentation</title>');
      expect(html).toContain('<h1>Pet Store</h1>');
    });

    it('escapes HTML characters in collection name', () => {
      const html = gen.generateHtml(makeCollection({ name: 'A & B <script>' }));
      expect(html).toContain('A &amp; B &lt;script&gt;');
      expect(html).not.toContain('<script>');
    });

    it('includes description when present', () => {
      const html = gen.generateHtml(makeCollection({ description: 'My awesome API' }));
      expect(html).toContain('My awesome API');
    });

    it('includes base URL when variable exists', () => {
      const html = gen.generateHtml(makeCollection({
        variables: [{ key: 'base_url', value: 'https://api.test.com', type: 'string', enabled: true }],
      }));
      expect(html).toContain('https://api.test.com');
      expect(html).toContain('base-url');
    });

    it('renders endpoint with method badge', () => {
      const html = gen.generateHtml(makeCollection({
        items: [makeRequest({ method: 'POST', name: 'Create User' })],
      }));
      expect(html).toContain('method-POST');
      expect(html).toContain('POST');
    });

    it('renders headers table in endpoint', () => {
      const html = gen.generateHtml(makeCollection({
        items: [
          makeRequest({
            headers: [{ key: 'Accept', value: 'application/json', enabled: true }],
          }),
        ],
      }));
      expect(html).toContain('Headers');
      expect(html).toContain('Accept');
    });

    it('renders query params table in endpoint', () => {
      const html = gen.generateHtml(makeCollection({
        items: [
          makeRequest({
            params: [{ key: 'limit', value: '10', enabled: true }],
          }),
        ],
      }));
      expect(html).toContain('Query Parameters');
      expect(html).toContain('limit');
    });

    it('renders body example for non-none modes', () => {
      const html = gen.generateHtml(makeCollection({
        items: [
          makeRequest({
            method: 'POST',
            body: { mode: 'json', raw: '{"key":"val"}' },
          }),
        ],
      }));
      expect(html).toContain('Body (json)');
      expect(html).toContain('{&quot;key&quot;:&quot;val&quot;}');
    });

    it('renders cURL section', () => {
      const html = gen.generateHtml(makeCollection({
        items: [makeRequest()],
      }));
      expect(html).toContain('cURL');
      expect(html).toContain('curl');
    });

    it('includes a footer with the collection name', () => {
      const html = gen.generateHtml(makeCollection({ name: 'Test API' }));
      expect(html).toContain('Generated from "Test API" by Nexus');
    });

    it('renders table of contents with links', () => {
      const html = gen.generateHtml(makeCollection({
        items: [
          makeFolder({
            name: 'Auth',
            items: [makeRequest({ name: 'Login', method: 'POST', url: '/login' })],
          }),
        ],
      }));
      expect(html).toContain('Endpoints');
      expect(html).toContain('href="#post-login"');
    });

    it('handles empty collection', () => {
      const html = gen.generateHtml(makeCollection());
      expect(html).toContain('<h1>My API</h1>');
      expect(html).toContain('</html>');
    });
  });
});
