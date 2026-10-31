import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { CollectionManager } from './collection-manager.js';
import type { NexusCollection, NexusRequest, NexusFolder } from '../types/index.js';

function makeRequest(id: string, name: string): NexusRequest {
  return {
    id,
    name,
    method: 'GET',
    url: `https://api.example.com/${name}`,
    headers: [],
    params: [],
    body: { mode: 'none' },
    auth: { type: 'none' },
    settings: {},
  };
}

function makeFolder(id: string, name: string, items: (NexusRequest | NexusFolder)[]): NexusFolder {
  return { id, name, items };
}

describe('CollectionManager — item ordering (issue #1)', () => {
  let tmpDir: string;
  let manager: CollectionManager;

  beforeEach(async () => {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'nexus-test-'));
    manager = new CollectionManager(tmpDir);
  });

  afterEach(async () => {
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  it('preserves top-level request order through create→load round-trip', async () => {
    const r1 = makeRequest('req_aaa', 'third-request');
    const r2 = makeRequest('req_bbb', 'first-request');
    const r3 = makeRequest('req_ccc', 'second-request');

    const collection: NexusCollection = {
      id: 'col_order',
      name: 'Order Test',
      variables: [],
      items: [r1, r2, r3],
    };

    await manager.createCollection(collection);

    const dirEntries = await fs.readdir(tmpDir);
    const slug = dirEntries[0]!;
    const loaded = await manager.loadCollection(slug);

    expect(loaded.items.map((i) => i.id)).toEqual(['req_aaa', 'req_bbb', 'req_ccc']);
  });

  it('preserves folder item order through create→load round-trip', async () => {
    const r1 = makeRequest('req_f1', 'zebra');
    const r2 = makeRequest('req_f2', 'alpha');
    const r3 = makeRequest('req_f3', 'middle');
    const folder = makeFolder('folder_abc', 'My Folder', [r1, r2, r3]);

    const collection: NexusCollection = {
      id: 'col_folder_order',
      name: 'Folder Order Test',
      variables: [],
      items: [folder],
    };

    await manager.createCollection(collection);

    const dirEntries = await fs.readdir(tmpDir);
    const slug = dirEntries[0]!;
    const loaded = await manager.loadCollection(slug);

    const loadedFolder = loaded.items[0] as NexusFolder;
    expect(loadedFolder.items.map((i) => i.id)).toEqual(['req_f1', 'req_f2', 'req_f3']);
  });

  it('preserves mixed folders and requests in order', async () => {
    const r1 = makeRequest('req_1', 'first');
    const folder = makeFolder('folder_1', 'middle-folder', [
      makeRequest('req_inner', 'inner'),
    ]);
    const r2 = makeRequest('req_2', 'last');

    const collection: NexusCollection = {
      id: 'col_mixed',
      name: 'Mixed Order Test',
      variables: [],
      items: [r1, folder, r2],
    };

    await manager.createCollection(collection);

    const dirEntries = await fs.readdir(tmpDir);
    const slug = dirEntries[0]!;
    const loaded = await manager.loadCollection(slug);

    expect(loaded.items.map((i) => i.id)).toEqual(['req_1', 'folder_1', 'req_2']);
  });

  it('preserves deeply nested folder order', async () => {
    const inner1 = makeRequest('req_deep1', 'deep-first');
    const inner2 = makeRequest('req_deep2', 'deep-second');
    const childFolder = makeFolder('folder_child', 'Child', [inner1, inner2]);
    const parentFolder = makeFolder('folder_parent', 'Parent', [
      makeRequest('req_top', 'top-of-parent'),
      childFolder,
    ]);

    const collection: NexusCollection = {
      id: 'col_nested',
      name: 'Nested Order Test',
      variables: [],
      items: [parentFolder],
    };

    await manager.createCollection(collection);

    const dirEntries = await fs.readdir(tmpDir);
    const slug = dirEntries[0]!;
    const loaded = await manager.loadCollection(slug);

    const loadedParent = loaded.items[0] as NexusFolder;
    expect(loadedParent.items.map((i) => i.id)).toEqual(['req_top', 'folder_child']);

    const loadedChild = loadedParent.items[1] as NexusFolder;
    expect(loadedChild.items.map((i) => i.id)).toEqual(['req_deep1', 'req_deep2']);
  });

  it('appends items not in itemOrder at the end', async () => {
    const collection: NexusCollection = {
      id: 'col_extra',
      name: 'Extra Items Test',
      variables: [],
      items: [makeRequest('req_a', 'a'), makeRequest('req_b', 'b')],
    };

    await manager.createCollection(collection);

    const dirEntries = await fs.readdir(tmpDir);
    const slug = dirEntries[0]!;

    // Manually add a request file that isn't in itemOrder
    const extraReq = makeRequest('req_extra', 'extra');
    await manager.addRequest(slug, extraReq);

    const loaded = await manager.loadCollection(slug);
    const ids = loaded.items.map((i) => i.id);

    expect(ids.indexOf('req_a')).toBeLessThan(ids.indexOf('req_b'));
    expect(ids).toContain('req_extra');
    expect(ids.indexOf('req_extra')).toBe(ids.length - 1);
  });
});

