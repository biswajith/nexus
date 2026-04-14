import type {
  NexusCollection, NexusRequest, NexusFolder,
  NexusEnvironment, AuthConfig,
} from '../types/index.js';
import { isFolder } from '../types/index.js';
import { transpileNexusToPostman } from './script-transpiler.js';

interface PostmanCollectionV21 {
  info: {
    name: string;
    description?: string;
    schema: string;
    _postman_id: string;
  };
  item: PostmanItem[];
  variable?: PostmanVariable[];
  auth?: PostmanAuth;
  event?: PostmanEvent[];
}

interface PostmanItem {
  name: string;
  item?: PostmanItem[];
  request?: PostmanRequestExport;
  event?: PostmanEvent[];
  auth?: PostmanAuth;
}

interface PostmanRequestExport {
  method: string;
  url: { raw: string; host?: string[]; path?: string[]; query?: PostmanQueryParam[] };
  header: PostmanHeaderExport[];
  body?: PostmanBodyExport;
  auth?: PostmanAuth;
  description?: string;
}

interface PostmanHeaderExport {
  key: string;
  value: string;
  disabled?: boolean;
  description?: string;
}

interface PostmanQueryParam {
  key: string;
  value: string;
  disabled?: boolean;
  description?: string;
}

interface PostmanBodyExport {
  mode: string;
  raw?: string;
  urlencoded?: PostmanKeyValue[];
  formdata?: PostmanFormDataExport[];
  graphql?: { query?: string; variables?: string };
  options?: { raw?: { language: string } };
}

interface PostmanKeyValue { key: string; value: string; disabled?: boolean; description?: string }
interface PostmanFormDataExport { key: string; value?: string; type: string; disabled?: boolean; description?: string }
interface PostmanVariable { key: string; value: string; disabled?: boolean; description?: string; type?: string }
interface PostmanAuth { type: string; [key: string]: unknown }
interface PostmanEvent { listen: string; script: { type: string; exec: string[] } }

export class NexusExporter {
  exportAsNexusJson(collection: NexusCollection): string {
    return JSON.stringify(collection, null, 2);
  }

  exportAsPostmanV21(collection: NexusCollection): string {
    const warnings: string[] = [];

    const postmanCollection: PostmanCollectionV21 = {
      info: {
        name: collection.name,
        description: collection.description,
        schema: 'https://schema.getpostman.com/json/collection/v2.1.0/collection.json',
        _postman_id: collection.id,
      },
      item: collection.items.map((item) => this.convertItem(item, warnings)),
      variable: collection.variables.map((v) => ({
        key: v.key,
        value: v.value,
        disabled: !v.enabled,
        description: v.description,
        type: v.type === 'secret' ? 'secret' : 'string',
      })),
    };

    if (collection.auth && collection.auth.type !== 'none' && collection.auth.type !== 'inherit') {
      postmanCollection.auth = this.convertAuth(collection.auth);
    }

    const events: PostmanEvent[] = [];
    if (collection.preRequestScript) {
      events.push(this.buildEvent('prerequest', collection.preRequestScript));
    }
    if (collection.postResponseScript) {
      events.push(this.buildEvent('test', collection.postResponseScript));
    }
    if (events.length > 0) {
      postmanCollection.event = events;
    }

    return JSON.stringify(postmanCollection, null, 2);
  }

  exportEnvironmentAsPostman(env: NexusEnvironment): string {
    return JSON.stringify({
      name: env.name,
      _postman_variable_scope: 'environment',
      values: env.variables.map((v) => ({
        key: v.key,
        value: v.type === 'secret' ? '' : v.value,
        enabled: v.enabled,
        type: v.type === 'secret' ? 'secret' : 'default',
      })),
    }, null, 2);
  }

  private convertItem(item: NexusRequest | NexusFolder, warnings: string[]): PostmanItem {
    if (isFolder(item)) {
      return this.convertFolder(item, warnings);
    }
    return this.convertRequest(item);
  }

  private convertFolder(folder: NexusFolder, warnings: string[]): PostmanItem {
    const result: PostmanItem = {
      name: folder.name,
      item: folder.items.map((i) => this.convertItem(i, warnings)),
    };

    if (folder.auth && folder.auth.type !== 'inherit') {
      result.auth = this.convertAuth(folder.auth);
    }

    const events: PostmanEvent[] = [];
    if (folder.preRequestScript) {
      events.push(this.buildEvent('prerequest', folder.preRequestScript));
    }
    if (folder.postResponseScript) {
      events.push(this.buildEvent('test', folder.postResponseScript));
    }
    if (events.length > 0) {
      result.event = events;
    }

    return result;
  }

