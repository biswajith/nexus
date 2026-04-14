import { describe, it, expect } from 'vitest';
import { getAuthHandler, resolveAuth } from './auth-handler.js';
import type { PreparedRequest } from './auth-handler.js';
import type { AuthConfig } from '../types/index.js';

function makeRequest(overrides?: Partial<PreparedRequest>): PreparedRequest {
  return {
    method: 'GET',
    url: 'https://api.example.com/data?existing=1',
    headers: {},
    body: null,
    ...overrides,
  };
}

describe('getAuthHandler', () => {
  it('returns a handler for each known auth type', () => {
    const types = ['none', 'inherit', 'api-key', 'bearer', 'basic', 'oauth2', 'digest', 'aws-sig-v4'] as const;
    for (const type of types) {
      const handler = getAuthHandler(type);
      expect(handler).toBeDefined();
      expect(typeof handler.apply).toBe('function');
    }
  });

  it('returns the NoAuth handler for unknown types', async () => {
    const handler = getAuthHandler('unknown-type' as any);
    const result = await handler.apply({ type: 'none' }, makeRequest());
    expect(result).toEqual({});
  });
});

describe('NoAuth', () => {
  it('returns an empty result', async () => {
    const handler = getAuthHandler('none');
    const result = await handler.apply({ type: 'none' }, makeRequest());
    expect(result).toEqual({});
  });
});

describe('InheritAuth', () => {
  it('returns an empty result (same as NoAuth)', async () => {
    const handler = getAuthHandler('inherit');
    const result = await handler.apply({ type: 'inherit' }, makeRequest());
    expect(result).toEqual({});
  });
});

describe('BearerTokenAuth', () => {
  it('sets Authorization header with default Bearer prefix', async () => {
    const handler = getAuthHandler('bearer');
    const result = await handler.apply(
      { type: 'bearer', token: 'my-jwt-token' },
      makeRequest(),
    );
    expect(result.headers).toEqual({ Authorization: 'Bearer my-jwt-token' });
  });

  it('uses a custom prefix when provided', async () => {
    const handler = getAuthHandler('bearer');
    const result = await handler.apply(
      { type: 'bearer', token: 'tok123', prefix: 'Token' },
      makeRequest(),
    );
    expect(result.headers).toEqual({ Authorization: 'Token tok123' });
  });

  it('returns empty result when token is missing', async () => {
    const handler = getAuthHandler('bearer');
    const result = await handler.apply({ type: 'bearer' }, makeRequest());
    expect(result).toEqual({});
  });

  it('returns empty result when token is empty string', async () => {
    const handler = getAuthHandler('bearer');
    const result = await handler.apply({ type: 'bearer', token: '' }, makeRequest());
    expect(result).toEqual({});
  });
});

describe('BasicAuth', () => {
  it('encodes username:password in base64', async () => {
    const handler = getAuthHandler('basic');
    const result = await handler.apply(
      { type: 'basic', username: 'user', password: 'pass' },
      makeRequest(),
    );
    const expected = Buffer.from('user:pass').toString('base64');
    expect(result.headers).toEqual({ Authorization: `Basic ${expected}` });
  });

  it('handles missing username and password as empty strings', async () => {
    const handler = getAuthHandler('basic');
    const result = await handler.apply({ type: 'basic' }, makeRequest());
    const expected = Buffer.from(':').toString('base64');
    expect(result.headers).toEqual({ Authorization: `Basic ${expected}` });
  });

  it('handles special characters in credentials', async () => {
    const handler = getAuthHandler('basic');
    const result = await handler.apply(
      { type: 'basic', username: 'user@domain.com', password: 'p@ss:w0rd!' },
      makeRequest(),
    );
    const expected = Buffer.from('user@domain.com:p@ss:w0rd!').toString('base64');
    expect(result.headers).toEqual({ Authorization: `Basic ${expected}` });
  });
});

