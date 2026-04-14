import { CookieJar, Cookie } from 'tough-cookie';
import * as fs from 'node:fs/promises';

export class NexusCookieJar {
  private jar: CookieJar;

  constructor(private persistPath?: string) {
    this.jar = new CookieJar();
  }

  async load(): Promise<void> {
    if (!this.persistPath) return;
    try {
      const data = await fs.readFile(this.persistPath, 'utf-8');
      const serialized = JSON.parse(data);
      this.jar = await CookieJar.deserialize(serialized);
    } catch {
      this.jar = new CookieJar();
    }
  }

  async setCookie(rawCookie: string, url: string): Promise<void> {
    await this.jar.setCookie(rawCookie, url);
    await this.persist();
  }

  async getCookies(url: string): Promise<Cookie[]> {
    return this.jar.getCookies(url);
  }

  async getCookieString(url: string): Promise<string> {
    return this.jar.getCookieString(url);
  }

  async getAllCookies(): Promise<Cookie[]> {
    const serialized = await this.jar.serialize();
    return serialized.cookies
      .map((c) => Cookie.fromJSON(c))
      .filter((c): c is Cookie => c !== null);
  }

  async removeCookie(domain: string, cookiePath: string, key: string): Promise<void> {
    const cookies = await this.getAllCookies();
    await this.jar.removeAllCookies();
    for (const cookie of cookies) {
      if (cookie.domain === domain && cookie.path === cookiePath && cookie.key === key) {
        continue;
      }
      if (cookie.domain && cookie.path) {
        await this.jar.setCookie(cookie, `https://${cookie.domain}${cookie.path}`);
      }
    }
    await this.persist();
  }

  async removeAllCookies(): Promise<void> {
    await this.jar.removeAllCookies();
    await this.persist();
  }

  async applyToRequest(url: string, headers: Record<string, string>): Promise<Record<string, string>> {
    const cookieString = await this.getCookieString(url);
    if (cookieString) {
      headers['cookie'] = cookieString;
    }
    return headers;
  }

  async processResponse(url: string, responseHeaders: Record<string, string | string[]>): Promise<void> {
    const setCookies = responseHeaders['set-cookie'];
    if (!setCookies) return;

    const cookies = Array.isArray(setCookies) ? setCookies : [setCookies];
    for (const raw of cookies) {
      await this.setCookie(raw, url);
    }
  }

  private async persist(): Promise<void> {
    if (!this.persistPath) return;
    const serialized = await this.jar.serialize();
    await fs.writeFile(this.persistPath, JSON.stringify(serialized, null, 2));
  }
}
