import type { NexusRequest } from '../types/index.js';

/**
 * Builds a `curl` shell command string that mirrors the given Nexus request (URL, method, headers, auth, body, and settings).
 *
 * @param request - The HTTP request to translate into curl arguments.
 * @returns A multi-line curl command with line continuations (`\` + newline + two spaces).
 */
export function generateCurl(request: NexusRequest): string {
  const parts: string[] = ['curl'];

  if (request.method !== 'GET') {
    parts.push(`-X ${/^[A-Za-z]+$/.test(request.method) ? request.method : shellQuote(request.method)}`);
  }

  let url = request.url;
  const enabledParams = request.params.filter((p) => p.enabled && p.key);
  if (enabledParams.length > 0) {
    const qs = enabledParams
      .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
      .join('&');
    url += (url.includes('?') ? '&' : '?') + qs;
  }
  parts.push(shellQuote(url));

  for (const h of request.headers.filter((h) => h.enabled && h.key)) {
    parts.push(`-H ${shellQuote(`${h.key}: ${h.value}`)}`);
  }

  if (request.auth.type === 'bearer') {
    const token = String(request.auth.token ?? '');
    if (token) {
      parts.push(`-H ${shellQuote(`Authorization: Bearer ${token}`)}`);
    }
  } else if (request.auth.type === 'basic') {
    const user = String(request.auth.username ?? '');
    const pass = String(request.auth.password ?? '');
    parts.push(`-u ${shellQuote(`${user}:${pass}`)}`);
  } else if (request.auth.type === 'api-key' && request.auth.addTo === 'header') {
    const key = String(request.auth.key ?? '');
    const value = String(request.auth.value ?? '');
    if (key && value) {
      parts.push(`-H ${shellQuote(`${key}: ${value}`)}`);
    }
  }

  switch (request.body.mode) {
    case 'json':
    case 'xml':
    case 'text':
    case 'html': {
      if (request.body.raw) {
        const contentTypes: Record<string, string> = {
          json: 'application/json',
          xml: 'application/xml',
          html: 'text/html',
          text: 'text/plain',
        };
        const ct = contentTypes[request.body.mode];
        if (ct && !request.headers.some((h) => h.enabled && h.key.toLowerCase() === 'content-type')) {
          parts.push(`-H ${shellQuote(`Content-Type: ${ct}`)}`);
        }
        // --data-raw: with -d, a body starting with "@" makes curl read and send a local file.
        parts.push(`--data-raw ${shellQuote(request.body.raw)}`);
      }
      break;
    }
    case 'x-www-form-urlencoded': {
      for (const kv of request.body.urlencoded ?? []) {
        if (kv.enabled) {
          // Encoding the name keeps "@" out of it, which curl would read as "name@filename".
          parts.push(`--data-urlencode ${shellQuote(`${encodeURIComponent(kv.key)}=${kv.value}`)}`);
        }
      }
      break;
    }
    case 'form-data': {
      for (const fd of request.body.formData ?? []) {
        if (fd.enabled) {
          if (fd.type === 'file') {
            parts.push(`-F ${shellQuote(`${fd.key}=@${fd.value}`)}`);
          } else {
            // --form-string: with -F, a value starting with "@" or "<" reads a local file.
            parts.push(`--form-string ${shellQuote(`${fd.key}=${fd.value}`)}`);
          }
        }
      }
      break;
    }
    case 'binary': {
      if (request.body.binary?.path) {
        parts.push(`--data-binary ${shellQuote(`@${request.body.binary.path}`)}`);
      }
      break;
    }
    case 'graphql': {
      if (request.body.graphql?.query) {
        const gqlBody = JSON.stringify({
          query: request.body.graphql.query,
          ...(request.body.graphql.variables ? { variables: JSON.parse(request.body.graphql.variables) } : {}),
        });
        if (!request.headers.some((h) => h.enabled && h.key.toLowerCase() === 'content-type')) {
          parts.push(`-H 'Content-Type: application/json'`);
        }
        parts.push(`--data-raw ${shellQuote(gqlBody)}`);
      }
      break;
    }
  }

  if (request.settings.followRedirects === false) {
    // default is to follow, so only add flag if explicitly disabled... actually curl doesn't follow by default
  } else {
    parts.push('-L');
  }

  if (request.settings.rejectUnauthorized === false) {
    parts.push('-k');
  }

  // Settings come from imported files too, so only finite numbers reach the command line.
  const timeout = Number(request.settings.timeout);
  if (Number.isFinite(timeout) && timeout > 0) {
    parts.push(`--max-time ${Math.ceil(timeout / 1000)}`);
  }

  const maxRedirects = Number(request.settings.maxRedirects);
  if (Number.isInteger(maxRedirects) && maxRedirects > 0) {
    parts.push(`--max-redirs ${maxRedirects}`);
  }

  return parts.join(' \\\n  ');
}

/**
 * Quotes one argument for a POSIX shell: wraps it in single quotes and replaces each `'` with `'\''`.
 * Every request-derived value in the command must go through this.
 *
 * @param str - Raw argument text.
 * @returns A single shell word that expands to exactly `str`.
 */
export function shellQuote(str: string): string {
  return `'${String(str).replace(/'/g, "'\\''")}'`;
}
