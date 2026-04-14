import * as fs from 'node:fs/promises';

export interface StoredToken {
  accessToken: string;
  tokenType: string;
  expiresIn?: number;
  expiresAt: number;
  refreshToken?: string;
  scope?: string;
}

export class TokenStore {
  private tokens: Map<string, StoredToken> = new Map();

  constructor(private filePath?: string) {}

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

  async getToken(key: string): Promise<StoredToken | null> {
    const token = this.tokens.get(key);
    if (!token) return null;
    return token;
  }

  isExpired(token: StoredToken): boolean {
    return Date.now() >= token.expiresAt - 30_000;
  }

  async saveToken(key: string, token: StoredToken): Promise<void> {
    this.tokens.set(key, token);
    await this.persist();
  }

  async removeToken(key: string): Promise<void> {
    this.tokens.delete(key);
    await this.persist();
  }

  async clear(): Promise<void> {
    this.tokens.clear();
    await this.persist();
  }

  private async persist(): Promise<void> {
    if (!this.filePath) return;
    const obj: Record<string, StoredToken> = {};
    for (const [k, v] of this.tokens) {
      obj[k] = v;
    }
    await fs.writeFile(this.filePath, JSON.stringify(obj, null, 2));
  }
}
