import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import type { NexusCollection, NexusRequest, NexusFolder } from '../types/index.js';
import { isFolder } from '../types/index.js';

interface CollectionMeta {
  id: string;
  name: string;
  description?: string;
  path: string;
}

export class CollectionManager {
  constructor(private basePath: string) {}

  async ensureDirectory(): Promise<void> {
    await fs.mkdir(this.basePath, { recursive: true });
  }

  async listCollections(): Promise<CollectionMeta[]> {
    await this.ensureDirectory();
    const entries = await fs.readdir(this.basePath, { withFileTypes: true });
    const collections: CollectionMeta[] = [];

    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      const collectionJsonPath = path.join(this.basePath, entry.name, 'collection.json');
      try {
        const data = await fs.readFile(collectionJsonPath, 'utf-8');
        const parsed = JSON.parse(data) as { id: string; name: string; description?: string };
        collections.push({
          id: parsed.id,
          name: parsed.name,
          description: parsed.description,
          path: path.join(this.basePath, entry.name),
        });
      } catch {
        // Skip directories without valid collection.json
      }
    }

    return collections;
  }

  async loadCollection(dirName: string): Promise<NexusCollection> {
    const collectionDir = path.join(this.basePath, dirName);
    const metaPath = path.join(collectionDir, 'collection.json');
    const meta = JSON.parse(await fs.readFile(metaPath, 'utf-8')) as NexusCollection;

    const requestsDir = path.join(collectionDir, 'requests');
    const foldersDir = path.join(collectionDir, 'folders');

    const items: (NexusRequest | NexusFolder)[] = [];

    try {
      const reqFiles = await fs.readdir(requestsDir);
      for (const file of reqFiles) {
        if (!file.endsWith('.json')) continue;
        const data = await fs.readFile(path.join(requestsDir, file), 'utf-8');
        items.push(JSON.parse(data) as NexusRequest);
      }
    } catch {
      // No requests directory yet
    }

    try {
      const folderEntries = await fs.readdir(foldersDir, { withFileTypes: true });
      for (const entry of folderEntries) {
        if (!entry.isDirectory()) continue;
        const folder = await this.loadFolder(path.join(foldersDir, entry.name));
        items.push(folder);
      }
    } catch {
      // No folders directory yet
    }

    return { ...meta, items };
  }

  private async loadFolder(folderPath: string): Promise<NexusFolder> {
    const metaPath = path.join(folderPath, 'folder.json');
    let meta: NexusFolder;
    try {
      meta = JSON.parse(await fs.readFile(metaPath, 'utf-8')) as NexusFolder;
    } catch {
      meta = {
        id: path.basename(folderPath),
        name: path.basename(folderPath),
        items: [],
      };
    }

    const items: (NexusRequest | NexusFolder)[] = [];

    const entries = await fs.readdir(folderPath, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.name === 'folder.json') continue;
      const fullPath = path.join(folderPath, entry.name);
      if (entry.isDirectory()) {
        items.push(await this.loadFolder(fullPath));
      } else if (entry.name.endsWith('.json')) {
        const data = await fs.readFile(fullPath, 'utf-8');
        items.push(JSON.parse(data) as NexusRequest);
      }
    }

    return { ...meta, items };
  }

  async createCollection(collection: NexusCollection): Promise<void> {
    const slug = this.slugify(collection.name);
    const collectionDir = path.join(this.basePath, slug);
    await fs.mkdir(path.join(collectionDir, 'requests'), { recursive: true });
    await fs.mkdir(path.join(collectionDir, 'folders'), { recursive: true });

    const { items: _items, ...meta } = collection;
    await fs.writeFile(
      path.join(collectionDir, 'collection.json'),
      JSON.stringify({ ...meta, itemOrder: collection.items.map((i) => i.id) }, null, 2),
    );

    for (const item of collection.items) {
      if (isFolder(item)) {
        await this.saveFolder(path.join(collectionDir, 'folders', this.slugify(item.name)), item);
      } else {
        await this.saveRequest(collectionDir, item);
      }
    }
  }

  async saveRequest(collectionDir: string, request: NexusRequest): Promise<void> {
    const requestsDir = path.join(collectionDir, 'requests');
    await fs.mkdir(requestsDir, { recursive: true });
    const fileName = `${this.slugify(request.name)}.json`;
    await fs.writeFile(
      path.join(requestsDir, fileName),
      JSON.stringify(request, null, 2),
    );
  }

  private async saveFolder(folderPath: string, folder: NexusFolder): Promise<void> {
    await fs.mkdir(folderPath, { recursive: true });

    const { items: _items, ...meta } = folder;
    await fs.writeFile(
      path.join(folderPath, 'folder.json'),
      JSON.stringify({ ...meta, itemOrder: folder.items.map((i) => i.id) }, null, 2),
    );

    for (const item of folder.items) {
      if (isFolder(item)) {
        await this.saveFolder(path.join(folderPath, this.slugify(item.name)), item);
      } else {
        const fileName = `${this.slugify(item.name)}.json`;
        await fs.writeFile(
          path.join(folderPath, fileName),
          JSON.stringify(item, null, 2),
        );
      }
    }
  }

  async addRequest(dirName: string, request: NexusRequest): Promise<void> {
    const collectionDir = path.join(this.basePath, dirName);
    await this.saveRequest(collectionDir, request);
  }

  async updateRequest(dirName: string, requestId: string, request: NexusRequest): Promise<void> {
    const requestsDir = path.join(this.basePath, dirName, 'requests');
    try {
      const files = await fs.readdir(requestsDir);
      for (const file of files) {
        if (!file.endsWith('.json')) continue;
        const filePath = path.join(requestsDir, file);
        const data = JSON.parse(await fs.readFile(filePath, 'utf-8')) as { id?: string };
        if (data.id === requestId) {
          await fs.writeFile(filePath, JSON.stringify({ ...request, id: requestId }, null, 2));
          return;
        }
      }
    } catch {
      // requests dir may not exist
    }
    // If not found, create as new
    await this.addRequest(dirName, { ...request, id: requestId });
  }

  async deleteRequest(dirName: string, requestId: string): Promise<void> {
    const requestsDir = path.join(this.basePath, dirName, 'requests');
    try {
      const files = await fs.readdir(requestsDir);
      for (const file of files) {
        if (!file.endsWith('.json')) continue;
        const filePath = path.join(requestsDir, file);
        const data = JSON.parse(await fs.readFile(filePath, 'utf-8')) as { id?: string };
        if (data.id === requestId) {
          await fs.rm(filePath);
          return;
        }
      }
    } catch {
      // requests dir may not exist
    }
  }

  async deleteCollection(dirName: string): Promise<void> {
    const collectionDir = path.join(this.basePath, dirName);
    await fs.rm(collectionDir, { recursive: true, force: true });
  }

  private slugify(name: string): string {
    return name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '');
  }
}