describe('ApiKeyAuth', () => {
  it('adds key/value as a header by default', async () => {
    const handler = getAuthHandler('api-key');
    const result = await handler.apply(
      { type: 'api-key', key: 'X-Api-Key', value: 'secret123' },
      makeRequest(),
    );
    expect(result.headers).toEqual({ 'X-Api-Key': 'secret123' });
  });

  it('adds key/value as a header when addTo is "header"', async () => {
    const handler = getAuthHandler('api-key');
    const result = await handler.apply(
      { type: 'api-key', key: 'X-Api-Key', value: 'secret123', addTo: 'header' },
      makeRequest(),
    );
    expect(result.headers).toEqual({ 'X-Api-Key': 'secret123' });
  });

  it('adds key/value as a query param when addTo is "query"', async () => {
    const handler = getAuthHandler('api-key');
    const result = await handler.apply(
      { type: 'api-key', key: 'api_key', value: 'abc', addTo: 'query' },
      makeRequest(),
    );
    expect(result.params).toEqual({ api_key: 'abc' });
  });

  it('returns empty when key is missing', async () => {
    const handler = getAuthHandler('api-key');
    const result = await handler.apply(
      { type: 'api-key', value: 'abc' },
      makeRequest(),
    );
    expect(result).toEqual({});
  });

  it('returns empty when value is missing', async () => {
    const handler = getAuthHandler('api-key');
    const result = await handler.apply(
      { type: 'api-key', key: 'X-Api-Key' },
      makeRequest(),
    );
    expect(result).toEqual({});
  });
});

describe('OAuth2Auth', () => {
  it('sets Authorization header with default Bearer token type', async () => {
    const handler = getAuthHandler('oauth2');
    const result = await handler.apply(
      { type: 'oauth2', accessToken: 'oauth-tok' },
      makeRequest(),
    );
    expect(result.headers).toEqual({ Authorization: 'Bearer oauth-tok' });
  });

  it('uses custom tokenType', async () => {
    const handler = getAuthHandler('oauth2');
    const result = await handler.apply(
      { type: 'oauth2', accessToken: 'tok', tokenType: 'MAC' },
      makeRequest(),
    );
    expect(result.headers).toEqual({ Authorization: 'MAC tok' });
  });

  it('sends access_token as query param when addTo is "query"', async () => {
    const handler = getAuthHandler('oauth2');
    const result = await handler.apply(
      { type: 'oauth2', accessToken: 'tok', addTo: 'query' },
      makeRequest(),
    );
    expect(result.params).toEqual({ access_token: 'tok' });
  });

  it('returns empty when accessToken is missing', async () => {
    const handler = getAuthHandler('oauth2');
    const result = await handler.apply({ type: 'oauth2' }, makeRequest());
    expect(result).toEqual({});
  });
});

describe('DigestAuth', () => {
  const digestConfig: AuthConfig = {
    type: 'digest',
    username: 'admin',
    password: 'secret',
    realm: 'test@example.com',
    nonce: 'abc123nonce',
    qop: 'auth',
    algorithm: 'MD5',
    opaque: 'opaque-val',
  };

  it('produces a Digest Authorization header with expected fields', async () => {
    const handler = getAuthHandler('digest');
    const result = await handler.apply(digestConfig, makeRequest({ method: 'GET', url: 'https://api.example.com/resource?q=1' }));
    const header = result.headers!['Authorization'];
    expect(header).toContain('Digest');
    expect(header).toContain('username="admin"');
    expect(header).toContain('realm="test@example.com"');
    expect(header).toContain('nonce="abc123nonce"');
    expect(header).toContain('uri="/resource?q=1"');
    expect(header).toContain('algorithm=MD5');
    expect(header).toContain('qop=auth');
    expect(header).toContain('nc=00000001');
    expect(header).toContain('opaque="opaque-val"');
    expect(header).toMatch(/response="[a-f0-9]+"/);
    expect(header).toMatch(/cnonce="[a-f0-9]+"/);
  });

  it('returns empty when username is missing', async () => {
    const handler = getAuthHandler('digest');
    const result = await handler.apply(
      { type: 'digest', nonce: 'nonce' },
      makeRequest(),
    );
    expect(result).toEqual({});
  });

  it('returns empty when nonce is missing', async () => {
    const handler = getAuthHandler('digest');
    const result = await handler.apply(
      { type: 'digest', username: 'admin' },
      makeRequest(),
    );
    expect(result).toEqual({});
  });

  it('handles SHA-256 algorithm', async () => {
    const handler = getAuthHandler('digest');
    const result = await handler.apply(
      { ...digestConfig, algorithm: 'SHA-256' },
      makeRequest(),
    );
    const header = result.headers!['Authorization'];
    expect(header).toContain('algorithm=SHA-256');
    expect(header).toMatch(/response="[a-f0-9]{64}"/);
  });

  it('handles qop without auth (no qop field)', async () => {
    const handler = getAuthHandler('digest');
    const result = await handler.apply(
      { ...digestConfig, qop: '' },
      makeRequest(),
    );
    const header = result.headers!['Authorization'];
    expect(header).toContain('Digest');
    expect(header).toContain('username="admin"');
  });

  it('omits opaque when not provided', async () => {
    const handler = getAuthHandler('digest');
    const result = await handler.apply(
      { ...digestConfig, opaque: '' },
      makeRequest(),
    );
    const header = result.headers!['Authorization'];
    expect(header).not.toContain('opaque=');
  });
});

