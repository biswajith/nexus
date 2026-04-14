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
