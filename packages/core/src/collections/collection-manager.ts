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

/**
 * Reads and writes Nexus collection folders (metadata, requests, and nested folders) under a base path.
 */
export class CollectionManager {
  /**
   * Creates a manager scoped to the given collections root directory.
   * @param basePath - Root directory where collection folders are stored.
   */
  constructor(private basePath: string) {}

  /**
   * Creates the base collections directory if it does not already exist.
   * @returns A promise that resolves when the directory is ready.
   */
  async ensureDirectory(): Promise<void> {
    await fs.mkdir(this.basePath, { recursive: true });
  }

  /**
   * Scans the base path for subdirectories with a valid `collection.json` and returns their metadata.
   * @returns A promise that resolves to metadata for each valid collection; invalid dirs are skipped.
   */
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

  /**
   * Loads a collection’s `collection.json`, root-level requests, and nested folders from disk.
   * @param dirName - Folder name of the collection under the base path.
   * @returns A promise that resolves to the full `NexusCollection` including `items`.
   */
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

    return { ...meta, items: this.sortByItemOrder(items, meta.itemOrder) };
  }

  /**
   * Recursively loads a folder’s `folder.json` (or infers defaults), nested folders, and `.json` requests.
   * @param folderPath - Absolute path to the folder on disk.
   * @returns A promise that resolves to the populated `NexusFolder` tree.
   */
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

    return { ...meta, items: this.sortByItemOrder(items, meta.itemOrder) };
  }

  /**
   * Reorders items to match a persisted `itemOrder` array of IDs; unrecognized items are appended.
   * @param items - Unsorted items loaded from disk.
   * @param order - Optional ID array defining the desired order.
   * @returns Items sorted by `order`, or the original array when no order is stored.
   */
  private sortByItemOrder(items: (NexusRequest | NexusFolder)[], order?: string[]): (NexusRequest | NexusFolder)[] {
    if (!order || order.length === 0) return items;
    const itemMap = new Map(items.map((item) => [item.id, item]));
    const sorted: (NexusRequest | NexusFolder)[] = [];
    for (const id of order) {
      const item = itemMap.get(id);
      if (item) {
        sorted.push(item);
        itemMap.delete(id);
      }
    }
    for (const item of itemMap.values()) {
      sorted.push(item);
    }
    return sorted;
  }

  /**
   * Writes a new collection directory (slug from name), `collection.json`, and all requests and folders.
   * @param collection - Collection to persist, including nested folder items.
   * @returns A promise that resolves when files and directories are written.
   */
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

  /**
   * Writes a single request as a slug-named `.json` file under the collection’s `requests` directory.
   * @param collectionDir - Absolute path to the collection root (not the base manager path).
   * @param request - Request payload to serialize.
   * @returns A promise that resolves when the file is written.
   */
  async saveRequest(collectionDir: string, request: NexusRequest): Promise<void> {
    const requestsDir = path.join(collectionDir, 'requests');
    await fs.mkdir(requestsDir, { recursive: true });
    const fileName = `${this.slugify(request.name)}.json`;
    await fs.writeFile(
      path.join(requestsDir, fileName),
      JSON.stringify(request, null, 2),
    );
  }

  /**
   * Recursively writes `folder.json` and nested requests or subfolders under `folderPath`.
   * @param folderPath - Absolute path where this folder’s files should be created.
   * @param folder - Folder tree to persist.
   * @returns A promise that resolves when the folder and descendants are written.
   */
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

  /**
   * Appends a request to an existing collection by saving it under that collection’s `requests` folder.
   * @param dirName - Collection folder name under the manager’s base path.
   * @param request - Request to add.
   * @returns A promise that resolves when the request file is written.
   */
  async addRequest(dirName: string, request: NexusRequest): Promise<void> {
    const collectionDir = path.join(this.basePath, dirName);
    await this.saveRequest(collectionDir, request);
  }

  /**
   * Replaces the request file whose stored `id` matches `requestId`, or adds a new file if none match.
   * @param dirName - Collection folder name under the manager’s base path.
   * @param requestId - ID of the request to update.
   * @param request - Updated request body; `id` is forced to `requestId` when written.
   * @returns A promise that resolves when the file is updated or created.
   */
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

  /**
   * Removes the `.json` request file in the collection’s `requests` directory whose `id` matches `requestId`.
   * @param dirName - Collection folder name under the manager’s base path.
   * @param requestId - ID of the request to delete.
   * @returns A promise that resolves when the file is removed or no match is found.
   */
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

  /**
   * Deletes the entire collection directory under the base path (recursive, ignores missing paths).
   * @param dirName - Collection folder name to remove.
   * @returns A promise that resolves when removal finishes.
   */
  async deleteCollection(dirName: string): Promise<void> {
    const collectionDir = path.join(this.basePath, dirName);
    await fs.rm(collectionDir, { recursive: true, force: true });
  }

  /**
   * Converts a display name to a lowercase hyphenated slug safe for file and directory names.
   * @param name - Raw name to normalize.
   * @returns The slug string.
   */
  private slugify(name: string): string {
    return name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '');
  }
}