describe('AwsSigV4Auth', () => {
  const awsConfig: AuthConfig = {
    type: 'aws-sig-v4',
    accessKey: 'AKIAIOSFODNN7EXAMPLE',
    secretKey: 'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY',
    region: 'us-east-1',
    service: 's3',
  };

  it('produces AWS4-HMAC-SHA256 Authorization header', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(awsConfig, makeRequest({ method: 'GET', url: 'https://s3.amazonaws.com/mybucket' }));
    const authHeader = result.headers!['Authorization'];
    expect(authHeader).toContain('AWS4-HMAC-SHA256');
    expect(authHeader).toContain('Credential=AKIAIOSFODNN7EXAMPLE/');
    expect(authHeader).toContain('/us-east-1/s3/aws4_request');
    expect(authHeader).toContain('SignedHeaders=');
    expect(authHeader).toContain('Signature=');
  });

  it('includes x-amz-date header', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(awsConfig, makeRequest());
    expect(result.headers!['x-amz-date']).toMatch(/^\d{8}T\d{6}Z$/);
  });

  it('includes host header', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(awsConfig, makeRequest());
    expect(result.headers!['host']).toBe('api.example.com');
  });

  it('includes x-amz-security-token when sessionToken is set', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(
      { ...awsConfig, sessionToken: 'my-session-token' },
      makeRequest(),
    );
    expect(result.headers!['x-amz-security-token']).toBe('my-session-token');
  });

  it('does not include x-amz-security-token when sessionToken is absent', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(awsConfig, makeRequest());
    expect(result.headers!['x-amz-security-token']).toBeUndefined();
  });

  it('returns empty when accessKey is missing', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(
      { ...awsConfig, accessKey: '' },
      makeRequest(),
    );
    expect(result).toEqual({});
  });

  it('returns empty when secretKey is missing', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(
      { ...awsConfig, secretKey: '' },
      makeRequest(),
    );
    expect(result).toEqual({});
  });

  it('defaults region to us-east-1 and service to execute-api', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const result = await handler.apply(
      { type: 'aws-sig-v4', accessKey: 'AK', secretKey: 'SK' },
      makeRequest(),
    );
    const authHeader = result.headers!['Authorization'];
    expect(authHeader).toContain('/us-east-1/execute-api/aws4_request');
  });

  it('hashes the request body', async () => {
    const handler = getAuthHandler('aws-sig-v4');
    const resultWithBody = await handler.apply(awsConfig, makeRequest({ body: '{"key":"value"}' }));
    const resultNoBody = await handler.apply(awsConfig, makeRequest({ body: null }));
    expect(resultWithBody.headers!['Authorization']).not.toBe(resultNoBody.headers!['Authorization']);
  });
});

describe('resolveAuth', () => {
  it('returns the request unchanged for type "none"', async () => {
    const req = makeRequest();
    const result = await resolveAuth({ type: 'none' }, req);
    expect(result).toBe(req);
  });

  it('returns the request unchanged for type "inherit"', async () => {
    const req = makeRequest();
    const result = await resolveAuth({ type: 'inherit' }, req);
    expect(result).toBe(req);
  });

  it('merges auth headers into request headers', async () => {
    const req = makeRequest({ headers: { 'X-Custom': 'value' } });
    const result = await resolveAuth(
      { type: 'bearer', token: 'tok' },
      req,
    );
    expect(result.headers['Authorization']).toBe('Bearer tok');
    expect(result.headers['X-Custom']).toBe('value');
  });

  it('does not mutate the original request', async () => {
    const req = makeRequest({ headers: { 'X-Custom': 'value' } });
    await resolveAuth({ type: 'bearer', token: 'tok' }, req);
    expect(req.headers['Authorization']).toBeUndefined();
  });

  it('appends auth query params to the URL', async () => {
    const req = makeRequest({ url: 'https://api.example.com/data' });
    const result = await resolveAuth(
      { type: 'api-key', key: 'api_key', value: 'secret', addTo: 'query' },
      req,
    );
    const url = new URL(result.url);
    expect(url.searchParams.get('api_key')).toBe('secret');
  });

  it('preserves existing query params when appending auth params', async () => {
    const req = makeRequest({ url: 'https://api.example.com/data?page=1' });
    const result = await resolveAuth(
      { type: 'api-key', key: 'api_key', value: 'secret', addTo: 'query' },
      req,
    );
    const url = new URL(result.url);
    expect(url.searchParams.get('page')).toBe('1');
    expect(url.searchParams.get('api_key')).toBe('secret');
  });
});
