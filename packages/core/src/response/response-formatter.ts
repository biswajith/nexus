import type { ResponseFormat } from '../types/index.js';

/**
 * Infers a display format from the `content-type` header (JSON, HTML, XML, image, PDF, binary, or text).
 * @param headers Response header map (values may be a single string or an array).
 * @returns The best-matching `ResponseFormat` for the body.
 */
export function detectContentType(headers: Record<string, string | string[]>): ResponseFormat {
  const ct = String(headers['content-type'] ?? '').toLowerCase();
  if (ct.includes('json')) return 'json';
  if (ct.includes('html')) return 'html';
  if (ct.includes('xml')) return 'xml';
  if (ct.includes('image/')) return 'image';
  if (ct.includes('pdf')) return 'pdf';
  if (ct.includes('octet-stream')) return 'binary';
  return 'text';
}

/**
 * Pretty-prints JSON input with indentation; on parse failure returns the original string and an error flag.
 * @param raw Unparsed JSON text.
 * @returns `formatted` pretty JSON, or the raw input plus `error` when parsing fails.
 */
export function formatJson(raw: string): { formatted: string; error?: string } {
  try {
    return { formatted: JSON.stringify(JSON.parse(raw), null, 2) };
  } catch {
    return { formatted: raw, error: 'Invalid JSON' };
  }
}

/**
 * Formats a byte length as B, KB, or MB with fixed precision for larger units.
 * @param bytes Size in bytes (non-negative expected).
 * @returns Human-readable size string (e.g. `"512 B"`, `"1.5 KB"`).
 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

/**
 * Formats a duration in milliseconds as whole ms or seconds with two decimal places.
 * @param ms Elapsed time in milliseconds.
 * @returns A short duration string (e.g. `"42 ms"` or `"1.23 s"`).
 */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${(ms / 1000).toFixed(2)} s`;
}

export interface TimingWaterfallSegment {
  label: string;
  startMs: number;
  durationMs: number;
  color: string;
}

/**
 * Builds ordered waterfall segments (DNS → TCP → TLS → TTFB → Download) for timing visualization.
 * @param timing Phase durations in milliseconds; `total` is accepted but not emitted as its own segment.
 * @returns Segments with cumulative `startMs`, per-phase `durationMs`, and display `color`.
 */
export function buildTimingWaterfall(timing: {
  dns: number;
  tcp: number;
  tls: number;
  ttfb: number;
  download: number;
  total: number;
}): TimingWaterfallSegment[] {
  let offset = 0;
  const segments: TimingWaterfallSegment[] = [];

  const steps: [string, number, string][] = [
    ['DNS', timing.dns, '#8b5cf6'],
    ['TCP', timing.tcp, '#3b82f6'],
    ['TLS', timing.tls, '#f59e0b'],
    ['TTFB', timing.ttfb, '#10b981'],
    ['Download', timing.download, '#6366f1'],
  ];

  for (const [label, duration, color] of steps) {
    segments.push({ label, startMs: offset, durationMs: duration, color });
    offset += duration;
  }

  return segments;
}

/**
 * Maps an HTTP status code to a semantic color token for UI styling (success, redirect, client/server error, etc.).
 * @param status Numeric HTTP status code.
 * @returns A theme token such as `"positive"`, `"informative"`, `"notice"`, `"negative"`, or `"neutral"`.
 */
export function getStatusColor(status: number): string {
  if (status >= 200 && status < 300) return 'positive';
  if (status >= 300 && status < 400) return 'informative';
  if (status >= 400 && status < 500) return 'notice';
  if (status >= 500) return 'negative';
  return 'neutral';
}
