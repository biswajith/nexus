import { describe, it, expect } from 'vitest';
import {
  detectContentType,
  formatJson,
  formatSize,
  formatDuration,
  buildTimingWaterfall,
  getStatusColor,
} from './response-formatter.js';

describe('detectContentType', () => {
  it('detects JSON content type', () => {
    expect(detectContentType({ 'content-type': 'application/json' })).toBe('json');
    expect(detectContentType({ 'content-type': 'application/json; charset=utf-8' })).toBe('json');
    expect(detectContentType({ 'content-type': 'text/json' })).toBe('json');
  });

  it('detects HTML content type', () => {
    expect(detectContentType({ 'content-type': 'text/html' })).toBe('html');
    expect(detectContentType({ 'content-type': 'text/html; charset=utf-8' })).toBe('html');
  });

  it('detects XML content type', () => {
    expect(detectContentType({ 'content-type': 'application/xml' })).toBe('xml');
    expect(detectContentType({ 'content-type': 'text/xml' })).toBe('xml');
  });

  it('detects image content type', () => {
    expect(detectContentType({ 'content-type': 'image/png' })).toBe('image');
    expect(detectContentType({ 'content-type': 'image/jpeg' })).toBe('image');
    expect(detectContentType({ 'content-type': 'image/svg+xml' })).toBe('xml');
  });

  it('detects PDF content type', () => {
    expect(detectContentType({ 'content-type': 'application/pdf' })).toBe('pdf');
  });

  it('detects binary/octet-stream content type', () => {
    expect(detectContentType({ 'content-type': 'application/octet-stream' })).toBe('binary');
  });

  it('falls back to text for unknown or plain text', () => {
    expect(detectContentType({ 'content-type': 'text/plain' })).toBe('text');
    expect(detectContentType({ 'content-type': 'application/yaml' })).toBe('text');
  });

  it('falls back to text when content-type header is missing', () => {
    expect(detectContentType({})).toBe('text');
  });

  it('handles array header values via String coercion', () => {
    expect(detectContentType({ 'content-type': ['application/json', 'text/plain'] })).toBe('json');
  });

  it('is case-insensitive', () => {
    expect(detectContentType({ 'content-type': 'APPLICATION/JSON' })).toBe('json');
    expect(detectContentType({ 'content-type': 'Text/HTML' })).toBe('html');
  });
});

describe('formatJson', () => {
  it('pretty-prints valid JSON', () => {
    const result = formatJson('{"a":1,"b":[2,3]}');
    expect(result.error).toBeUndefined();
    expect(result.formatted).toBe(JSON.stringify({ a: 1, b: [2, 3] }, null, 2));
  });

  it('returns original string with error for invalid JSON', () => {
    const result = formatJson('not-json');
    expect(result.formatted).toBe('not-json');
    expect(result.error).toBe('Invalid JSON');
  });

  it('handles empty object', () => {
    const result = formatJson('{}');
    expect(result.formatted).toBe('{}');
    expect(result.error).toBeUndefined();
  });

  it('handles empty array', () => {
    const result = formatJson('[]');
    expect(result.formatted).toBe('[]');
    expect(result.error).toBeUndefined();
  });

  it('handles empty string', () => {
    const result = formatJson('');
    expect(result.formatted).toBe('');
    expect(result.error).toBe('Invalid JSON');
  });

  it('formats nested JSON with proper indentation', () => {
    const input = '{"user":{"name":"Alice","tags":["admin","dev"]}}';
    const result = formatJson(input);
    expect(result.error).toBeUndefined();
    expect(result.formatted).toContain('  "user"');
    expect(result.formatted).toContain('    "name"');
  });
});

describe('formatSize', () => {
  it('formats bytes under 1 KB', () => {
    expect(formatSize(0)).toBe('0 B');
    expect(formatSize(512)).toBe('512 B');
    expect(formatSize(1023)).toBe('1023 B');
  });

  it('formats kilobytes', () => {
    expect(formatSize(1024)).toBe('1.0 KB');
    expect(formatSize(1536)).toBe('1.5 KB');
    expect(formatSize(1024 * 1023)).toBe('1023.0 KB');
  });

  it('formats megabytes', () => {
    expect(formatSize(1024 * 1024)).toBe('1.00 MB');
    expect(formatSize(1024 * 1024 * 5.25)).toBe('5.25 MB');
  });

  it('uses correct precision per unit', () => {
    expect(formatSize(2048)).toBe('2.0 KB');
    expect(formatSize(1048576)).toBe('1.00 MB');
  });
});

