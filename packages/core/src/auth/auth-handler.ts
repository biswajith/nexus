import type { AuthConfig, AuthType } from '../types/index.js';
import { createHmac, createHash, randomBytes } from 'node:crypto';

export interface PreparedRequest {
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: string | Buffer | null;
}

export interface AuthResult {
  headers?: Record<string, string>;
  params?: Record<string, string>;
}

export interface AuthHandler {
  apply(config: AuthConfig, request: PreparedRequest): Promise<AuthResult>;
}

class NoAuth implements AuthHandler {
  async apply(): Promise<AuthResult> {
    return {};
  }
}

class ApiKeyAuth implements AuthHandler {
  async apply(config: AuthConfig): Promise<AuthResult> {
    const key = String(config.key ?? '');
    const value = String(config.value ?? '');
    const addTo = String(config.addTo ?? 'header');

    if (!key || !value) return {};

    if (addTo === 'query') {
      return { params: { [key]: value } };
    }
    return { headers: { [key]: value } };
  }
}

class BearerTokenAuth implements AuthHandler {
  async apply(config: AuthConfig): Promise<AuthResult> {
    const token = String(config.token ?? '');
    const prefix = String(config.prefix ?? 'Bearer');
    if (!token) return {};
    return { headers: { 'Authorization': `${prefix} ${token}` } };
  }
}

class BasicAuth implements AuthHandler {
  async apply(config: AuthConfig): Promise<AuthResult> {
    const username = String(config.username ?? '');
    const password = String(config.password ?? '');
    const encoded = Buffer.from(`${username}:${password}`).toString('base64');
    return { headers: { 'Authorization': `Basic ${encoded}` } };
  }
}

class OAuth2Auth implements AuthHandler {
  async apply(config: AuthConfig): Promise<AuthResult> {
    const accessToken = String(config.accessToken ?? '');
    const tokenType = String(config.tokenType ?? 'Bearer');
    const addTo = String(config.addTo ?? 'header');

    if (!accessToken) return {};

    if (addTo === 'query') {
      return { params: { 'access_token': accessToken } };
    }
    return { headers: { 'Authorization': `${tokenType} ${accessToken}` } };
  }
}

class DigestAuth implements AuthHandler {
  async apply(config: AuthConfig, request: PreparedRequest): Promise<AuthResult> {
    const username = String(config.username ?? '');
    const password = String(config.password ?? '');
    const realm = String(config.realm ?? '');
    const nonce = String(config.nonce ?? '');
    const qop = String(config.qop ?? 'auth');
    const algorithm = String(config.algorithm ?? 'MD5');
    const opaque = String(config.opaque ?? '');

    if (!username || !nonce) return {};

    const url = new URL(request.url);
    const uri = url.pathname + url.search;
    const nc = '00000001';
    const cnonce = randomBytes(8).toString('hex');

    const hashFn = algorithm.toUpperCase().includes('SHA-256') ? 'sha256' : 'md5';

    const ha1 = this.hash(hashFn, `${username}:${realm}:${password}`);
    const ha2 = this.hash(hashFn, `${request.method}:${uri}`);

    let responseHash: string;
    if (qop === 'auth' || qop === 'auth-int') {
      responseHash = this.hash(hashFn, `${ha1}:${nonce}:${nc}:${cnonce}:${qop}:${ha2}`);
    } else {
      responseHash = this.hash(hashFn, `${ha1}:${nonce}:${ha2}`);
    }

    let header = `Digest username="${username}", realm="${realm}", nonce="${nonce}", uri="${uri}", response="${responseHash}", algorithm=${algorithm}`;
    if (qop) header += `, qop=${qop}, nc=${nc}, cnonce="${cnonce}"`;
    if (opaque) header += `, opaque="${opaque}"`;

    return { headers: { 'Authorization': header } };
  }

  private hash(algorithm: string, data: string): string {
    return createHash(algorithm).update(data).digest('hex');
  }
}

class AwsSigV4Auth implements AuthHandler {
  async apply(config: AuthConfig, request: PreparedRequest): Promise<AuthResult> {
    const accessKey = String(config.accessKey ?? '');
    const secretKey = String(config.secretKey ?? '');
    const region = String(config.region ?? 'us-east-1');
    const service = String(config.service ?? 'execute-api');
    const sessionToken = config.sessionToken ? String(config.sessionToken) : undefined;

    if (!accessKey || !secretKey) return {};

    const url = new URL(request.url);
    const now = new Date();
    const dateStamp = now.toISOString().replace(/[:-]|\.\d{3}/g, '').slice(0, 8);
    const amzDate = now.toISOString().replace(/[:-]|\.\d{3}/g, '');

    const headers: Record<string, string> = {
      'host': url.host,
      'x-amz-date': amzDate,
    };
    if (sessionToken) {
      headers['x-amz-security-token'] = sessionToken;
    }

    const signedHeaderKeys = Object.keys(headers).sort().join(';');
    const canonicalHeaders = Object.keys(headers).sort()
      .map(k => `${k}:${headers[k]!.trim()}\n`).join('');

    const bodyHash = createHash('sha256')
      .update(request.body?.toString() ?? '')
      .digest('hex');

    const canonicalRequest = [
      request.method.toUpperCase(),
      url.pathname || '/',
      url.searchParams.toString(),
      canonicalHeaders,
      signedHeaderKeys,
      bodyHash,
    ].join('\n');

    const credentialScope = `${dateStamp}/${region}/${service}/aws4_request`;
    const stringToSign = [
      'AWS4-HMAC-SHA256',
      amzDate,
      credentialScope,
      createHash('sha256').update(canonicalRequest).digest('hex'),
    ].join('\n');

    const signingKey = this.getSignatureKey(secretKey, dateStamp, region, service);
    const signature = createHmac('sha256', signingKey).update(stringToSign).digest('hex');

    headers['Authorization'] = `AWS4-HMAC-SHA256 Credential=${accessKey}/${credentialScope}, SignedHeaders=${signedHeaderKeys}, Signature=${signature}`;

    return { headers };
  }

  private getSignatureKey(key: string, dateStamp: string, region: string, service: string): Buffer {
    const kDate = createHmac('sha256', `AWS4${key}`).update(dateStamp).digest();
    const kRegion = createHmac('sha256', kDate).update(region).digest();
    const kService = createHmac('sha256', kRegion).update(service).digest();
    return createHmac('sha256', kService).update('aws4_request').digest();
  }
}

const authHandlers: Record<string, AuthHandler> = {
  'none': new NoAuth(),
  'inherit': new NoAuth(),
  'api-key': new ApiKeyAuth(),
  'bearer': new BearerTokenAuth(),
  'basic': new BasicAuth(),
  'oauth2': new OAuth2Auth(),
  'digest': new DigestAuth(),
  'aws-sig-v4': new AwsSigV4Auth(),
};

export function getAuthHandler(type: AuthType): AuthHandler {
  return authHandlers[type] ?? authHandlers['none']!;
}

export async function resolveAuth(
  config: AuthConfig,
  request: PreparedRequest,
): Promise<PreparedRequest> {
  if (config.type === 'none' || config.type === 'inherit') {
    return request;
  }

  const handler = getAuthHandler(config.type);
  const result = await handler.apply(config, request);

  const updatedHeaders = { ...request.headers, ...result.headers };

  let updatedUrl = request.url;
  if (result.params && Object.keys(result.params).length > 0) {
    const url = new URL(request.url);
    for (const [key, value] of Object.entries(result.params)) {
      url.searchParams.set(key, value);
    }
    updatedUrl = url.toString();
  }

  return { ...request, url: updatedUrl, headers: updatedHeaders };
}
