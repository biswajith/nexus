import { describe, it, expect, vi } from 'vitest';
import { NexusConsole } from './nexus-console.js';

describe('NexusConsole', () => {
  describe('log', () => {
    it('stores a log entry with all fields', () => {
      const console = new NexusConsole();
      console.log('info', 'http', 'Request sent', { url: '/api' }, 'req-1');

      const entries = console.getAll();
      expect(entries).toHaveLength(1);
      expect(entries[0]!.level).toBe('info');
      expect(entries[0]!.source).toBe('http');
      expect(entries[0]!.message).toBe('Request sent');
      expect(entries[0]!.data).toEqual({ url: '/api' });
      expect(entries[0]!.requestId).toBe('req-1');
    });

    it('assigns a unique id to each entry', () => {
      const console = new NexusConsole();
      console.log('info', 'system', 'msg1');
      console.log('info', 'system', 'msg2');

      const entries = console.getAll();
      expect(entries[0]!.id).not.toBe(entries[1]!.id);
    });

    it('assigns a timestamp to each entry', () => {
      const console = new NexusConsole();
      const before = Date.now();
      console.log('info', 'system', 'msg');
      const after = Date.now();

      const entry = console.getAll()[0]!;
      expect(entry.timestamp).toBeGreaterThanOrEqual(before);
      expect(entry.timestamp).toBeLessThanOrEqual(after);
    });

    it('enforces maxEntries limit by evicting oldest entries', () => {
      const console = new NexusConsole();
      for (let i = 0; i < 10_001; i++) {
        console.log('info', 'system', `msg-${i}`);
      }
      const entries = console.getAll();
      expect(entries).toHaveLength(10_000);
      expect(entries[0]!.message).toBe('msg-1');
      expect(entries[entries.length - 1]!.message).toBe('msg-10000');
    });

    it('notifies entry handlers on each log', () => {
      const console = new NexusConsole();
      const handler = vi.fn();
      console.onEntry(handler);

      console.log('warn', 'script', 'test warning');
      expect(handler).toHaveBeenCalledTimes(1);
      expect(handler).toHaveBeenCalledWith(
        expect.objectContaining({ level: 'warn', message: 'test warning' }),
      );
    });
  });

  describe('convenience methods', () => {
    it('info() logs with level "info"', () => {
      const console = new NexusConsole();
      console.info('http', 'info msg');
      expect(console.getAll()[0]!.level).toBe('info');
    });

    it('warn() logs with level "warn"', () => {
      const console = new NexusConsole();
      console.warn('script', 'warn msg');
      expect(console.getAll()[0]!.level).toBe('warn');
    });

    it('error() logs with level "error"', () => {
      const console = new NexusConsole();
      console.error('system', 'error msg');
      expect(console.getAll()[0]!.level).toBe('error');
    });

    it('debug() logs with level "debug"', () => {
      const console = new NexusConsole();
      console.debug('variable', 'debug msg');
      expect(console.getAll()[0]!.level).toBe('debug');
    });

    it('convenience methods pass data and requestId through', () => {
      const console = new NexusConsole();
      console.info('http', 'msg', { detail: 1 }, 'req-42');

      const entry = console.getAll()[0]!;
      expect(entry.data).toEqual({ detail: 1 });
      expect(entry.requestId).toBe('req-42');
    });
  });

  describe('query', () => {
    function populatedConsole(): NexusConsole {
      const c = new NexusConsole();
      c.info('http', 'HTTP request sent', undefined, 'req-1');
      c.warn('script', 'Deprecated API used', undefined, 'req-1');
      c.error('system', 'Connection failed', undefined, 'req-2');
      c.debug('variable', 'Resolved variable {{host}}', undefined, 'req-2');
      return c;
    }

    it('returns all entries with no filters', () => {
      const c = populatedConsole();
      expect(c.query()).toHaveLength(4);
    });

    it('filters by level', () => {
      const c = populatedConsole();
      const results = c.query({ level: 'warn' });
      expect(results).toHaveLength(1);
      expect(results[0]!.message).toBe('Deprecated API used');
    });

    it('filters by source', () => {
      const c = populatedConsole();
      const results = c.query({ source: 'http' });
      expect(results).toHaveLength(1);
      expect(results[0]!.message).toBe('HTTP request sent');
    });

    it('filters by requestId', () => {
      const c = populatedConsole();
      const results = c.query({ requestId: 'req-2' });
      expect(results).toHaveLength(2);
    });

    it('filters by search string (case-insensitive)', () => {
      const c = populatedConsole();
      const results = c.query({ search: 'connection' });
      expect(results).toHaveLength(1);
      expect(results[0]!.message).toBe('Connection failed');
    });

    it('combines multiple filters', () => {
      const c = populatedConsole();
      const results = c.query({ level: 'debug', requestId: 'req-2' });
      expect(results).toHaveLength(1);
      expect(results[0]!.source).toBe('variable');
    });

    it('returns empty array when no entries match', () => {
      const c = populatedConsole();
      expect(c.query({ level: 'info', source: 'script' })).toHaveLength(0);
    });
  });

  describe('getAll', () => {
    it('returns a copy (not a reference to internal array)', () => {
      const console = new NexusConsole();
      console.info('system', 'msg');
      const all = console.getAll();
      all.length = 0;
      expect(console.getAll()).toHaveLength(1);
    });
  });

  describe('clear', () => {
    it('removes all entries', () => {
      const console = new NexusConsole();
      console.info('system', 'msg1');
      console.info('system', 'msg2');
      console.clear();
      expect(console.getAll()).toHaveLength(0);
    });

    it('notifies clear handlers', () => {
      const console = new NexusConsole();
      const handler = vi.fn();
      console.onClear(handler);

      console.clear();
      expect(handler).toHaveBeenCalledTimes(1);
    });
  });

  describe('onEntry', () => {
    it('notifies multiple handlers', () => {
      const console = new NexusConsole();
      const h1 = vi.fn();
      const h2 = vi.fn();
      console.onEntry(h1);
      console.onEntry(h2);

      console.info('system', 'msg');
      expect(h1).toHaveBeenCalledTimes(1);
      expect(h2).toHaveBeenCalledTimes(1);
    });

    it('returns an unsubscribe function', () => {
      const console = new NexusConsole();
      const handler = vi.fn();
      const unsub = console.onEntry(handler);

      console.info('system', 'first');
      expect(handler).toHaveBeenCalledTimes(1);

      unsub();
      console.info('system', 'second');
      expect(handler).toHaveBeenCalledTimes(1);
    });

    it('unsubscribing one handler does not affect others', () => {
      const console = new NexusConsole();
      const h1 = vi.fn();
      const h2 = vi.fn();
      const unsub1 = console.onEntry(h1);
      console.onEntry(h2);

      unsub1();
      console.info('system', 'msg');
      expect(h1).not.toHaveBeenCalled();
      expect(h2).toHaveBeenCalledTimes(1);
    });
  });

  describe('onClear', () => {
    it('notifies multiple clear handlers', () => {
      const console = new NexusConsole();
      const h1 = vi.fn();
      const h2 = vi.fn();
      console.onClear(h1);
      console.onClear(h2);

      console.clear();
      expect(h1).toHaveBeenCalledTimes(1);
      expect(h2).toHaveBeenCalledTimes(1);
    });

    it('returns an unsubscribe function', () => {
      const console = new NexusConsole();
      const handler = vi.fn();
      const unsub = console.onClear(handler);

      console.clear();
      expect(handler).toHaveBeenCalledTimes(1);

      unsub();
      console.clear();
      expect(handler).toHaveBeenCalledTimes(1);
    });
  });
});
