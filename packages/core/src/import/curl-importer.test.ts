import { describe, it, expect, beforeEach } from 'vitest';
import { CurlImporter } from './curl-importer.js';

describe('CurlImporter', () => {
  let importer: CurlImporter;

  beforeEach(() => {
    importer = new CurlImporter();
  });

  describe('basic GET requests', () => {
    it('parses a simple GET curl command', () => {
      const req = importer.import('curl https://api.example.com/users');
      expect(req.method).toBe('GET');
      expect(req.url).toBe('https://api.example.com/users');
      expect(req.headers).toHaveLength(0);
      expect(req.body.mode).toBe('none');
    });

    it('generates request id with req_ prefix', () => {
      const req = importer.import('curl https://api.example.com/test');
      expect(req.id).toMatch(/^req_/);
    });

    it('generates a name from method + pathname', () => {
      const req = importer.import('curl https://api.example.com/users');
      expect(req.name).toBe('GET /users');
    });

    it('sets auth to none', () => {
      const req = importer.import('curl https://api.example.com');
      expect(req.auth).toEqual({ type: 'none' });
    });

    it('handles URL without protocol by defaulting for name generation', () => {
      const req = importer.import('curl example.com/api');
      expect(req.url).toBe('example.com/api');
      expect(req.name).toContain('/api');
    });
  });

  describe('-X / --request flag', () => {
    it('parses -X POST', () => {
      const req = importer.import('curl -X POST https://api.example.com/users');
      expect(req.method).toBe('POST');
    });

    it('parses --request PUT', () => {
      const req = importer.import('curl --request PUT https://api.example.com/users/1');
      expect(req.method).toBe('PUT');
    });

    it('parses -X DELETE', () => {
      const req = importer.import('curl -X DELETE https://api.example.com/users/1');
      expect(req.method).toBe('DELETE');
    });

    it('uppercases the method', () => {
      const req = importer.import('curl -X patch https://api.example.com/users/1');
      expect(req.method).toBe('PATCH');
    });
  });

  describe('headers (-H / --header)', () => {
    it('parses a single header', () => {
      const req = importer.import('curl -H "Content-Type: application/json" https://api.example.com');
      expect(req.headers).toHaveLength(1);
      expect(req.headers[0]).toEqual({ key: 'Content-Type', value: 'application/json', enabled: true });
    });

    it('parses multiple headers', () => {
      const req = importer.import(
        'curl -H "Accept: application/json" -H "Authorization: Bearer tok123" https://api.example.com',
      );
      expect(req.headers).toHaveLength(2);
      expect(req.headers[0]!.key).toBe('Accept');
      expect(req.headers[1]!.key).toBe('Authorization');
    });

    it('handles --header long form', () => {
      const req = importer.import('curl --header "X-Custom: value" https://api.example.com');
      expect(req.headers[0]!.key).toBe('X-Custom');
      expect(req.headers[0]!.value).toBe('value');
    });

    it('handles header values with colons', () => {
      const req = importer.import('curl -H "X-URL: https://example.com" https://api.example.com');
      expect(req.headers[0]!.value).toBe('https://example.com');
    });

    it('ignores malformed headers without colon', () => {
      const req = importer.import('curl -H "NoColon" https://api.example.com');
      expect(req.headers).toHaveLength(0);
    });
  });

  describe('data flags (-d, --data, --data-raw, --data-binary)', () => {
    it('parses -d with JSON body and auto-detects JSON mode', () => {
      const req = importer.import('curl -d \'{"name":"alice"}\' https://api.example.com/users');
      expect(req.method).toBe('POST');
      expect(req.body.mode).toBe('json');
      expect(req.body.raw).toBe('{"name":"alice"}');
    });

    it('auto-detects JSON from content-type header', () => {
      const req = importer.import(
        'curl -H "Content-Type: application/json" -d \'data\' https://api.example.com',
      );
      expect(req.body.mode).toBe('json');
    });

    it('detects urlencoded from key=value body', () => {
      const req = importer.import('curl -d "user=alice&pass=secret" https://api.example.com/login');
      expect(req.method).toBe('POST');
      expect(req.body.mode).toBe('x-www-form-urlencoded');
      expect(req.body.urlencoded).toHaveLength(2);
      expect(req.body.urlencoded![0]!.key).toBe('user');
      expect(req.body.urlencoded![0]!.value).toBe('alice');
    });

    it('detects urlencoded from content-type header', () => {
      const req = importer.import(
        'curl -H "Content-Type: application/x-www-form-urlencoded" -d "a=1" https://api.example.com',
      );
      expect(req.body.mode).toBe('x-www-form-urlencoded');
    });

    it('falls back to text mode for non-JSON non-urlencoded data', () => {
      const req = importer.import(
        'curl -H "Content-Type: text/plain" -d "hello world" https://api.example.com',
      );
      expect(req.body.mode).toBe('text');
      expect(req.body.raw).toBe('hello world');
    });

    it('changes method to POST when -d is used with no explicit method', () => {
      const req = importer.import('curl -d \'{"key":"val"}\' https://api.example.com');
      expect(req.method).toBe('POST');
    });

    it('keeps explicit method even with -d', () => {
      const req = importer.import('curl -X PUT -d \'{"key":"val"}\' https://api.example.com');
      expect(req.method).toBe('PUT');
    });

    it('handles --data-raw flag', () => {
      const req = importer.import('curl --data-raw \'{"a":1}\' https://api.example.com');
      expect(req.body.mode).toBe('json');
    });

    it('handles --data-binary flag', () => {
      const req = importer.import('curl --data-binary \'{"b":2}\' https://api.example.com');
      expect(req.body.mode).toBe('json');
    });
  });

  describe('--data-urlencode', () => {
    it('parses --data-urlencode pairs into urlencoded body', () => {
      const req = importer.import(
        'curl --data-urlencode "name=Alice" --data-urlencode "city=New York" https://api.example.com',
      );
      expect(req.method).toBe('POST');
      expect(req.body.mode).toBe('x-www-form-urlencoded');
      expect(req.body.urlencoded).toHaveLength(2);
      expect(req.body.urlencoded![0]!.key).toBe('name');
      expect(req.body.urlencoded![0]!.value).toBe('Alice');
    });
  });

  describe('-F / --form (multipart)', () => {
    it('parses -F into form-data body', () => {
      const req = importer.import('curl -F "file=@/path/to/file" -F "name=doc" https://api.example.com/upload');
      expect(req.method).toBe('POST');
      expect(req.body.mode).toBe('form-data');
      expect(req.body.formData).toHaveLength(2);
      expect(req.body.formData![0]!.key).toBe('file');
      expect(req.body.formData![0]!.value).toBe('@/path/to/file');
      expect(req.body.formData![0]!.type).toBe('text');
    });

    it('handles --form long form', () => {
      const req = importer.import('curl --form "key=val" https://api.example.com');
      expect(req.body.mode).toBe('form-data');
    });
  });

  describe('authentication (-u / --user)', () => {
    it('converts -u to Basic auth header', () => {
      const req = importer.import('curl -u admin:password https://api.example.com');
      const authHeader = req.headers.find((h) => h.key === 'Authorization');
      expect(authHeader).toBeDefined();
      expect(authHeader!.value).toMatch(/^Basic /);
      const decoded = atob(authHeader!.value.replace('Basic ', ''));
      expect(decoded).toBe('admin:password');
    });

    it('handles --user long form', () => {
      const req = importer.import('curl --user user:pass https://api.example.com');
      const authHeader = req.headers.find((h) => h.key === 'Authorization');
      expect(authHeader).toBeDefined();
    });
  });

  describe('query parameters', () => {
    it('extracts query params from URL', () => {
      const req = importer.import('curl "https://api.example.com/search?q=hello&page=2"');
      expect(req.params).toHaveLength(2);
      expect(req.params[0]!.key).toBe('q');
      expect(req.params[0]!.value).toBe('hello');
      expect(req.params[1]!.key).toBe('page');
      expect(req.params[1]!.value).toBe('2');
      expect(req.url).toBe('https://api.example.com/search');
    });

    it('handles URL-encoded query params', () => {
      const req = importer.import('curl "https://api.example.com/search?q=hello%20world"');
      expect(req.params[0]!.value).toBe('hello world');
    });

    it('returns empty params when no query string', () => {
      const req = importer.import('curl https://api.example.com/data');
      expect(req.params).toHaveLength(0);
    });
  });

  describe('ignored flags', () => {
    it('ignores -k / --insecure', () => {
      const req = importer.import('curl -k https://api.example.com');
      expect(req.url).toBe('https://api.example.com');
    });

    it('ignores -L / --location', () => {
      const req = importer.import('curl -L https://api.example.com');
      expect(req.url).toBe('https://api.example.com');
    });

    it('ignores -v / --verbose', () => {
      const req = importer.import('curl -v https://api.example.com');
      expect(req.url).toBe('https://api.example.com');
    });

    it('ignores -s / --silent', () => {
      const req = importer.import('curl -s https://api.example.com');
      expect(req.url).toBe('https://api.example.com');
    });
  });

  describe('line continuation and quoting', () => {
    it('handles backslash-newline continuations', () => {
      const req = importer.import(
        'curl \\\n  -X POST \\\n  -H "Content-Type: application/json" \\\n  https://api.example.com',
      );
      expect(req.method).toBe('POST');
      expect(req.headers[0]!.key).toBe('Content-Type');
    });

    it('handles backslash-CRLF continuations', () => {
      const req = importer.import(
        'curl \\\r\n  -X POST \\\r\n  https://api.example.com',
      );
      expect(req.method).toBe('POST');
    });

    it('handles single-quoted arguments', () => {
      const req = importer.import("curl -H 'Accept: text/plain' https://api.example.com");
      expect(req.headers[0]!.key).toBe('Accept');
      expect(req.headers[0]!.value).toBe('text/plain');
    });

    it('handles double-quoted arguments', () => {
      const req = importer.import('curl -H "Accept: text/plain" https://api.example.com');
      expect(req.headers[0]!.key).toBe('Accept');
    });

    it('handles quoted URLs', () => {
      const req = importer.import("curl 'https://api.example.com/test'");
      expect(req.url).toBe('https://api.example.com/test');
    });
  });

  describe('complex real-world examples', () => {
    it('parses a full POST with headers, JSON body, and auth', () => {
      const req = importer.import(
        `curl -X POST \\
  -H "Content-Type: application/json" \\
  -H "Accept: application/json" \\
  -u admin:secret \\
  -d '{"name":"New User","email":"new@example.com"}' \\
  https://api.example.com/users`,
      );
      expect(req.method).toBe('POST');
      expect(req.url).toBe('https://api.example.com/users');
      expect(req.headers.length).toBeGreaterThanOrEqual(3);
      expect(req.body.mode).toBe('json');
      const bodyJson = JSON.parse(req.body.raw!);
      expect(bodyJson.name).toBe('New User');
    });

    it('parses a curl with multiple flags and query params', () => {
      const req = importer.import(
        'curl -s -L -k -X GET -H "Authorization: Bearer token123" "https://api.example.com/data?format=json&limit=100"',
      );
      expect(req.method).toBe('GET');
      expect(req.params).toHaveLength(2);
      expect(req.headers.some((h) => h.key === 'Authorization')).toBe(true);
    });
  });
});