describe('CollectionManager — name collisions never overwrite', () => {
  let tmpDir: string;
  let manager: CollectionManager;

  beforeEach(async () => {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'nexus-test-'));
    manager = new CollectionManager(tmpDir);
  });

  afterEach(async () => {
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  it('writes an imported collection with a colliding slug to a new directory', async () => {
    const mine: NexusCollection = {
      id: 'col_mine',
      name: 'Payments API',
      variables: [{ key: 'baseUrl', value: 'https://pay.internal', type: 'string', enabled: true }],
      preRequestScript: '',
      items: [makeRequest('req_mine', 'charge')],
    };
    const imported: NexusCollection = {
      id: 'col_evil',
      name: 'payments  API!!',
      variables: [{ key: 'baseUrl', value: 'https://attacker.test', type: 'string', enabled: true }],
      preRequestScript: 'console.log("attacker")',
      items: [makeRequest('req_evil', 'charge')],
    };

    const first = await manager.createCollection(mine);
    const second = await manager.createCollection(imported);

    expect(first).toBe('payments-api');
    expect(second).toBe('payments-api-2');

    const original = await manager.loadCollection(first);
    expect(original.id).toBe('col_mine');
    expect(original.variables[0]?.value).toBe('https://pay.internal');
    expect(original.preRequestScript).toBe('');
    expect(original.items.map((i) => i.id)).toEqual(['req_mine']);

    const copy = await manager.loadCollection(second);
    expect(copy.id).toBe('col_evil');
    expect(copy.items.map((i) => i.id)).toEqual(['req_evil']);
  });

  it('keeps both requests when two share a name', async () => {
    const dir = await manager.createCollection({
      id: 'col_dup',
      name: 'Dup',
      variables: [],
      items: [makeRequest('req_1', 'Get User'), makeRequest('req_2', 'get-user')],
    });
    const loaded = await manager.loadCollection(dir);
    expect(loaded.items.map((i) => i.id).sort()).toEqual(['req_1', 'req_2']);
  });

  it('keeps both folders when two share a name', async () => {
    const dir = await manager.createCollection({
      id: 'col_dupf',
      name: 'Dup folders',
      variables: [],
      items: [
        makeFolder('fld_1', 'Users', [makeRequest('req_a', 'a')]),
        makeFolder('fld_2', 'users', [makeRequest('req_b', 'b')]),
      ],
    });
    const loaded = await manager.loadCollection(dir);
    expect(loaded.items.map((i) => i.id).sort()).toEqual(['fld_1', 'fld_2']);
  });

  it('does not write into the base directory when the name has no slug characters', async () => {
    const dir = await manager.createCollection({ id: 'col_x', name: '!!!', variables: [], items: [] });
    expect(dir).toBe('collection');
    await expect(fs.stat(path.join(tmpDir, 'collection.json'))).rejects.toThrow();
  });
});
