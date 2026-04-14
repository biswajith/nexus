import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import type { HistoryEntry, HistoryFilter } from '../types/index.js';

export class HistoryManager {
  constructor(private basePath: string) {}

  private async ensureDirectory(): Promise<void> {
    await fs.mkdir(this.basePath, { recursive: true });
  }

  async log(entry: HistoryEntry): Promise<void> {
    await this.ensureDirectory();
    const date = new Date(entry.timestamp).toISOString().slice(0, 10);
    const filePath = path.join(this.basePath, `${date}.jsonl`);
    await fs.appendFile(filePath, JSON.stringify(entry) + '\n');
  }

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
