import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import type { HistoryEntry, HistoryFilter } from '../types/index.js';

/**
 * Persists HTTP history as append-only JSON Lines files under a base directory (one file per calendar day).
 */
export class HistoryManager {
  /**
   * @param basePath - Root directory for dated `.jsonl` history files.
   */
  constructor(private basePath: string) {}

  /**
   * Ensures the history base directory exists, creating it and any parent directories if needed.
   * @returns A Promise that resolves when the directory is ready.
   */
  private async ensureDirectory(): Promise<void> {
    await fs.mkdir(this.basePath, { recursive: true });
  }

  /**
   * Appends one history entry to the JSONL file for the entry's calendar day.
   * @param entry - The record to persist.
   * @returns A Promise that resolves when the line has been written.
   */
  async log(entry: HistoryEntry): Promise<void> {
    await this.ensureDirectory();
    const date = new Date(entry.timestamp).toISOString().slice(0, 10);
    const filePath = path.join(this.basePath, `${date}.jsonl`);
    await fs.appendFile(filePath, JSON.stringify(entry) + '\n');
  }

  /**
   * Reads history from newest day files first, filters entries, then returns a slice for pagination.
   * @param filters - Optional date range, request/response filters, limit (default 100), and offset (default 0).
   * @returns Matching entries in reverse chronological file order, limited to the requested window.
   */
  async query(filters: HistoryFilter = {}): Promise<HistoryEntry[]> {
    await this.ensureDirectory();
    const files = await fs.readdir(this.basePath);
    const jsonlFiles = files.filter((f) => f.endsWith('.jsonl')).sort().reverse();

    const entries: HistoryEntry[] = [];
    const limit = filters.limit ?? 100;
    const offset = filters.offset ?? 0;

    for (const file of jsonlFiles) {
      const dateStr = file.replace('.jsonl', '');
      const fileDate = new Date(dateStr).getTime();

      if (filters.startDate && fileDate < filters.startDate) continue;
      if (filters.endDate && fileDate > filters.endDate) continue;

      const content = await fs.readFile(path.join(this.basePath, file), 'utf-8');
      const lines = content.trim().split('\n').filter(Boolean);

      for (const line of lines.reverse()) {
        try {
          const entry = JSON.parse(line) as HistoryEntry;
          if (filters.method && entry.request.method !== filters.method) continue;
          if (filters.statusCode && entry.response.status !== filters.statusCode) continue;
          if (filters.urlPattern && !entry.request.url.includes(filters.urlPattern)) continue;
          entries.push(entry);
        } catch {
          // Skip malformed lines
        }
      }
    }

    return entries.slice(offset, offset + limit);
  }

  /**
   * Looks up a single history entry by id across all JSONL files.
   * @param id - Unique id of the entry to find.
   * @returns The matching entry, or `null` if none exists.
   */
  async getEntry(id: string): Promise<HistoryEntry | null> {
    await this.ensureDirectory();
    const files = await fs.readdir(this.basePath);
    const jsonlFiles = files.filter((f) => f.endsWith('.jsonl'));

    for (const file of jsonlFiles) {
      const content = await fs.readFile(path.join(this.basePath, file), 'utf-8');
      const lines = content.trim().split('\n').filter(Boolean);
      for (const line of lines) {
        try {
          const entry = JSON.parse(line) as HistoryEntry;
          if (entry.id === id) return entry;
        } catch {
          // Skip malformed lines
        }
      }
    }
    return null;
  }

  /**
   * Removes `.jsonl` history files; if `before` is set, only deletes files for dates strictly before that instant.
   * @param before - Optional cutoff; files on or after this date/time are retained.
   * @returns A Promise that resolves when applicable files have been removed.
   */
  async clear(before?: Date): Promise<void> {
    await this.ensureDirectory();
    const files = await fs.readdir(this.basePath);

    for (const file of files) {
      if (!file.endsWith('.jsonl')) continue;
      if (before) {
        const dateStr = file.replace('.jsonl', '');
        if (new Date(dateStr) >= before) continue;
      }
      await fs.unlink(path.join(this.basePath, file));
    }
  }
}
