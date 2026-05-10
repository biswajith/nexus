import type { NexusRequest, HttpMethod, KeyValuePair, RequestBody } from '../types/index.js';

/**
 * Parses curl command strings into {@link NexusRequest} objects for use in Nexus.
 */
export class CurlImporter {
  /**
   * Parses a curl command into method, URL, headers, body, and query parameters.
   *
   * @param curlCommand - Raw curl command text (line continuations are normalized to spaces).
   * @returns A {@link NexusRequest} built from the parsed flags and URL.
   */
  import(curlCommand: string): NexusRequest {
    const normalized = curlCommand
      .replace(/\\\n/g, ' ')
      .replace(/\\\r\n/g, ' ')
      .trim();

    const tokens = this.tokenize(normalized);
    let method: HttpMethod = 'GET';
    let url = '';
    const headers: KeyValuePair[] = [];
    let bodyRaw: string | undefined;
    let bodyMode: 'json' | 'text' | 'x-www-form-urlencoded' | 'form-data' | 'none' = 'none';
    const formData: KeyValuePair[] = [];

    let i = 0;
    while (i < tokens.length) {
      const token = tokens[i]!;

      if (token === 'curl') { i++; continue; }

      if (token === '-X' || token === '--request') {
        method = (tokens[++i] ?? 'GET').toUpperCase() as HttpMethod;
      } else if (token === '-H' || token === '--header') {
        const headerStr = tokens[++i] ?? '';
        const colonIdx = headerStr.indexOf(':');
        if (colonIdx > 0) {
          headers.push({
            key: headerStr.slice(0, colonIdx).trim(),
            value: headerStr.slice(colonIdx + 1).trim(),
            enabled: true,
          });
        }
      } else if (token === '-d' || token === '--data' || token === '--data-raw' || token === '--data-binary') {
        bodyRaw = tokens[++i] ?? '';
        if (!method || method === 'GET') method = 'POST';
        const ct = headers.find((h) => h.key.toLowerCase() === 'content-type');
        if (ct?.value.includes('json') || (bodyRaw.trim().startsWith('{') || bodyRaw.trim().startsWith('['))) {
          bodyMode = 'json';
        } else if (ct?.value.includes('x-www-form-urlencoded') || bodyRaw.includes('=')) {
          bodyMode = 'x-www-form-urlencoded';
        } else {
          bodyMode = 'text';
        }
      } else if (token === '--data-urlencode') {
        const pair = tokens[++i] ?? '';
        const eqIdx = pair.indexOf('=');
        if (eqIdx > 0) {
          formData.push({ key: pair.slice(0, eqIdx), value: pair.slice(eqIdx + 1), enabled: true });
        }
        bodyMode = 'x-www-form-urlencoded';
        if (!method || method === 'GET') method = 'POST';
      } else if (token === '-F' || token === '--form') {
        const pair = tokens[++i] ?? '';
        const eqIdx = pair.indexOf('=');
        if (eqIdx > 0) {
          formData.push({ key: pair.slice(0, eqIdx), value: pair.slice(eqIdx + 1), enabled: true });
        }
        bodyMode = 'form-data';
        if (!method || method === 'GET') method = 'POST';
      } else if (token === '-u' || token === '--user') {
        const creds = tokens[++i] ?? '';
        const encoded = btoa(creds);
        headers.push({ key: 'Authorization', value: `Basic ${encoded}`, enabled: true });
      } else if (token === '-k' || token === '--insecure') {
        // SSL verification disabled
      } else if (token === '-L' || token === '--location') {
        // Follow redirects — default behavior
      } else if (token === '-v' || token === '--verbose' || token === '-s' || token === '--silent' || token === '--compressed') {
        // Ignore output/transport flags
      } else if (!token.startsWith('-') && !url) {
        url = token.replace(/^['"]|['"]$/g, '');
      }

      i++;
    }

    let body: RequestBody;
    if (bodyMode === 'json') {
      body = { mode: 'json', raw: bodyRaw };
    } else if (bodyMode === 'x-www-form-urlencoded') {
      if (formData.length > 0) {
        body = { mode: 'x-www-form-urlencoded', urlencoded: formData };
      } else if (bodyRaw) {
        const pairs = bodyRaw.split('&').map((p) => {
          const [key = '', value = ''] = p.split('=');
          return { key: decodeURIComponent(key), value: decodeURIComponent(value), enabled: true };
        });
        body = { mode: 'x-www-form-urlencoded', urlencoded: pairs };
      } else {
        body = { mode: 'none' };
      }
    } else if (bodyMode === 'form-data') {
      body = {
        mode: 'form-data',
        formData: formData.map((f) => ({ ...f, type: 'text' as const })),
      };
    } else if (bodyRaw) {
      body = { mode: 'text', raw: bodyRaw };
    } else {
      body = { mode: 'none' };
    }

    const params: KeyValuePair[] = [];
    if (url.includes('?')) {
      const [base, qs] = url.split('?');
      url = base ?? url;
      for (const pair of (qs ?? '').split('&')) {
        const [key = '', value = ''] = pair.split('=');
        params.push({ key: decodeURIComponent(key), value: decodeURIComponent(value), enabled: true });
      }
    }

    return {
      id: `req_${crypto.randomUUID().slice(0, 8)}`,
      name: `${method} ${new URL(url.startsWith('http') ? url : `https://${url}`).pathname}`,
      method,
      url,
      headers,
      params,
      body,
      auth: { type: 'none' },
      settings: {},
    };
  }

  /**
   * Splits the command into whitespace-separated tokens while honoring quoted segments.
   *
   * @param input - The command string to split (already normalized for line breaks).
   * @returns Token strings in order, without the surrounding quote delimiters.
   */
  private tokenize(input: string): string[] {
    const tokens: string[] = [];
    let current = '';
    let inSingle = false;
    let inDouble = false;

    for (let i = 0; i < input.length; i++) {
      const ch = input[i]!;

      if (ch === '$' && input[i + 1] === "'" && !inSingle && !inDouble) {
        inSingle = true;
        i++;
        continue;
      }
      if (ch === "'" && !inDouble) {
        inSingle = !inSingle;
        continue;
      }
      if (ch === '"' && !inSingle) {
        inDouble = !inDouble;
        continue;
      }
      if (ch === '\\' && inSingle) {
        const next = input[i + 1];
        if (next === 'n') { current += '\n'; i++; continue; }
        if (next === 't') { current += '\t'; i++; continue; }
        if (next === 'r') { current += '\r'; i++; continue; }
        if (next === '\\') { current += '\\'; i++; continue; }
        if (next === "'") { current += "'"; i++; continue; }
      }
      if (ch === ' ' && !inSingle && !inDouble) {
        if (current) {
          tokens.push(current);
          current = '';
        }
        continue;
      }
      current += ch;
    }
    if (current) tokens.push(current);
    return tokens;
  }
}
