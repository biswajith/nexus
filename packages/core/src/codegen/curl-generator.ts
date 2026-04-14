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
    parts.push(`-X ${request.method}`);
  }

  let url = request.url;
  const enabledParams = request.params.filter((p) => p.enabled && p.key);
  if (enabledParams.length > 0) {
    const qs = enabledParams
      .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
      .join('&');
    url += (url.includes('?') ? '&' : '?') + qs;
  }
  parts.push(`'${url}'`);

  for (const h of request.headers.filter((h) => h.enabled && h.key)) {
    parts.push(`-H '${h.key}: ${h.value}'`);
  }

  if (request.auth.type === 'bearer') {
    const token = String(request.auth.token ?? '');
    if (token) {
      parts.push(`-H 'Authorization: Bearer ${token}'`);
    }
  } else if (request.auth.type === 'basic') {
    const user = String(request.auth.username ?? '');
    const pass = String(request.auth.password ?? '');
    parts.push(`-u '${user}:${pass}'`);
  } else if (request.auth.type === 'api-key' && request.auth.addTo === 'header') {
    const key = String(request.auth.key ?? '');
    const value = String(request.auth.value ?? '');
    if (key && value) {
      parts.push(`-H '${key}: ${value}'`);
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
          parts.push(`-H 'Content-Type: ${ct}'`);
        }
        parts.push(`-d '${escapeShell(request.body.raw)}'`);
      }
      break;
    }
    case 'x-www-form-urlencoded': {
      for (const kv of request.body.urlencoded ?? []) {
        if (kv.enabled) {
          parts.push(`--data-urlencode '${kv.key}=${kv.value}'`);
        }
      }
      break;
    }
    case 'form-data': {
      for (const fd of request.body.formData ?? []) {
        if (fd.enabled) {
          if (fd.type === 'file') {
            parts.push(`-F '${fd.key}=@${fd.value}'`);
          } else {
            parts.push(`-F '${fd.key}=${fd.value}'`);
          }
        }
      }
      break;
    }
    case 'binary': {
      if (request.body.binary?.path) {
        parts.push(`--data-binary '@${request.body.binary.path}'`);
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
        parts.push(`-d '${escapeShell(gqlBody)}'`);
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

  if (request.settings.timeout) {
    parts.push(`--max-time ${Math.ceil(request.settings.timeout / 1000)}`);
  }

  if (request.settings.maxRedirects) {
    parts.push(`--max-redirs ${request.settings.maxRedirects}`);
  }

  return parts.join(' \\\n  ');
}

/**
 * Escapes a string for use inside single quotes in a POSIX shell (replaces `'` with `'\''`).
 *
 * @param str - Raw text that will be embedded in a single-quoted curl argument.
 * @returns The same text with single-quote characters safely escaped.
 */
function escapeShell(str: string): string {
  return str.replace(/'/g, "'\\''");
}