  private convertRequest(req: NexusRequest): PostmanItem {
    const postmanReq: PostmanRequestExport = {
      method: req.method,
      url: {
        raw: req.url + this.buildQueryString(req.params),
        query: req.params.map((p) => ({
          key: p.key,
          value: p.value,
          disabled: !p.enabled,
          description: p.description,
        })),
      },
      header: req.headers.map((h) => ({
        key: h.key,
        value: h.value,
        disabled: !h.enabled,
        description: h.description,
      })),
    };

    const body = this.convertBody(req);
    if (body) postmanReq.body = body;

    if (req.auth && req.auth.type !== 'inherit' && req.auth.type !== 'none') {
      postmanReq.auth = this.convertAuth(req.auth);
    }

    const result: PostmanItem = { name: req.name, request: postmanReq };

    const events: PostmanEvent[] = [];
    if (req.preRequestScript) {
      events.push(this.buildEvent('prerequest', req.preRequestScript));
    }
    if (req.postResponseScript) {
      events.push(this.buildEvent('test', req.postResponseScript));
    }
    if (events.length > 0) {
      result.event = events;
    }

    return result;
  }

  private convertBody(req: NexusRequest): PostmanBodyExport | undefined {
    switch (req.body.mode) {
      case 'none': return undefined;
      case 'json': return { mode: 'raw', raw: req.body.raw ?? '', options: { raw: { language: 'json' } } };
      case 'xml': return { mode: 'raw', raw: req.body.raw ?? '', options: { raw: { language: 'xml' } } };
      case 'html': return { mode: 'raw', raw: req.body.raw ?? '', options: { raw: { language: 'html' } } };
      case 'text': return { mode: 'raw', raw: req.body.raw ?? '', options: { raw: { language: 'text' } } };
      case 'x-www-form-urlencoded':
        return {
          mode: 'urlencoded',
          urlencoded: (req.body.urlencoded ?? []).map((kv) => ({
            key: kv.key, value: kv.value, disabled: !kv.enabled, description: kv.description,
          })),
        };
      case 'form-data':
        return {
          mode: 'formdata',
          formdata: (req.body.formData ?? []).map((fd) => ({
            key: fd.key, value: fd.value, type: fd.type, disabled: !fd.enabled, description: fd.description,
          })),
        };
      case 'graphql':
        return {
          mode: 'graphql',
          graphql: { query: req.body.graphql?.query, variables: req.body.graphql?.variables },
        };
      default: return undefined;
    }
  }

  private convertAuth(auth: AuthConfig): PostmanAuth {
    const kvArr = (obj: Record<string, string>): PostmanKeyValue[] =>
      Object.entries(obj).map(([key, value]) => ({ key, value }));

    switch (auth.type) {
      case 'bearer':
        return { type: 'bearer', bearer: kvArr({ token: String(auth.token ?? '') }) };
      case 'basic':
        return { type: 'basic', basic: kvArr({ username: String(auth.username ?? ''), password: String(auth.password ?? '') }) };
      case 'api-key':
        return { type: 'apikey', apikey: kvArr({ key: String(auth.key ?? ''), value: String(auth.value ?? ''), in: auth.addTo === 'query' ? 'query' : 'header' }) };
      case 'oauth2':
        return { type: 'oauth2', oauth2: kvArr({
          accessToken: String(auth.accessToken ?? ''),
          tokenType: String(auth.tokenType ?? 'Bearer'),
          addTokenTo: auth.addTo === 'query' ? 'queryParams' : 'header',
        }) };
      case 'digest':
        return { type: 'digest', digest: kvArr({
          username: String(auth.username ?? ''), password: String(auth.password ?? ''),
          realm: String(auth.realm ?? ''), nonce: String(auth.nonce ?? ''),
        }) };
      case 'aws-sig-v4':
        return { type: 'awsv4', awsv4: kvArr({
          accessKey: String(auth.accessKey ?? ''), secretKey: String(auth.secretKey ?? ''),
          region: String(auth.region ?? ''), service: String(auth.service ?? ''),
        }) };
      default:
        return { type: 'noauth' };
    }
  }

  private buildEvent(listen: string, script: string): PostmanEvent {
    const { script: transpiled } = transpileNexusToPostman(script);
    return {
      listen,
      script: { type: 'text/javascript', exec: transpiled.split('\n') },
    };
  }

  private buildQueryString(params: NexusRequest['params']): string {
    const enabled = params.filter((p) => p.enabled && p.key);
    if (enabled.length === 0) return '';
    return '?' + enabled.map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`).join('&');
  }
}
