import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { HistoryManager } from './history-manager.js';
import type { HistoryEntry } from '../types/index.js';

function makeEntry(overrides: Partial<HistoryEntry> & { id: string; timestamp: number }): HistoryEntry {
  return {
    request: {
      method: 'GET',
      url: 'https://api.example.com/users',
      headers: {},
    },
    response: {
      status: 200,
      statusText: 'OK',
      responseTime: 42,
      size: 1024,
    },
    ...overrides,
  };
}

describe('HistoryManager', () => {
  let tmpDir: string;
  let manager: HistoryManager;

  beforeEach(async () => {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'nexus-history-test-'));
    manager = new HistoryManager(tmpDir);
  });

  afterEach(async () => {
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  describe('log()', () => {
    it('creates the base directory if it does not exist', async () => {
      const nested = path.join(tmpDir, 'deep', 'nested', 'history');
      const nestedManager = new HistoryManager(nested);

      await nestedManager.log(makeEntry({ id: 'e1', timestamp: Date.parse('2025-06-15T10:00:00Z') }));

      const stat = await fs.stat(nested);
      expect(stat.isDirectory()).toBe(true);
    });

    it('appends an entry to a dated .jsonl file', async () => {
      const entry = makeEntry({ id: 'e1', timestamp: Date.parse('2025-06-15T10:00:00Z') });
      await manager.log(entry);

      const content = await fs.readFile(path.join(tmpDir, '2025-06-15.jsonl'), 'utf-8');
      const parsed = JSON.parse(content.trim());
      expect(parsed.id).toBe('e1');
    });

    it('groups entries by calendar day', async () => {
      await manager.log(makeEntry({ id: 'e1', timestamp: Date.parse('2025-06-15T08:00:00Z') }));
      await manager.log(makeEntry({ id: 'e2', timestamp: Date.parse('2025-06-15T20:00:00Z') }));
      await manager.log(makeEntry({ id: 'e3', timestamp: Date.parse('2025-06-16T05:00:00Z') }));

      const day15 = await fs.readFile(path.join(tmpDir, '2025-06-15.jsonl'), 'utf-8');
      expect(day15.trim().split('\n')).toHaveLength(2);

      const day16 = await fs.readFile(path.join(tmpDir, '2025-06-16.jsonl'), 'utf-8');
      expect(day16.trim().split('\n')).toHaveLength(1);
    });

    it('round-trips a full entry through log→query', async () => {
      const entry = makeEntry({
        id: 'round-trip-1',
        timestamp: Date.parse('2025-06-15T10:00:00Z'),
        collectionId: 'col-1',
        requestId: 'req-1',
      });
      entry.request.body = '{"key":"value"}';

      await manager.log(entry);
      const results = await manager.query();

      expect(results).toHaveLength(1);
      expect(results[0]).toEqual(entry);
    });
  });

  describe('query()', () => {
    beforeEach(async () => {
      await manager.log(makeEntry({
        id: 'e1', timestamp: Date.parse('2025-06-14T10:00:00Z'),
        request: { method: 'GET', url: 'https://api.example.com/users', headers: {} },
        response: { status: 200, statusText: 'OK', responseTime: 50, size: 100 },
      }));
      await manager.log(makeEntry({
        id: 'e2', timestamp: Date.parse('2025-06-15T12:00:00Z'),
        request: { method: 'POST', url: 'https://api.example.com/users', headers: {} },
        response: { status: 201, statusText: 'Created', responseTime: 120, size: 256 },
      }));
      await manager.log(makeEntry({
        id: 'e3', timestamp: Date.parse('2025-06-15T14:00:00Z'),
        request: { method: 'GET', url: 'https://api.example.com/products', headers: {} },
        response: { status: 404, statusText: 'Not Found', responseTime: 30, size: 64 },
      }));
      await manager.log(makeEntry({
        id: 'e4', timestamp: Date.parse('2025-06-16T09:00:00Z'),
        request: { method: 'DELETE', url: 'https://api.example.com/users/1', headers: {} },
        response: { status: 204, statusText: 'No Content', responseTime: 15, size: 0 },
      }));
    });

    it('returns all entries when no filters are specified', async () => {
      const results = await manager.query();
      expect(results).toHaveLength(4);
    });

    it('returns entries in reverse file order (newest day first)', async () => {
      const results = await manager.query();
      expect(results[0]!.id).toBe('e4');
    });

    it('filters by HTTP method', async () => {
      const results = await manager.query({ method: 'GET' });
      expect(results).toHaveLength(2);
      expect(results.every((e) => e.request.method === 'GET')).toBe(true);
    });

    it('filters by URL pattern (substring)', async () => {
      const results = await manager.query({ urlPattern: 'products' });
      expect(results).toHaveLength(1);
      expect(results[0]!.id).toBe('e3');
    });

    it('filters by status code', async () => {
      const results = await manager.query({ statusCode: 404 });
      expect(results).toHaveLength(1);
      expect(results[0]!.id).toBe('e3');
    });

    it('filters by startDate (excludes files before that date)', async () => {
      const results = await manager.query({ startDate: Date.parse('2025-06-15') });
      const ids = results.map((e) => e.id);
      expect(ids).not.toContain('e1');
      expect(ids).toContain('e2');
    });

    it('filters by endDate (excludes files after that date)', async () => {
      const results = await manager.query({ endDate: Date.parse('2025-06-15') });
      const ids = results.map((e) => e.id);
      expect(ids).not.toContain('e4');
      expect(ids).toContain('e2');
    });

    it('filters by date range (startDate + endDate)', async () => {
      const results = await manager.query({
        startDate: Date.parse('2025-06-15'),
        endDate: Date.parse('2025-06-15'),
      });
      expect(results.every((e) => e.id === 'e2' || e.id === 'e3')).toBe(true);
    });

    it('combines method and urlPattern filters', async () => {
      const results = await manager.query({ method: 'GET', urlPattern: 'users' });
      expect(results).toHaveLength(1);
      expect(results[0]!.id).toBe('e1');
    });

    it('applies limit', async () => {
      const results = await manager.query({ limit: 2 });
      expect(results).toHaveLength(2);
    });

    it('applies offset', async () => {
      const all = await manager.query();
      const offsetResults = await manager.query({ offset: 2 });
      expect(offsetResults).toEqual(all.slice(2));
    });

    it('applies limit and offset together', async () => {
      const all = await manager.query();
      const page = await manager.query({ limit: 1, offset: 1 });
      expect(page).toHaveLength(1);
      expect(page[0]).toEqual(all[1]);
    });

    it('returns empty array when no entries match', async () => {
      const results = await manager.query({ method: 'PATCH' as any });
      expect(results).toEqual([]);
    });

    it('returns empty array for a fresh (empty) directory', async () => {
      const emptyDir = await fs.mkdtemp(path.join(os.tmpdir(), 'nexus-empty-'));
      const emptyManager = new HistoryManager(emptyDir);
      const results = await emptyManager.query();
      expect(results).toEqual([]);
      await fs.rm(emptyDir, { recursive: true, force: true });
    });

    it('skips malformed JSON lines gracefully', async () => {
      const filePath = path.join(tmpDir, '2025-06-17.jsonl');
      await fs.writeFile(filePath, 'not-json\n{"bad json\n');

      const results = await manager.query();
      expect(results.every((e) => typeof e.id === 'string')).toBe(true);
    });
  });

  describe('getEntry()', () => {
    it('retrieves an entry by id', async () => {
      await manager.log(makeEntry({ id: 'find-me', timestamp: Date.parse('2025-06-15T10:00:00Z') }));
      await manager.log(makeEntry({ id: 'not-me', timestamp: Date.parse('2025-06-15T11:00:00Z') }));

      const entry = await manager.getEntry('find-me');
      expect(entry).not.toBeNull();
      expect(entry!.id).toBe('find-me');
    });

    it('searches across multiple day files', async () => {
      await manager.log(makeEntry({ id: 'day1', timestamp: Date.parse('2025-06-14T10:00:00Z') }));
      await manager.log(makeEntry({ id: 'day2', timestamp: Date.parse('2025-06-16T10:00:00Z') }));

      expect(await manager.getEntry('day1')).not.toBeNull();
      expect(await manager.getEntry('day2')).not.toBeNull();
    });

    it('returns null for a non-existent id', async () => {
      await manager.log(makeEntry({ id: 'exists', timestamp: Date.parse('2025-06-15T10:00:00Z') }));
      expect(await manager.getEntry('nope')).toBeNull();
    });

    it('returns null when no history files exist', async () => {
      expect(await manager.getEntry('anything')).toBeNull();
    });

    it('skips malformed lines without throwing', async () => {
      const filePath = path.join(tmpDir, '2025-06-15.jsonl');
      const good = JSON.stringify(makeEntry({ id: 'good', timestamp: Date.parse('2025-06-15T10:00:00Z') }));
      await fs.writeFile(filePath, `bad-json\n${good}\n`);

      expect(await manager.getEntry('good')).not.toBeNull();
    });
  });

  describe('clear()', () => {
    it('removes all .jsonl files when called without a date', async () => {
      await manager.log(makeEntry({ id: 'e1', timestamp: Date.parse('2025-06-14T10:00:00Z') }));
      await manager.log(makeEntry({ id: 'e2', timestamp: Date.parse('2025-06-15T10:00:00Z') }));

      await manager.clear();

      const files = await fs.readdir(tmpDir);
      expect(files.filter((f) => f.endsWith('.jsonl'))).toHaveLength(0);
    });

    it('only removes files strictly before the given date', async () => {
      await manager.log(makeEntry({ id: 'e1', timestamp: Date.parse('2025-06-13T10:00:00Z') }));
      await manager.log(makeEntry({ id: 'e2', timestamp: Date.parse('2025-06-14T10:00:00Z') }));
      await manager.log(makeEntry({ id: 'e3', timestamp: Date.parse('2025-06-15T10:00:00Z') }));

      await manager.clear(new Date('2025-06-14'));

      const files = await fs.readdir(tmpDir);
      const jsonlFiles = files.filter((f) => f.endsWith('.jsonl'));
      expect(jsonlFiles).toContain('2025-06-14.jsonl');
      expect(jsonlFiles).toContain('2025-06-15.jsonl');
      expect(jsonlFiles).not.toContain('2025-06-13.jsonl');
    });

    it('leaves non-jsonl files untouched', async () => {
      await manager.log(makeEntry({ id: 'e1', timestamp: Date.parse('2025-06-15T10:00:00Z') }));
      await fs.writeFile(path.join(tmpDir, 'metadata.json'), '{}');

      await manager.clear();

      const files = await fs.readdir(tmpDir);
      expect(files).toContain('metadata.json');
    });

    it('does not throw on an empty directory', async () => {
      await expect(manager.clear()).resolves.toBeUndefined();
    });

    it('entries are no longer queryable after full clear', async () => {
      await manager.log(makeEntry({ id: 'e1', timestamp: Date.parse('2025-06-15T10:00:00Z') }));
      await manager.clear();

      expect(await manager.query()).toEqual([]);
      expect(await manager.getEntry('e1')).toBeNull();
    });
  });

  describe('file persistence', () => {
    it('entries survive creating a new HistoryManager on the same directory', async () => {
      await manager.log(makeEntry({ id: 'persist-1', timestamp: Date.parse('2025-06-15T10:00:00Z') }));

      const manager2 = new HistoryManager(tmpDir);
      const results = await manager2.query();
      expect(results).toHaveLength(1);
      expect(results[0]!.id).toBe('persist-1');
    });

    it('preserves all fields through a save/load cycle', async () => {
      const entry = makeEntry({
        id: 'full-entry',
        timestamp: Date.parse('2025-06-15T10:00:00Z'),
        collectionId: 'col-abc',
        requestId: 'req-xyz',
      });
      entry.request = {
        method: 'POST',
        url: 'https://api.example.com/submit',
        headers: { 'content-type': 'application/json', 'authorization': 'Bearer tok' },
        body: '{"name":"test"}',
      };
      entry.response = {
        status: 201,
        statusText: 'Created',
        responseTime: 98,
        size: 512,
      };

      await manager.log(entry);
      const loaded = await manager.getEntry('full-entry');

      expect(loaded).toEqual(entry);
    });
  });
});
