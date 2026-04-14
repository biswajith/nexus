import { describe, it, expect, beforeEach } from 'vitest';
import { OpenApiImporter } from './openapi-importer.js';

function makeOpenApi3Spec(overrides: Record<string, unknown> = {}) {
  return JSON.stringify({
    openapi: '3.0.3',
    info: { title: 'Test API', description: 'A test API', version: '1.0.0' },
    servers: [{ url: 'https://api.example.com/v1' }],
    paths: {},
    ...overrides,
  });
}

function makeSwagger2Spec(overrides: Record<string, unknown> = {}) {
  return JSON.stringify({
    swagger: '2.0',
    info: { title: 'Swagger API', version: '1.0.0' },
    host: 'api.example.com',
    basePath: '/v1',
    schemes: ['https'],
    paths: {},
    ...overrides,
  });
}

describe('OpenApiImporter', () => {
  let importer: OpenApiImporter;

  beforeEach(() => {
    importer = new OpenApiImporter();
  });

  describe('basic import', () => {
    it('returns one collection from an OpenAPI 3 spec', () => {
      const result = importer.import(makeOpenApi3Spec());
      expect(result.collections).toHaveLength(1);
      expect(result.collections[0]!.name).toBe('Test API');
      expect(result.collections[0]!.description).toBe('A test API');
    });

    it('generates collection id with col_ prefix', () => {
      const result = importer.import(makeOpenApi3Spec());
      expect(result.collections[0]!.id).toMatch(/^col_/);
    });

    it('sets base_url variable from first server', () => {
      const result = importer.import(makeOpenApi3Spec());
      const vars = result.collections[0]!.variables;
      expect(vars).toHaveLength(1);
      expect(vars[0]!.key).toBe('base_url');
      expect(vars[0]!.value).toBe('https://api.example.com/v1');
    });

    it('defaults to http://localhost when no servers', () => {
      const result = importer.import(makeOpenApi3Spec({ servers: undefined }));
      expect(result.collections[0]!.variables[0]!.value).toBe('http://localhost');
    });
  });

  describe('environments from multiple servers', () => {
    it('creates environments for each server', () => {
      const result = importer.import(makeOpenApi3Spec({
        servers: [
          { url: 'https://api.prod.com', description: 'Production' },
          { url: 'https://api.staging.com', description: 'Staging' },
          { url: 'https://api.dev.com' },
        ],
      }));
      expect(result.environments).toHaveLength(3);
      expect(result.environments[0]!.name).toBe('Production');
      expect(result.environments[0]!.variables[0]!.value).toBe('https://api.prod.com');
      expect(result.environments[1]!.name).toBe('Staging');
      expect(result.environments[2]!.name).toBe('Server 3');
    });

    it('returns no environments for single server', () => {
      const result = importer.import(makeOpenApi3Spec());
      expect(result.environments).toHaveLength(0);
    });

    it('returns no environments when servers is empty', () => {
      const result = importer.import(makeOpenApi3Spec({ servers: [] }));
      expect(result.environments).toHaveLength(0);
    });
  });

  describe('path and method parsing', () => {
    it('builds a request for each HTTP method on a path', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/users': {
            get: { summary: 'List users' },
            post: { summary: 'Create user' },
          },
        },
      }));
      const items = result.collections[0]!.items;
      expect(items.length).toBeGreaterThanOrEqual(2);
    });

    it('constructs URL with {{base_url}} prefix and :param substitution', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/users/{userId}/posts/{postId}': {
            get: { summary: 'Get post' },
          },
        },
      }));
      const items = result.collections[0]!.items;
      const req = (items[0] && 'url' in items[0]) ? items[0] : (items[0] as { items: Array<{ url: string }> }).items[0];
      expect((req as { url: string }).url).toBe('{{base_url}}/users/:userId/posts/:postId');
    });

    it('uppercases the method', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: { '/test': { delete: { summary: 'Delete' } } },
      }));
      const items = result.collections[0]!.items;
      const req = findFirstRequest(items);
      expect(req.method).toBe('DELETE');
    });

    it('uses summary as name, falling back to operationId, then method + path', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/a': { get: { summary: 'Get A' } },
          '/b': { get: { operationId: 'getB' } },
          '/c': { get: {} },
        },
      }));
      const names = getAllRequestNames(result.collections[0]!.items);
      expect(names).toContain('Get A');
      expect(names).toContain('getB');
      expect(names).toContain('GET /c');
    });

    it('prefixes deprecated operations with [DEPRECATED]', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: { '/old': { get: { summary: 'Old endpoint', deprecated: true } } },
      }));
      const names = getAllRequestNames(result.collections[0]!.items);
      expect(names.some((n) => n.startsWith('[DEPRECATED]'))).toBe(true);
    });

    it('skips non-HTTP method keys on path items', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: { '/x': { summary: 'not a method', get: { summary: 'Real' } } },
      }));
      const reqs = getAllRequests(result.collections[0]!.items);
      expect(reqs).toHaveLength(1);
    });
  });

  describe('parameters', () => {
    it('maps query parameters to request params', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/search': {
            get: {
              summary: 'Search',
              parameters: [
                { name: 'q', in: 'query', required: true, description: 'search term', example: 'hello' },
                { name: 'page', in: 'query', required: false, schema: { default: 1 } },
              ],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.params.length).toBe(2);
      expect(req.params[0]!.key).toBe('q');
      expect(req.params[0]!.value).toBe('hello');
      expect(req.params[0]!.enabled).toBe(true);
      expect(req.params[1]!.key).toBe('page');
      expect(req.params[1]!.value).toBe('1');
      expect(req.params[1]!.enabled).toBe(false);
    });

    it('maps header parameters to request headers', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/data': {
            get: {
              summary: 'Get data',
              parameters: [{ name: 'X-Request-Id', in: 'header', example: 'abc-123' }],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.headers.some((h: { key: string }) => h.key === 'X-Request-Id')).toBe(true);
    });

    it('merges path-level and operation-level parameters', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/items': {
            parameters: [{ name: 'X-Trace', in: 'header', example: 'trace-1' }],
            get: {
              summary: 'List items',
              parameters: [{ name: 'limit', in: 'query', example: '10' }],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.headers.some((h: { key: string }) => h.key === 'X-Trace')).toBe(true);
      expect(req.params.some((p: { key: string }) => p.key === 'limit')).toBe(true);
    });

    it('resolves example from schema.example', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            get: {
              summary: 'Test',
              parameters: [{ name: 'foo', in: 'query', schema: { example: 'bar' } }],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.params[0]!.value).toBe('bar');
    });

    it('resolves example from schema.enum', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            get: {
              summary: 'Test',
              parameters: [{ name: 'status', in: 'query', schema: { enum: ['active', 'inactive'] } }],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.params[0]!.value).toBe('active');
    });

    it('returns empty value when no example or default', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            get: {
              summary: 'Test',
              parameters: [{ name: 'x', in: 'query' }],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.params[0]!.value).toBe('');
    });
  });

  describe('request bodies (OpenAPI 3)', () => {
    it('creates JSON body from application/json content', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/users': {
            post: {
              summary: 'Create user',
              requestBody: {
                content: {
                  'application/json': {
                    schema: {
                      type: 'object',
                      properties: {
                        name: { type: 'string', example: 'Alice' },
                        age: { type: 'integer', example: 30 },
                      },
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('json');
      const parsed = JSON.parse(req.body.raw);
      expect(parsed.name).toBe('Alice');
      expect(parsed.age).toBe(30);
    });

    it('uses explicit example over schema generation', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/users': {
            post: {
              summary: 'Create user',
              requestBody: {
                content: {
                  'application/json': {
                    example: { custom: 'example' },
                    schema: { type: 'object', properties: { name: { type: 'string' } } },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      const parsed = JSON.parse(req.body.raw);
      expect(parsed.custom).toBe('example');
    });

    it('creates urlencoded body from application/x-www-form-urlencoded', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/login': {
            post: {
              summary: 'Login',
              requestBody: {
                content: {
                  'application/x-www-form-urlencoded': {
                    schema: {
                      type: 'object',
                      properties: {
                        username: { type: 'string', example: 'admin' },
                        password: { type: 'string' },
                      },
                      required: ['username'],
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('x-www-form-urlencoded');
      expect(req.body.urlencoded.length).toBe(2);
      expect(req.body.urlencoded[0]!.key).toBe('username');
      expect(req.body.urlencoded[0]!.enabled).toBe(true);
      expect(req.body.urlencoded[1]!.enabled).toBe(false);
    });

    it('creates form-data body from multipart/form-data', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/upload': {
            post: {
              summary: 'Upload',
              requestBody: {
                content: {
                  'multipart/form-data': {
                    schema: {
                      type: 'object',
                      properties: {
                        file: { type: 'string', format: 'binary' },
                        desc: { type: 'string', example: 'My file' },
                      },
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('form-data');
      expect(req.body.formData.length).toBe(2);
      expect(req.body.formData[0]!.type).toBe('text');
    });

    it('creates XML body for application/xml', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/xml': {
            post: {
              summary: 'Post XML',
              requestBody: { content: { 'application/xml': { schema: { type: 'object' } } } },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('xml');
    });

    it('falls back to text for unknown content type', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/bin': {
            post: {
              summary: 'Post binary',
              requestBody: { content: { 'application/octet-stream': {} } },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('text');
    });

    it('returns none body when no requestBody', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: { '/test': { get: { summary: 'Test' } } },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('none');
    });

    it('adds Content-Type header for JSON bodies', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/data': {
            post: {
              summary: 'Post',
              requestBody: {
                content: { 'application/json': { schema: { type: 'object' } } },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.headers.some((h: { key: string }) => h.key === 'Content-Type')).toBe(true);
    });
  });

  describe('tag-based folder grouping', () => {
    it('groups tagged operations into folders', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/users': { get: { summary: 'List users', tags: ['Users'] } },
          '/users/{id}': { get: { summary: 'Get user', tags: ['Users'] } },
          '/posts': { get: { summary: 'List posts', tags: ['Posts'] } },
        },
        tags: [
          { name: 'Users', description: 'User management' },
          { name: 'Posts', description: 'Blog posts' },
        ],
      }));
      const items = result.collections[0]!.items;
      const folders = items.filter((i) => 'items' in i);
      expect(folders.length).toBe(2);
      const usersFolder = folders.find((f) => f.name === 'Users')!;
      expect((usersFolder as { items: unknown[] }).items).toHaveLength(2);
      expect((usersFolder as { description?: string }).description).toBe('User management');
    });

    it('leaves untagged operations at top level', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/health': { get: { summary: 'Health check' } },
          '/users': { get: { summary: 'List users', tags: ['Users'] } },
        },
      }));
      const items = result.collections[0]!.items;
      const topLevelRequests = items.filter((i) => 'method' in i);
      expect(topLevelRequests.length).toBe(1);
      expect(topLevelRequests[0]!.name).toBe('Health check');
    });
  });

  describe('schema example generation', () => {
    it('generates examples for string formats', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            post: {
              summary: 'Test',
              requestBody: {
                content: {
                  'application/json': {
                    schema: {
                      type: 'object',
                      properties: {
                        email: { type: 'string', format: 'email' },
                        createdAt: { type: 'string', format: 'date-time' },
                        website: { type: 'string', format: 'uri' },
                        uid: { type: 'string', format: 'uuid' },
                        birthday: { type: 'string', format: 'date' },
                      },
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      const body = JSON.parse(req.body.raw);
      expect(body.email).toBe('user@example.com');
      expect(body.createdAt).toBe('2026-01-01T00:00:00Z');
      expect(body.website).toBe('https://example.com');
      expect(body.uid).toBe('550e8400-e29b-41d4-a716-446655440000');
      expect(body.birthday).toBe('2026-01-01');
    });

    it('generates examples for primitives', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            post: {
              summary: 'Test',
              requestBody: {
                content: {
                  'application/json': {
                    schema: {
                      type: 'object',
                      properties: {
                        count: { type: 'integer' },
                        ratio: { type: 'number' },
                        active: { type: 'boolean' },
                        label: { type: 'string' },
                      },
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      const body = JSON.parse(req.body.raw);
      expect(body.count).toBe(0);
      expect(body.ratio).toBe(0);
      expect(body.active).toBe(true);
      expect(body.label).toBe('string');
    });

    it('uses first enum value for strings', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            post: {
              summary: 'Test',
              requestBody: {
                content: {
                  'application/json': {
                    schema: {
                      type: 'object',
                      properties: {
                        status: { type: 'string', enum: ['active', 'inactive'] },
                      },
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      const body = JSON.parse(req.body.raw);
      expect(body.status).toBe('active');
    });

    it('generates array examples', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            post: {
              summary: 'Test',
              requestBody: {
                content: {
                  'application/json': {
                    schema: {
                      type: 'object',
                      properties: {
                        tags: { type: 'array', items: { type: 'string' } },
                        empty: { type: 'array' },
                      },
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      const body = JSON.parse(req.body.raw);
      expect(body.tags).toEqual(['string']);
      expect(body.empty).toEqual([]);
    });

    it('warns on unresolved $ref', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            post: {
              summary: 'Test',
              requestBody: {
                content: {
                  'application/json': {
                    schema: { $ref: '#/components/schemas/User' },
                  },
                },
              },
            },
          },
        },
      }));
      expect(result.warnings.some((w) => w.includes('$ref'))).toBe(true);
    });

    it('uses schema default values', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: {
          '/test': {
            post: {
              summary: 'Test',
              requestBody: {
                content: {
                  'application/json': {
                    schema: {
                      type: 'object',
                      properties: {
                        count: { type: 'integer', default: 42 },
                        name: { type: 'string', default: 'default-name' },
                      },
                    },
                  },
                },
              },
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      const body = JSON.parse(req.body.raw);
      expect(body.count).toBe(42);
      expect(body.name).toBe('default-name');
    });
  });

  describe('Swagger 2 support', () => {
    it('resolves base URL from host/schemes/basePath', () => {
      const result = importer.import(makeSwagger2Spec());
      expect(result.collections[0]!.variables[0]!.value).toBe('https://api.example.com/v1');
    });

    it('uses defaults for missing host/schemes', () => {
      const result = importer.import(makeSwagger2Spec({ host: undefined, schemes: undefined, basePath: undefined }));
      expect(result.collections[0]!.variables[0]!.value).toBe('https://localhost');
    });

    it('builds JSON body from body parameter', () => {
      const result = importer.import(makeSwagger2Spec({
        paths: {
          '/users': {
            post: {
              summary: 'Create user',
              parameters: [{
                name: 'body',
                in: 'body',
                schema: {
                  type: 'object',
                  properties: { name: { type: 'string', example: 'Bob' } },
                },
              }],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('json');
      const body = JSON.parse(req.body.raw);
      expect(body.name).toBe('Bob');
    });

    it('builds urlencoded body from formData parameters', () => {
      const result = importer.import(makeSwagger2Spec({
        paths: {
          '/login': {
            post: {
              summary: 'Login',
              parameters: [
                { name: 'username', in: 'formData', type: 'string', example: 'admin' },
                { name: 'password', in: 'formData', type: 'string' },
              ],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('x-www-form-urlencoded');
      expect(req.body.urlencoded[0]!.key).toBe('username');
    });

    it('builds form-data body when consumes includes multipart', () => {
      const result = importer.import(makeSwagger2Spec({
        paths: {
          '/upload': {
            post: {
              summary: 'Upload file',
              consumes: ['multipart/form-data'],
              parameters: [
                { name: 'file', in: 'formData', type: 'file' },
                { name: 'desc', in: 'formData', type: 'string', example: 'My file' },
              ],
            },
          },
        },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('form-data');
      expect(req.body.formData[0]!.type).toBe('file');
      expect(req.body.formData[1]!.type).toBe('text');
    });

    it('returns none body when no body/formData params', () => {
      const result = importer.import(makeSwagger2Spec({
        paths: { '/test': { get: { summary: 'Test' } } },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.body.mode).toBe('none');
    });
  });

  describe('YAML support', () => {
    it('parses a YAML OpenAPI spec', () => {
      const yaml = `
openapi: "3.0.0"
info:
  title: YAML API
  version: "1.0"
paths:
  /ping:
    get:
      summary: Ping
`;
      const result = importer.import(yaml);
      expect(result.collections[0]!.name).toBe('YAML API');
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.name).toBe('Ping');
    });
  });

  describe('edge cases', () => {
    it('handles empty paths', () => {
      const result = importer.import(makeOpenApi3Spec({ paths: {} }));
      expect(result.collections[0]!.items).toHaveLength(0);
    });

    it('handles missing paths key', () => {
      const result = importer.import(makeOpenApi3Spec({ paths: undefined }));
      expect(result.collections[0]!.items).toHaveLength(0);
    });

    it('all requests have auth type inherit', () => {
      const result = importer.import(makeOpenApi3Spec({
        paths: { '/test': { get: { summary: 'Test' } } },
      }));
      const req = findFirstRequest(result.collections[0]!.items);
      expect(req.auth.type).toBe('inherit');
    });
  });
});

// --- helpers ---

function findFirstRequest(items: unknown[]): Record<string, any> {
  for (const item of items) {
    if (item && typeof item === 'object' && 'method' in item) return item as Record<string, any>;
    if (item && typeof item === 'object' && 'items' in item) {
      const found = findFirstRequest((item as { items: unknown[] }).items);
      if (found) return found;
    }
  }
  throw new Error('No request found');
}

function getAllRequests(items: unknown[]): Record<string, any>[] {
  const result: Record<string, any>[] = [];
  for (const item of items) {
    if (item && typeof item === 'object' && 'method' in item) result.push(item as Record<string, any>);
    if (item && typeof item === 'object' && 'items' in item) {
      result.push(...getAllRequests((item as { items: unknown[] }).items));
    }
  }
  return result;
}

function getAllRequestNames(items: unknown[]): string[] {
  return getAllRequests(items).map((r) => r.name as string);
}
