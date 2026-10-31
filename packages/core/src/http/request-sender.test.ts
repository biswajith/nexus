import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import * as http from 'node:http';
import type { AddressInfo } from 'node:net';
import { sendHttpRequest, stripCrossOriginHeaders } from './request-sender.js';

interface Seen {
  method: string;
  url: string;
  headers: http.IncomingHttpHeaders;
}

function listen(handler: http.RequestListener): Promise<{ server: http.Server; origin: string }> {
  return new Promise((resolve) => {
    const server = http.createServer(handler);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address() as AddressInfo;
      resolve({ server, origin: `http://127.0.0.1:${port}` });
    });
  });
}

describe('sendHttpRequest — redirects', () => {
  // Two servers on different ports are two origins.
  let target: { server: http.Server; origin: string };
  let source: { server: http.Server; origin: string };
  const seen: Seen[] = [];

  beforeAll(async () => {
    target = await listen((req, res) => {
      seen.push({ method: req.method!, url: req.url!, headers: req.headers });
      res.end('ok');
    });
    source = await listen((req, res) => {
      seen.push({ method: req.method!, url: req.url!, headers: req.headers });
      const [, code, where] = req.url!.split('/');
      if (where === 'same') {
        res.writeHead(Number(code), { location: '/final' });
      } else if (where === 'cross') {
        res.writeHead(Number(code), { location: `${target.origin}/final` });
      } else {
        res.writeHead(200);
      }
      res.end();
    });
  });

  afterAll(async () => {
    await Promise.all([source, target].map(({ server }) => new Promise((r) => server.close(r))));
  });

  const credentials = {
    'X-Api-Key': 'secret-key',
    'x-amz-security-token': 'session-token',
    Authorization: 'Bearer t',
    Cookie: 'sid=1',
    Accept: 'application/json',
  };

  it('drops credential headers when a redirect changes origin', async () => {
    seen.length = 0;
    const res = await sendHttpRequest({ method: 'GET', url: `${source.origin}/302/cross`, headers: credentials });
    expect(res.status).toBe(200);
    const last = seen.at(-1)!;
    expect(last.url).toBe('/final');
    expect(last.headers['x-api-key']).toBeUndefined();
    expect(last.headers['x-amz-security-token']).toBeUndefined();
    expect(last.headers.authorization).toBeUndefined();
    expect(last.headers.cookie).toBeUndefined();
    expect(last.headers.accept).toBe('application/json');
  });

  it('keeps headers on a same-origin redirect', async () => {
    seen.length = 0;
    await sendHttpRequest({ method: 'GET', url: `${source.origin}/301/same`, headers: credentials });
    const last = seen.at(-1)!;
    expect(last.url).toBe('/final');
    expect(last.headers['x-api-key']).toBe('secret-key');
    expect(last.headers['x-amz-security-token']).toBe('session-token');
  });

  it('does not follow redirects when followRedirects is false', async () => {
    seen.length = 0;
    const res = await sendHttpRequest({
      method: 'GET',
      url: `${source.origin}/302/cross`,
      headers: credentials,
      settings: { followRedirects: false },
    });
    expect(res.status).toBe(302);
    expect(seen).toHaveLength(1);
  });

  it('turns POST into GET on 303 and keeps the method on 307', async () => {
    seen.length = 0;
    await sendHttpRequest({ method: 'POST', url: `${source.origin}/303/same`, headers: {}, body: 'x' });
    expect(seen.at(-1)!.method).toBe('GET');

    seen.length = 0;
    await sendHttpRequest({ method: 'POST', url: `${source.origin}/307/same`, headers: {}, body: 'x' });
    expect(seen.at(-1)!.method).toBe('POST');
  });

  it('stops after maxRedirects', async () => {
    seen.length = 0;
    const res = await sendHttpRequest({
      method: 'GET',
      url: `${source.origin}/302/cross`,
      headers: {},
      settings: { maxRedirects: 0 },
    });
    expect(res.status).toBe(302);
  });
});

describe('stripCrossOriginHeaders', () => {
  it('keeps only allowlisted headers, case-insensitively', () => {
    expect(stripCrossOriginHeaders({
      'Content-Type': 'application/json',
      'USER-AGENT': 'nexus',
      'X-Custom-Token': 't',
      'Proxy-Authorization': 'p',
    })).toEqual({ 'Content-Type': 'application/json', 'USER-AGENT': 'nexus' });
  });
});
