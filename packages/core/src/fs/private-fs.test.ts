import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { ensurePrivateDir, writePrivateFile, appendPrivateFile, isSafeFileId } from './private-fs.js';
import { TokenStore } from '../auth/token-store.js';
import { HistoryManager } from '../history/history-manager.js';
import { CollectionManager } from '../collections/collection-manager.js';

const mode = async (p: string) => (await fs.stat(p)).mode & 0o777;

describe.skipIf(process.platform === 'win32')('private files', () => {
  let tmpDir: string;
  let oldUmask: number;

  beforeEach(async () => {
    // A permissive umask makes the test fail if a write relies on the default mode.
    oldUmask = process.umask(0o000);
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'nexus-private-'));
  });

  afterEach(async () => {
    process.umask(oldUmask);
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  it('creates files with mode 0600', async () => {
    const file = path.join(tmpDir, 'a.json');
    await writePrivateFile(file, '{}');
    expect(await mode(file)).toBe(0o600);
  });

  it('tightens an existing world-readable file on write', async () => {
    const file = path.join(tmpDir, 'old.json');
    await fs.writeFile(file, '{}', { mode: 0o644 });
    await writePrivateFile(file, '{"a":1}');
    expect(await mode(file)).toBe(0o600);
    expect(await fs.readFile(file, 'utf-8')).toBe('{"a":1}');
  });

  it('appends with mode 0600', async () => {
    const file = path.join(tmpDir, 'log.jsonl');
    await appendPrivateFile(file, 'a\n');
    await appendPrivateFile(file, 'b\n');
    expect(await fs.readFile(file, 'utf-8')).toBe('a\nb\n');
    expect(await mode(file)).toBe(0o600);
  });

  it('creates and tightens directories to 0700', async () => {
    const dir = path.join(tmpDir, 'd');
    await fs.mkdir(dir, { mode: 0o755 });
    await ensurePrivateDir(dir);
    expect(await mode(dir)).toBe(0o700);
  });

  it('refuses to replace with the wx flag', async () => {
    const file = path.join(tmpDir, 'x.json');
    await writePrivateFile(file, '1', 'wx');
    await expect(writePrivateFile(file, '2', 'wx')).rejects.toMatchObject({ code: 'EEXIST' });
  });

  it('stores tokens, history and collections as 0600', async () => {
    const tokens = new TokenStore(path.join(tmpDir, 'tokens.json'));
    await tokens.saveToken('k', { accessToken: 'a', tokenType: 'Bearer', expiresAt: 0 });
    expect(await mode(path.join(tmpDir, 'tokens.json'))).toBe(0o600);

    const history = new HistoryManager(path.join(tmpDir, 'history'));
    await history.log({ timestamp: Date.now() } as Parameters<HistoryManager['log']>[0]);
    const [historyFile] = await fs.readdir(path.join(tmpDir, 'history'));
    expect(await mode(path.join(tmpDir, 'history', historyFile!))).toBe(0o600);
    expect(await mode(path.join(tmpDir, 'history'))).toBe(0o700);

    const collections = new CollectionManager(path.join(tmpDir, 'collections'));
    const dir = await collections.createCollection({ id: 'c', name: 'C', variables: [], items: [] });
    expect(await mode(path.join(tmpDir, 'collections', dir, 'collection.json'))).toBe(0o600);
  });
});

describe('isSafeFileId', () => {
  it.each(['env_1', 'abc-DEF_123', 'a'])('accepts %s', (id) => {
    expect(isSafeFileId(id)).toBe(true);
  });

  it.each(['', '../x', 'a/b', 'a\\b', '..', 'a.json', 'x'.repeat(129), 42, null])('rejects %s', (id) => {
    expect(isSafeFileId(id)).toBe(false);
  });
});
