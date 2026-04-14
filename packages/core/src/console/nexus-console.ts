import type { ConsoleEntry, ConsoleFilter, LogLevel, LogSource } from '../types/index.js';

type ConsoleEventHandler = (entry: ConsoleEntry) => void;
type ConsoleClearHandler = () => void;

export class NexusConsole {
  private entries: ConsoleEntry[] = [];
  private maxEntries = 10_000;
  private entryHandlers: ConsoleEventHandler[] = [];
  private clearHandlers: ConsoleClearHandler[] = [];

  log(level: LogLevel, source: LogSource, message: string, data?: unknown, requestId?: string): void {
    const entry: ConsoleEntry = {
      id: crypto.randomUUID(),
      timestamp: Date.now(),
      level,
      source,
      requestId,
      message,
      data,
    };
    this.entries.push(entry);
    if (this.entries.length > this.maxEntries) {
      this.entries.shift();
    }
    for (const handler of this.entryHandlers) {
      handler(entry);
    }
  }

  info(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('info', source, message, data, requestId);
  }

  warn(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('warn', source, message, data, requestId);
  }

  error(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('error', source, message, data, requestId);
  }

  debug(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('debug', source, message, data, requestId);
  }

  query(filters: ConsoleFilter = {}): ConsoleEntry[] {
    return this.entries.filter((e) => {
      if (filters.level && e.level !== filters.level) return false;
      if (filters.source && e.source !== filters.source) return false;
      if (filters.requestId && e.requestId !== filters.requestId) return false;
      if (filters.search && !e.message.toLowerCase().includes(filters.search.toLowerCase())) return false;
      return true;
    });
  }

  getAll(): ConsoleEntry[] {
    return [...this.entries];
  }

  clear(): void {
    this.entries = [];
    for (const handler of this.clearHandlers) {
      handler();
    }
  }

  onEntry(handler: ConsoleEventHandler): () => void {
    this.entryHandlers.push(handler);
    return () => {
      this.entryHandlers = this.entryHandlers.filter((h) => h !== handler);
    };
  }

  onClear(handler: ConsoleClearHandler): () => void {
    this.clearHandlers.push(handler);
    return () => {
      this.clearHandlers = this.clearHandlers.filter((h) => h !== handler);
    };
  }
}
