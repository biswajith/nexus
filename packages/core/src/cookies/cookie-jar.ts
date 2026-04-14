import { CookieJar, Cookie } from 'tough-cookie';
import * as fs from 'node:fs/promises';

/**
 * Wraps a tough-cookie jar with optional JSON persistence to disk for HTTP requests and responses.
 */
export class NexusCookieJar {
  private jar: CookieJar;

  /**
   * Creates a jar; when `persistPath` is set, cookies can be loaded from and saved to that file.
   *
   * @param persistPath - Optional filesystem path for serialized jar storage.
   */
  constructor(private persistPath?: string) {
    this.jar = new CookieJar();
  }

  /**
   * Loads the jar from `persistPath` if set; on missing file or error, starts with an empty jar.
   *
   * @returns Resolves when loading (or no-op) completes.
   */
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

  /**
   * Parses and stores a raw `Set-Cookie` string for the given URL, then persists if configured.
   *
   * @param rawCookie - Raw `Set-Cookie` header value.
   * @param url - URL context used to associate the cookie.
   * @returns Resolves when the cookie is stored and persistence finishes.
   */
  async setCookie(rawCookie: string, url: string): Promise<void> {
    await this.jar.setCookie(rawCookie, url);
    await this.persist();
  }

  /**
   * Returns all cookies that apply to the given URL.
   *
   * @param url - URL to match cookies against.
   * @returns Cookies applicable to `url`.
   */
  async getCookies(url: string): Promise<Cookie[]> {
    return this.jar.getCookies(url);
  }

  /**
   * Builds the `Cookie` header value for a request to `url`.
   *
   * @param url - Request URL.
   * @returns Semicolon-separated cookie string, or empty if none.
   */
  async getCookieString(url: string): Promise<string> {
    return this.jar.getCookieString(url);
  }

  /**
   * Returns every cookie currently in the jar as `Cookie` instances.
   *
   * @returns All stored cookies; invalid entries from serialization are dropped.
   */
  async getAllCookies(): Promise<Cookie[]> {
    const serialized = await this.jar.serialize();
    return serialized.cookies
      .map((c) => Cookie.fromJSON(c))
      .filter((c): c is Cookie => c !== null);
  }

  /**
   * Removes the cookie identified by domain, path, and name, then persists if configured.
   *
   * @param domain - Cookie domain.
   * @param cookiePath - Cookie path.
   * @param key - Cookie name.
   * @returns Resolves when removal and persistence complete.
   */
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

  /**
   * Clears all cookies from the jar, then persists if configured.
   *
   * @returns Resolves when the jar is empty and persistence finishes.
   */
  async removeAllCookies(): Promise<void> {
    await this.jar.removeAllCookies();
    await this.persist();
  }

  /**
   * Adds a `Cookie` header to `headers` for `url` using the jar’s matching cookies.
   *
   * @param url - Request URL.
   * @param headers - Header map to mutate.
   * @returns The same `headers` object (possibly with `cookie` set).
   */
  async applyToRequest(url: string, headers: Record<string, string>): Promise<Record<string, string>> {
    const cookieString = await this.getCookieString(url);
    if (cookieString) {
      headers['cookie'] = cookieString;
    }
    return headers;
  }

  /**
   * Reads `Set-Cookie` headers from a response and stores each cookie for `url`.
   *
   * @param url - Response URL used as cookie context.
   * @param responseHeaders - Headers object; may contain string or array `set-cookie`.
   * @returns Resolves when all cookies are processed.
   */
  async processResponse(url: string, responseHeaders: Record<string, string | string[]>): Promise<void> {
    const setCookies = responseHeaders['set-cookie'];
    if (!setCookies) return;

    const cookies = Array.isArray(setCookies) ? setCookies : [setCookies];
    for (const raw of cookies) {
      await this.setCookie(raw, url);
    }
  }

  /**
   * Writes the serialized jar to `persistPath` when that path is configured.
   *
   * @returns Resolves when the file is written or when persistence is skipped.
   */
  private async persist(): Promise<void> {
    if (!this.persistPath) return;
    const serialized = await this.jar.serialize();
    await fs.writeFile(this.persistPath, JSON.stringify(serialized, null, 2));
  }
}
