import type { ConsoleEntry, ConsoleFilter, LogLevel, LogSource } from '../types/index.js';

type ConsoleEventHandler = (entry: ConsoleEntry) => void;
type ConsoleClearHandler = () => void;

/**
 * In-memory console buffer with optional filtering and subscription hooks for UI or tooling.
 */
export class NexusConsole {
  private entries: ConsoleEntry[] = [];
  private maxEntries = 10_000;
  private entryHandlers: ConsoleEventHandler[] = [];
  private clearHandlers: ConsoleClearHandler[] = [];

  /**
   * Appends a log entry, enforces max buffer size, and notifies entry subscribers.
   * @param level - Severity of the message.
   * @param source - Origin of the log (e.g. script or request pipeline).
   * @param message - Human-readable message text.
   * @param data - Optional structured payload stored with the entry.
   * @param requestId - Optional id to correlate logs with a request.
   * @returns void
   */
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

  /**
   * Logs an info-level message (delegates to {@link NexusConsole.log}).
   * @param source - Origin of the log (e.g. script or request pipeline).
   * @param message - Human-readable message text.
   * @param data - Optional structured payload stored with the entry.
   * @param requestId - Optional id to correlate logs with a request.
   * @returns void
   */
  info(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('info', source, message, data, requestId);
  }

  /**
   * Logs a warning-level message (delegates to {@link NexusConsole.log}).
   * @param source - Origin of the log (e.g. script or request pipeline).
   * @param message - Human-readable message text.
   * @param data - Optional structured payload stored with the entry.
   * @param requestId - Optional id to correlate logs with a request.
   * @returns void
   */
  warn(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('warn', source, message, data, requestId);
  }

  /**
   * Logs an error-level message (delegates to {@link NexusConsole.log}).
   * @param source - Origin of the log (e.g. script or request pipeline).
   * @param message - Human-readable message text.
   * @param data - Optional structured payload stored with the entry.
   * @param requestId - Optional id to correlate logs with a request.
   * @returns void
   */
  error(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('error', source, message, data, requestId);
  }

  /**
   * Logs a debug-level message (delegates to {@link NexusConsole.log}).
   * @param source - Origin of the log (e.g. script or request pipeline).
   * @param message - Human-readable message text.
   * @param data - Optional structured payload stored with the entry.
   * @param requestId - Optional id to correlate logs with a request.
   * @returns void
   */
  debug(source: LogSource, message: string, data?: unknown, requestId?: string): void {
    this.log('debug', source, message, data, requestId);
  }

  /**
   * Returns entries matching optional level, source, request id, and message substring filters.
   * @param filters - Criteria to narrow results; defaults match all entries.
   * @returns A new array of matching {@link ConsoleEntry} items (may be empty).
   */
  query(filters: ConsoleFilter = {}): ConsoleEntry[] {
    return this.entries.filter((e) => {
      if (filters.level && e.level !== filters.level) return false;
      if (filters.source && e.source !== filters.source) return false;
      if (filters.requestId && e.requestId !== filters.requestId) return false;
      if (filters.search && !e.message.toLowerCase().includes(filters.search.toLowerCase())) return false;
      return true;
    });
  }

  /**
   * Returns a snapshot copy of every stored entry in insertion order.
   * @returns A shallow copy of the internal entry list.
   */
  getAll(): ConsoleEntry[] {
    return [...this.entries];
  }

  /**
   * Clears all buffered entries and notifies clear subscribers.
   * @returns void
   */
  clear(): void {
    this.entries = [];
    for (const handler of this.clearHandlers) {
      handler();
    }
  }

  /**
   * Registers a listener invoked for each new log entry after it is stored.
   * @param handler - Callback receiving the appended {@link ConsoleEntry}.
   * @returns A function that removes this listener when called.
   */
  onEntry(handler: ConsoleEventHandler): () => void {
    this.entryHandlers.push(handler);
    return () => {
      this.entryHandlers = this.entryHandlers.filter((h) => h !== handler);
    };
  }

  /**
   * Registers a listener invoked whenever the buffer is cleared.
   * @param handler - Callback with no arguments, run after entries are wiped.
   * @returns A function that removes this listener when called.
   */
  onClear(handler: ConsoleClearHandler): () => void {
    this.clearHandlers.push(handler);
    return () => {
      this.clearHandlers = this.clearHandlers.filter((h) => h !== handler);
    };
  }
}
