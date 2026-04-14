import { describe, it, expect, afterEach } from 'vitest';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { NexusCookieJar } from './cookie-jar.js';

describe('NexusCookieJar', () => {
  let tmpDir: string | undefined;

  afterEach(async () => {
    if (tmpDir) {
      await fs.rm(tmpDir, { recursive: true, force: true });
      tmpDir = undefined;
    }
  });

  async function makeTmpPath(): Promise<string> {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'cookie-jar-test-'));
    return path.join(tmpDir, 'cookies.json');
  }

  describe('setCookie / getCookies', () => {
    it('stores and retrieves a cookie for a URL', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('session=abc123; Path=/', 'https://example.com/');
      const cookies = await jar.getCookies('https://example.com/');
      expect(cookies).toHaveLength(1);
      expect(cookies[0]!.key).toBe('session');
      expect(cookies[0]!.value).toBe('abc123');
    });

    it('stores multiple cookies for the same domain', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.setCookie('b=2; Path=/', 'https://example.com/');
      const cookies = await jar.getCookies('https://example.com/');
      expect(cookies).toHaveLength(2);
    });

    it('does not return cookies for a different domain', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      const cookies = await jar.getCookies('https://other.com/');
      expect(cookies).toHaveLength(0);
    });
  });

  describe('getCookieString', () => {
    it('returns semicolon-separated cookie string', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.setCookie('b=2; Path=/', 'https://example.com/');
      const str = await jar.getCookieString('https://example.com/');
      expect(str).toContain('a=1');
      expect(str).toContain('b=2');
    });

    it('returns empty string when no cookies match', async () => {
      const jar = new NexusCookieJar();
      const str = await jar.getCookieString('https://example.com/');
      expect(str).toBe('');
    });
  });

  describe('getAllCookies', () => {
    it('returns all cookies across domains', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.setCookie('b=2; Path=/', 'https://other.com/');
      const all = await jar.getAllCookies();
      expect(all).toHaveLength(2);
    });

    it('returns empty array for a fresh jar', async () => {
      const jar = new NexusCookieJar();
      const all = await jar.getAllCookies();
      expect(all).toHaveLength(0);
    });
  });

  describe('removeCookie', () => {
    it('removes a specific cookie by domain, path, and key', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.setCookie('b=2; Path=/', 'https://example.com/');

      await jar.removeCookie('example.com', '/', 'a');
      const cookies = await jar.getCookies('https://example.com/');
      expect(cookies).toHaveLength(1);
      expect(cookies[0]!.key).toBe('b');
    });

    it('leaves other domains untouched', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.setCookie('b=2; Path=/', 'https://other.com/');

      await jar.removeCookie('example.com', '/', 'a');
      const remaining = await jar.getAllCookies();
      expect(remaining).toHaveLength(1);
      expect(remaining[0]!.key).toBe('b');
    });
  });

  describe('removeAllCookies', () => {
    it('clears every cookie from the jar', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.setCookie('b=2; Path=/', 'https://other.com/');
      await jar.removeAllCookies();

      const all = await jar.getAllCookies();
      expect(all).toHaveLength(0);
    });
  });

  describe('applyToRequest', () => {
    it('sets cookie header on the headers object', async () => {
      const jar = new NexusCookieJar();
      await jar.setCookie('token=xyz; Path=/', 'https://api.example.com/');

      const headers: Record<string, string> = { 'Accept': 'application/json' };
      const result = await jar.applyToRequest('https://api.example.com/data', headers);
      expect(result['cookie']).toContain('token=xyz');
      expect(result['Accept']).toBe('application/json');
    });

    it('does not set cookie header when no cookies match', async () => {
      const jar = new NexusCookieJar();
      const headers: Record<string, string> = {};
      const result = await jar.applyToRequest('https://api.example.com/', headers);
      expect(result['cookie']).toBeUndefined();
    });

    it('returns the same headers object (mutation)', async () => {
      const jar = new NexusCookieJar();
      const headers: Record<string, string> = {};
      const result = await jar.applyToRequest('https://example.com/', headers);
      expect(result).toBe(headers);
    });
  });

  describe('processResponse', () => {
    it('stores cookies from a single set-cookie string header', async () => {
      const jar = new NexusCookieJar();
      await jar.processResponse('https://example.com/', {
        'set-cookie': 'session=abc; Path=/',
      });
      const cookies = await jar.getCookies('https://example.com/');
      expect(cookies).toHaveLength(1);
      expect(cookies[0]!.key).toBe('session');
    });

    it('stores cookies from an array of set-cookie headers', async () => {
      const jar = new NexusCookieJar();
      await jar.processResponse('https://example.com/', {
        'set-cookie': ['a=1; Path=/', 'b=2; Path=/'],
      });
      const cookies = await jar.getCookies('https://example.com/');
      expect(cookies).toHaveLength(2);
    });

    it('does nothing when set-cookie is absent', async () => {
      const jar = new NexusCookieJar();
      await jar.processResponse('https://example.com/', {
        'content-type': 'text/html',
      });
      const cookies = await jar.getAllCookies();
      expect(cookies).toHaveLength(0);
    });
  });

  describe('persistence', () => {
    it('persists cookies to disk on setCookie', async () => {
      const filePath = await makeTmpPath();
      const jar = new NexusCookieJar(filePath);
      await jar.setCookie('a=1; Path=/', 'https://example.com/');

      const data = await fs.readFile(filePath, 'utf-8');
      const parsed = JSON.parse(data);
      expect(parsed.cookies).toBeDefined();
      expect(parsed.cookies.length).toBeGreaterThan(0);
    });

    it('loads cookies from disk', async () => {
      const filePath = await makeTmpPath();

      const jar1 = new NexusCookieJar(filePath);
      await jar1.setCookie('token=xyz; Path=/', 'https://example.com/');

      const jar2 = new NexusCookieJar(filePath);
      await jar2.load();
      const cookies = await jar2.getCookies('https://example.com/');
      expect(cookies).toHaveLength(1);
      expect(cookies[0]!.value).toBe('xyz');
    });

    it('starts empty when persist file does not exist', async () => {
      const filePath = await makeTmpPath();
      const jar = new NexusCookieJar(filePath);
      await jar.load();
      const cookies = await jar.getAllCookies();
      expect(cookies).toHaveLength(0);
    });

    it('starts empty when persist file has invalid JSON', async () => {
      const filePath = await makeTmpPath();
      await fs.writeFile(filePath, 'not json!');
      const jar = new NexusCookieJar(filePath);
      await jar.load();
      const cookies = await jar.getAllCookies();
      expect(cookies).toHaveLength(0);
    });

    it('updates persist file on removeAllCookies', async () => {
      const filePath = await makeTmpPath();
      const jar = new NexusCookieJar(filePath);
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.removeAllCookies();

      const data = JSON.parse(await fs.readFile(filePath, 'utf-8'));
      expect(data.cookies).toHaveLength(0);
    });

    it('updates persist file on removeCookie', async () => {
      const filePath = await makeTmpPath();
      const jar = new NexusCookieJar(filePath);
      await jar.setCookie('a=1; Path=/', 'https://example.com/');
      await jar.setCookie('b=2; Path=/', 'https://example.com/');
      await jar.removeCookie('example.com', '/', 'a');

      const jar2 = new NexusCookieJar(filePath);
      await jar2.load();
      const cookies = await jar2.getCookies('https://example.com/');
      expect(cookies).toHaveLength(1);
      expect(cookies[0]!.key).toBe('b');
    });

    it('load() is a no-op when no persistPath is configured', async () => {
      const jar = new NexusCookieJar();
      await expect(jar.load()).resolves.toBeUndefined();
    });
  });
});
