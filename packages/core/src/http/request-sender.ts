import { request as undiciRequest, interceptors, Agent, setGlobalDispatcher } from 'undici';
import type { HttpMethod, NexusResponse, ResponseTiming, RequestSettings } from '../types/index.js';

export interface SendOptions {
  method: HttpMethod;
  url: string;
  headers: Record<string, string>;
  body?: string | Buffer | null;
  signal?: AbortSignal;
  settings?: RequestSettings;
}

let redirectAgentInitialized = false;

/**
 * Sets undici's global dispatcher to an agent that follows redirects up to `maxRedirects`.
 * @param maxRedirects - Maximum number of redirects to follow for subsequent requests.
 * @returns void
 */
function ensureRedirectAgent(maxRedirects: number): void {
  const agent = new Agent().compose(
    interceptors.redirect({ maxRedirections: maxRedirects }),
  );
  setGlobalDispatcher(agent);
  redirectAgentInitialized = true;
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
  if (!redirectAgentInitialized || maxRedirects !== 10) {
    ensureRedirectAgent(maxRedirects);
  }

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

    const response = await undiciRequest(opts.url, {
      method: opts.method,
      headers: opts.headers,
      body: opts.body ?? undefined,
      signal: controller.signal,
    });

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
