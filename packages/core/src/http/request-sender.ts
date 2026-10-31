import { request as undiciRequest, Agent } from 'undici';
import type { HttpMethod, NexusResponse, ResponseTiming, RequestSettings } from '../types/index.js';

export interface SendOptions {
  method: HttpMethod;
  url: string;
  headers: Record<string, string>;
  body?: string | Buffer | null;
  signal?: AbortSignal;
  settings?: RequestSettings;
}

// Redirects are followed by hand (not with undici's redirect interceptor) so credentials can be
// dropped when a redirect leaves the original origin. Undici strips only Authorization, Cookie and
// Proxy-Authorization, but Nexus also sends credentials in arbitrary headers (API-key auth,
// x-amz-security-token, user-defined headers).
const dispatcher = new Agent();

/** Request headers that carry no credentials and are kept on a cross-origin redirect. */
const CROSS_ORIGIN_SAFE_HEADERS = new Set([
  'accept',
  'accept-encoding',
  'accept-language',
  'cache-control',
  'content-language',
  'content-type',
  'pragma',
  'user-agent',
]);

/**
 * Removes every header not on the cross-origin allowlist.
 * @param headers - Headers sent to the previous origin.
 * @returns Headers safe to send to a different origin.
 */
export function stripCrossOriginHeaders(headers: Record<string, string>): Record<string, string> {
  const kept: Record<string, string> = {};
  for (const [key, value] of Object.entries(headers)) {
    if (CROSS_ORIGIN_SAFE_HEADERS.has(key.toLowerCase())) kept[key] = value;
  }
  return kept;
}

/**
 * Sends an HTTP request, records timing, and parses the response body (including JSON when valid).
 * @param opts - Method, URL, headers, optional body/abort signal, and request settings (timeout, redirects).
 * @returns A `NexusResponse` with status, headers, body buffers/text/JSON, and timing metadata.
 */
export async function sendHttpRequest(opts: SendOptions): Promise<NexusResponse> {
  const startTime = performance.now();
  const timings: Partial<ResponseTiming> = {};

  const maxRedirects = opts.settings?.followRedirects === false ? 0 : (opts.settings?.maxRedirects ?? 10);

  const controller = new AbortController();
  const timeoutMs = opts.settings?.timeout ?? 30_000;
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

  if (opts.signal) {
    opts.signal.addEventListener('abort', () => controller.abort());
  }

  try {
    timings.dns = 0;
    timings.tcp = 0;
    timings.tls = 0;

    const ttfbStart = performance.now();

    let url = opts.url;
    let method: string = opts.method;
    let reqHeaders = opts.headers;
    let body = opts.body ?? undefined;
    let response = await undiciRequest(url, { method: method as HttpMethod, headers: reqHeaders, body, signal: controller.signal, dispatcher });

    for (let redirects = 0; redirects < maxRedirects; redirects++) {
      const location = response.headers.location;
      const target = Array.isArray(location) ? location[0] : location;
      if (!target || ![301, 302, 303, 307, 308].includes(response.statusCode)) break;

      await response.body.dump();
      const next = new URL(target, url);
      if (next.origin !== new URL(url).origin) {
        reqHeaders = stripCrossOriginHeaders(reqHeaders);
      }
      // Same rules as fetch: 303 always becomes GET; 301/302 turn a POST into GET.
      if (response.statusCode === 303 ? method !== 'HEAD' : (response.statusCode <= 302 && method === 'POST')) {
        method = 'GET';
        body = undefined;
        reqHeaders = Object.fromEntries(
          Object.entries(reqHeaders).filter(([k]) => !['content-type', 'content-length'].includes(k.toLowerCase())),
        );
      }
      url = next.href;
      response = await undiciRequest(url, { method: method as HttpMethod, headers: reqHeaders, body, signal: controller.signal, dispatcher });
    }

    timings.ttfb = performance.now() - ttfbStart;

    const downloadStart = performance.now();
    const bodyBuffer = Buffer.from(await response.body.arrayBuffer());
    timings.download = performance.now() - downloadStart;
    timings.total = performance.now() - startTime;

    const headers: Record<string, string | string[]> = {};
    for (const [key, value] of Object.entries(response.headers)) {
      if (value !== undefined) {
        headers[key.toLowerCase()] = value;
      }
    }

    let bodyText = '';
    let bodyJson: unknown = undefined;
    try {
      bodyText = bodyBuffer.toString('utf-8');
      bodyJson = JSON.parse(bodyText);
    } catch {
      // Not JSON — bodyJson stays undefined
    }

    return {
      status: response.statusCode,
      statusText: getStatusText(response.statusCode),
      headers,
      body: bodyBuffer,
      bodyText,
      bodyJson,
      timing: timings as ResponseTiming,
      size: bodyBuffer.byteLength,
    };
  } finally {
    clearTimeout(timeoutId);
  }
}

/**
 * Returns a short HTTP reason phrase for common status codes, or an empty string if unmapped.
 * @param code - Numeric HTTP status code.
 * @returns The corresponding phrase (e.g. `"OK"` for 200), or `""` when unknown.
 */
function getStatusText(code: number): string {
  const map: Record<number, string> = {
    200: 'OK', 201: 'Created', 204: 'No Content',
    301: 'Moved Permanently', 302: 'Found', 304: 'Not Modified',
    400: 'Bad Request', 401: 'Unauthorized', 403: 'Forbidden',
    404: 'Not Found', 405: 'Method Not Allowed', 409: 'Conflict',
    422: 'Unprocessable Entity', 429: 'Too Many Requests',
    500: 'Internal Server Error', 502: 'Bad Gateway',
    503: 'Service Unavailable', 504: 'Gateway Timeout',
  };
  return map[code] ?? '';
}