describe('formatDuration', () => {
  it('formats milliseconds under 1 second', () => {
    expect(formatDuration(0)).toBe('0 ms');
    expect(formatDuration(42)).toBe('42 ms');
    expect(formatDuration(999)).toBe('999 ms');
  });

  it('rounds fractional milliseconds', () => {
    expect(formatDuration(42.7)).toBe('43 ms');
    expect(formatDuration(0.4)).toBe('0 ms');
  });

  it('formats seconds for >= 1000ms', () => {
    expect(formatDuration(1000)).toBe('1.00 s');
    expect(formatDuration(1234)).toBe('1.23 s');
    expect(formatDuration(65000)).toBe('65.00 s');
  });
});

describe('buildTimingWaterfall', () => {
  const timing = { dns: 10, tcp: 20, tls: 15, ttfb: 50, download: 5, total: 100 };

  it('returns 5 segments in order', () => {
    const segments = buildTimingWaterfall(timing);
    expect(segments).toHaveLength(5);
    expect(segments.map((s) => s.label)).toEqual(['DNS', 'TCP', 'TLS', 'TTFB', 'Download']);
  });

  it('computes cumulative start offsets', () => {
    const segments = buildTimingWaterfall(timing);
    expect(segments[0].startMs).toBe(0);
    expect(segments[1].startMs).toBe(10);
    expect(segments[2].startMs).toBe(30);
    expect(segments[3].startMs).toBe(45);
    expect(segments[4].startMs).toBe(95);
  });

  it('preserves per-phase durations', () => {
    const segments = buildTimingWaterfall(timing);
    expect(segments[0].durationMs).toBe(10);
    expect(segments[1].durationMs).toBe(20);
    expect(segments[2].durationMs).toBe(15);
    expect(segments[3].durationMs).toBe(50);
    expect(segments[4].durationMs).toBe(5);
  });

  it('assigns distinct colors to each segment', () => {
    const segments = buildTimingWaterfall(timing);
    const colors = segments.map((s) => s.color);
    expect(new Set(colors).size).toBe(5);
  });

  it('handles zero-duration phases', () => {
    const zero = { dns: 0, tcp: 0, tls: 0, ttfb: 100, download: 0, total: 100 };
    const segments = buildTimingWaterfall(zero);
    expect(segments[0].startMs).toBe(0);
    expect(segments[0].durationMs).toBe(0);
    expect(segments[3].startMs).toBe(0);
    expect(segments[3].durationMs).toBe(100);
    expect(segments[4].startMs).toBe(100);
  });
});

describe('getStatusColor', () => {
  it('returns positive for 2xx', () => {
    expect(getStatusColor(200)).toBe('positive');
    expect(getStatusColor(201)).toBe('positive');
    expect(getStatusColor(204)).toBe('positive');
    expect(getStatusColor(299)).toBe('positive');
  });

  it('returns informative for 3xx', () => {
    expect(getStatusColor(301)).toBe('informative');
    expect(getStatusColor(304)).toBe('informative');
    expect(getStatusColor(399)).toBe('informative');
  });

  it('returns notice for 4xx', () => {
    expect(getStatusColor(400)).toBe('notice');
    expect(getStatusColor(404)).toBe('notice');
    expect(getStatusColor(422)).toBe('notice');
    expect(getStatusColor(499)).toBe('notice');
  });

  it('returns negative for 5xx', () => {
    expect(getStatusColor(500)).toBe('negative');
    expect(getStatusColor(502)).toBe('negative');
    expect(getStatusColor(503)).toBe('negative');
  });

  it('returns neutral for 1xx and other codes', () => {
    expect(getStatusColor(100)).toBe('neutral');
    expect(getStatusColor(101)).toBe('neutral');
    expect(getStatusColor(199)).toBe('neutral');
  });
});
