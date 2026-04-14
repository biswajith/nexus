import { describe, it, expect, afterEach } from 'vitest';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { TokenStore } from './token-store.js';
import type { StoredToken } from './token-store.js';

function makeToken(overrides?: Partial<StoredToken>): StoredToken {
  return {
    accessToken: 'access-123',
    tokenType: 'Bearer',
    expiresAt: Date.now() + 3600_000,
    refreshToken: 'refresh-456',
    scope: 'read write',
    ...overrides,
  };
}

describe('TokenStore (in-memory, no file)', () => {
  it('returns null for a key that was never saved', async () => {
    const store = new TokenStore();
    expect(await store.getToken('missing')).toBeNull();
  });

  it('saves and retrieves a token', async () => {
    const store = new TokenStore();
    const token = makeToken();
    await store.saveToken('key1', token);
    expect(await store.getToken('key1')).toEqual(token);
  });

  it('overwrites a token with the same key', async () => {
    const store = new TokenStore();
    await store.saveToken('k', makeToken({ accessToken: 'first' }));
    await store.saveToken('k', makeToken({ accessToken: 'second' }));
    const retrieved = await store.getToken('k');
    expect(retrieved!.accessToken).toBe('second');
  });

  it('removes a token', async () => {
    const store = new TokenStore();
    await store.saveToken('k', makeToken());
    await store.removeToken('k');
    expect(await store.getToken('k')).toBeNull();
  });

  it('removing a non-existent key does not throw', async () => {
    const store = new TokenStore();
    await expect(store.removeToken('nope')).resolves.toBeUndefined();
  });

  it('clears all tokens', async () => {
    const store = new TokenStore();
    await store.saveToken('a', makeToken());
    await store.saveToken('b', makeToken());
    await store.clear();
    expect(await store.getToken('a')).toBeNull();
    expect(await store.getToken('b')).toBeNull();
  });
});

describe('TokenStore.isExpired', () => {
  it('returns false for a token expiring well in the future', () => {
    const store = new TokenStore();
    const token = makeToken({ expiresAt: Date.now() + 3600_000 });
    expect(store.isExpired(token)).toBe(false);
  });

  it('returns true for a token already past expiry', () => {
    const store = new TokenStore();
    const token = makeToken({ expiresAt: Date.now() - 1000 });
    expect(store.isExpired(token)).toBe(true);
  });

  it('returns true within the 30-second buffer window', () => {
    const store = new TokenStore();
    const token = makeToken({ expiresAt: Date.now() + 20_000 });
    expect(store.isExpired(token)).toBe(true);
  });

  it('returns false at exactly 30 seconds before expiry', () => {
    const store = new TokenStore();
    const token = makeToken({ expiresAt: Date.now() + 60_000 });
    expect(store.isExpired(token)).toBe(false);
  });
});

describe('TokenStore (with file persistence)', () => {
  let tmpDir: string;

  afterEach(async () => {
    if (tmpDir) {
      await fs.rm(tmpDir, { recursive: true, force: true });
    }
  });

  async function makeTmpPath(): Promise<string> {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'token-store-test-'));
    return path.join(tmpDir, 'tokens.json');
  }

  it('persists tokens to disk on save', async () => {
    const filePath = await makeTmpPath();
    const store = new TokenStore(filePath);
    const token = makeToken();
    await store.saveToken('key1', token);

    const data = JSON.parse(await fs.readFile(filePath, 'utf-8'));
    expect(data.key1).toEqual(token);
  });

  it('loads tokens from an existing file', async () => {
    const filePath = await makeTmpPath();
    const token = makeToken();
    await fs.writeFile(filePath, JSON.stringify({ mykey: token }));

    const store = new TokenStore(filePath);
    await store.load();
    expect(await store.getToken('mykey')).toEqual(token);
  });

  it('starts empty when the file does not exist', async () => {
    const filePath = await makeTmpPath();
    const store = new TokenStore(filePath);
    await store.load();
    expect(await store.getToken('any')).toBeNull();
  });

  it('starts empty when the file contains invalid JSON', async () => {
    const filePath = await makeTmpPath();
    await fs.writeFile(filePath, 'not valid json!!!');

    const store = new TokenStore(filePath);
    await store.load();
    expect(await store.getToken('any')).toBeNull();
  });

  it('updates the file when a token is removed', async () => {
    const filePath = await makeTmpPath();
    const store = new TokenStore(filePath);
    await store.saveToken('a', makeToken());
    await store.saveToken('b', makeToken());
    await store.removeToken('a');

    const data = JSON.parse(await fs.readFile(filePath, 'utf-8'));
    expect(data.a).toBeUndefined();
    expect(data.b).toBeDefined();
  });

  it('writes empty JSON object on clear', async () => {
    const filePath = await makeTmpPath();
    const store = new TokenStore(filePath);
    await store.saveToken('a', makeToken());
    await store.clear();

    const data = JSON.parse(await fs.readFile(filePath, 'utf-8'));
    expect(data).toEqual({});
  });

  it('load() is a no-op when no filePath is configured', async () => {
    const store = new TokenStore();
    await expect(store.load()).resolves.toBeUndefined();
  });
});
