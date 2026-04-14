import * as fs from 'node:fs/promises';

export interface StoredToken {
  accessToken: string;
  tokenType: string;
  expiresIn?: number;
  expiresAt: number;
  refreshToken?: string;
  scope?: string;
}

/**
 * In-memory map of tokens by key, with optional JSON file persistence when a path is provided.
 */
export class TokenStore {
  private tokens: Map<string, StoredToken> = new Map();

  /**
   * Creates a store; pass `filePath` to load and persist tokens as JSON on disk.
   * @param filePath - Optional path to the backing JSON file.
   */
  constructor(private filePath?: string) {}

  /**
   * Reads tokens from the backing file when `filePath` is set; clears to an empty map on failure.
   * @returns Resolves when loading finishes (or immediately if there is no file path).
   */
  async load(): Promise<void> {
    if (!this.filePath) return;
    try {
      const data = await fs.readFile(this.filePath, 'utf-8');
      const parsed = JSON.parse(data) as Record<string, StoredToken>;
      this.tokens = new Map(Object.entries(parsed));
    } catch {
      this.tokens = new Map();
    }
  }

  /**
   * Returns the stored token for `key`, or `null` if none exists.
   * @param key - Lookup key for the token entry.
   * @returns The stored token, or `null`.
   */
  async getToken(key: string): Promise<StoredToken | null> {
    const token = this.tokens.get(key);
    if (!token) return null;
    return token;
  }

  /**
   * Returns whether the token is past expiry, using a 30 second buffer before `expiresAt`.
   * @param token - Token to evaluate.
   * @returns `true` if the token should be treated as expired.
   */
  isExpired(token: StoredToken): boolean {
    return Date.now() >= token.expiresAt - 30_000;
  }

  /**
   * Stores `token` under `key` and persists to the backing file when configured.
   * @param key - Identifier for the token entry.
   * @param token - Token payload to store.
   * @returns Resolves when persistence completes.
   */
  async saveToken(key: string, token: StoredToken): Promise<void> {
    this.tokens.set(key, token);
    await this.persist();
  }

  /**
   * Deletes the token for `key` and persists to the backing file when configured.
   * @param key - Identifier of the entry to remove.
   * @returns Resolves when persistence completes.
   */
  async removeToken(key: string): Promise<void> {
    this.tokens.delete(key);
    await this.persist();
  }

  /**
   * Removes all tokens and persists to the backing file when configured.
   * @returns Resolves when persistence completes.
   */
  async clear(): Promise<void> {
    this.tokens.clear();
    await this.persist();
  }

  /**
   * Serializes the in-memory map to the backing JSON file when `filePath` is set.
   * @returns Resolves when the write completes (or immediately if there is no file path).
   */
  private async persist(): Promise<void> {
    if (!this.filePath) return;
    const obj: Record<string, StoredToken> = {};
    for (const [k, v] of this.tokens) {
      obj[k] = v;
    }
    await fs.writeFile(this.filePath, JSON.stringify(obj, null, 2));
  }
}
