export type ResponseFormat = 'json' | 'xml' | 'html' | 'text' | 'image' | 'pdf' | 'binary';

/**
 * Infers a display format from the `content-type` header (or related substrings).
 * @param headers - HTTP response headers keyed by lower-case name.
 * @returns The best-matching {@link ResponseFormat} for rendering.
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
 * Pretty-prints a JSON string; on parse failure returns the original text with an error flag.
 * @param raw - Unformatted JSON (or invalid JSON) as a string.
 * @returns Indented JSON in `formatted`, or `error` when parsing fails.
 */
export function formatJson(raw: string): { formatted: string; error?: string } {
  try {
    return { formatted: JSON.stringify(JSON.parse(raw), null, 2) };
  } catch {
    return { formatted: raw, error: 'Invalid JSON' };
  }
}

/**
 * Formats a byte length as B, KB, or MB with fixed decimal places.
 * @param bytes - Size in bytes (non-negative expected).
 * @returns Human-readable size string (e.g. `"1.5 KB"`).
 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

/**
 * Formats a duration in milliseconds as whole ms or seconds with two decimals.
 * @param ms - Elapsed time in milliseconds.
 * @returns A short string such as `"42 ms"` or `"1.23 s"`.
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
 * Builds ordered waterfall segments (DNS → TCP → TLS → TTFB → Download) with cumulative start times.
 * @param timing - Millisecond durations for each network phase and `total`.
 * @returns Segments suitable for a timing bar, each with `startMs` offset along the waterfall.
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
 * Maps an HTTP status code to a semantic color token for UI styling.
 * @param status - HTTP response status code.
 * @returns A token such as `positive`, `informative`, `notice`, `negative`, or `neutral`.
 */
export function getStatusColor(status: number): string {
  if (status >= 200 && status < 300) return 'positive';
  if (status >= 300 && status < 400) return 'informative';
  if (status >= 400 && status < 500) return 'notice';
  if (status >= 500) return 'negative';
  return 'neutral';
}
