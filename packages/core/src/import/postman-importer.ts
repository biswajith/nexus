import type {
  NexusCollection, NexusRequest, NexusFolder, NexusEnvironment,
  HttpMethod, KeyValuePair, RequestBody, AuthConfig, Variable, BodyMode,
} from '../types/index.js';
import type { ImportResult } from './types.js';
import { transpilePostmanToNexus } from './script-transpiler.js';

// Postman Collection v2.1 types (subset)
interface PostmanCollection {
  info: {
    name: string;
    description?: string;
    schema?: string;
    _postman_id?: string;
  };
  item: PostmanItem[];
  variable?: PostmanVariable[];
  auth?: PostmanAuth;
  event?: PostmanEvent[];
}

interface PostmanItem {
  name: string;
  item?: PostmanItem[];
  request?: PostmanRequest;
  response?: PostmanExampleResponse[];
  event?: PostmanEvent[];
  auth?: PostmanAuth;
  variable?: PostmanVariable[];
}

interface PostmanRequest {
  method: string;
  url: string | PostmanUrl;
  header?: PostmanHeader[];
  body?: PostmanBody;
  auth?: PostmanAuth;
  description?: string;
}

interface PostmanUrl {
  raw?: string;
  protocol?: string;
  host?: string[];
  path?: string[];
  query?: PostmanQueryParam[];
  variable?: PostmanVariable[];
}

interface PostmanQueryParam {
  key: string;
  value: string;
  disabled?: boolean;
  description?: string;
}

interface PostmanHeader {
  key: string;
  value: string;
  disabled?: boolean;
  description?: string;
}

interface PostmanBody {
  mode?: string;
  raw?: string;
  urlencoded?: PostmanKeyValue[];
  formdata?: PostmanFormDataItem[];
  graphql?: { query?: string; variables?: string };
  options?: { raw?: { language?: string } };
}

interface PostmanKeyValue {
  key: string;
  value: string;
  disabled?: boolean;
  description?: string;
  type?: string;
}

interface PostmanFormDataItem {
  key: string;
  value?: string;
  src?: string;
  disabled?: boolean;
  description?: string;
  type: 'text' | 'file';
}

interface PostmanAuth {
  type: string;
  bearer?: PostmanKeyValue[];
  basic?: PostmanKeyValue[];
  apikey?: PostmanKeyValue[];
  oauth2?: PostmanKeyValue[];
  digest?: PostmanKeyValue[];
  awsv4?: PostmanKeyValue[];
  hawk?: PostmanKeyValue[];
  [key: string]: unknown;
}

interface PostmanEvent {
  listen: string;
  script: { exec?: string[]; type?: string };
}

interface PostmanVariable {
  key: string;
  value?: string;
  disabled?: boolean;
  description?: string;
  type?: string;
}

interface PostmanExampleResponse {
  name: string;
  status: string;
  code: number;
  header?: PostmanHeader[];
  body?: string;
}

interface PostmanEnvironment {
  name: string;
  values: PostmanVariable[];
  _postman_variable_scope?: string;
}

/**
 * Converts Postman Collection v2.1 and Postman environment exports into Nexus collections and environments.
 */
export class PostmanImporter {
  private warnings: string[] = [];

  /**
   * Imports a Postman collection JSON value into an {@link ImportResult} with one Nexus collection and script/auth warnings.
   *
   * @param data - Unparsed Postman collection payload (Postman v2.1 shape).
   * @returns Import result containing the converted collection, no environments, and accumulated warnings.
   */
  importCollection(data: unknown): ImportResult {
    this.warnings = [];
    const parsed = data as PostmanCollection;

    const collection: NexusCollection = {
      id: `col_${crypto.randomUUID().slice(0, 8)}`,
      name: parsed.info.name,
      description: parsed.info.description,
      variables: (parsed.variable ?? []).map((v) => this.convertVariable(v)),
      auth: parsed.auth ? this.convertAuth(parsed.auth) : undefined,
      preRequestScript: this.extractScript(parsed.event, 'prerequest'),
      postResponseScript: this.extractScript(parsed.event, 'test'),
      items: parsed.item.map((item) => this.convertItem(item)),
    };

    return { collections: [collection], environments: [], warnings: [...this.warnings] };
  }

  /**
   * Imports a Postman environment JSON value into an {@link ImportResult} with one Nexus environment.
   *
   * @param data - Unparsed Postman environment payload.
   * @returns Import result with the converted environment, no collections, and no warnings.
   */
  importEnvironment(data: unknown): ImportResult {
    const parsed = data as PostmanEnvironment;

    const environment: NexusEnvironment = {
      id: crypto.randomUUID(),
      name: parsed.name,
      variables: parsed.values.map((v) => this.convertVariable(v)),
    };

    return { collections: [], environments: [environment], warnings: [] };
  }

  /**
   * Recursively converts a Postman `item` node to either a Nexus folder or a single request.
   *
   * @param item - Postman folder or request entry from the collection `item` tree.
   * @returns A folder when the node has nested items without a direct request; otherwise a request.
   */
  private convertItem(item: PostmanItem): NexusRequest | NexusFolder {
    if (item.item && !item.request) {
      return this.convertFolder(item);
    }
    return this.convertRequest(item);
  }

  /**
   * Builds a Nexus folder from a Postman folder item, including nested items, auth, and scripts.
   *
   * @param item - Postman item representing a folder (nested `item` array, no standalone `request`).
   * @returns A Nexus folder with a generated id and recursively converted children.
   */
  private convertFolder(item: PostmanItem): NexusFolder {
    return {
      id: `folder_${crypto.randomUUID().slice(0, 8)}`,
      name: item.name,
      items: (item.item ?? []).map((i) => this.convertItem(i)),
      auth: item.auth ? this.convertAuth(item.auth) : undefined,
      preRequestScript: this.extractScript(item.event, 'prerequest'),
      postResponseScript: this.extractScript(item.event, 'test'),
    };
  }

  /**
   * Converts a Postman request item into a fully populated Nexus request.
   *
   * @param item - Postman item whose `request` field defines the HTTP call.
   * @returns A Nexus request with URL, headers, query params, body, auth, and scripts.
   */
  private convertRequest(item: PostmanItem): NexusRequest {
    const req = item.request!;
    return {
      id: `req_${crypto.randomUUID().slice(0, 8)}`,
      name: item.name,
      method: (req.method ?? 'GET').toUpperCase() as HttpMethod,
      url: this.buildUrl(req.url),
      headers: (req.header ?? []).map((h) => ({
        key: h.key,
        value: h.value,
        enabled: !h.disabled,
        description: h.description,
      })),
      params: this.extractParams(req.url),
      body: this.convertBody(req.body),
      auth: req.auth ? this.convertAuth(req.auth) : { type: 'inherit' },
      preRequestScript: this.extractScript(item.event, 'prerequest'),
      postResponseScript: this.extractScript(item.event, 'test'),
      settings: {},
    };
  }

  /**
   * Normalizes Postman's `url` field to a single string (structured object's `raw` or a plain string).
   *
   * @param url - Postman URL object, string, or undefined.
   * @returns URL string, or empty string when absent.
   */
  private buildUrl(url: string | PostmanUrl | undefined): string {
    if (!url) return '';
    if (typeof url === 'string') return url;
    return url.raw ?? '';
  }

  /**
   * Extracts query parameters from a Postman URL string (`?...`) or from a structured URL's `query` array.
   *
   * @param url - Postman URL as string or object, or undefined.
   * @returns Key/value pairs with enabled flags for Nexus request params.
   */
  private extractParams(url: string | PostmanUrl | undefined): KeyValuePair[] {
    if (!url || typeof url === 'string') {
      if (typeof url === 'string' && url.includes('?')) {
        const qs = url.split('?')[1] ?? '';
        return qs.split('&').map((pair) => {
          const [key = '', value = ''] = pair.split('=');
          return { key: decodeURIComponent(key), value: decodeURIComponent(value), enabled: true };
        });
      }
      return [];
    }
    return (url.query ?? []).map((q) => ({
      key: q.key,
      value: q.value ?? '',
      enabled: !q.disabled,
      description: q.description,
    }));
  }

  /**
   * Maps Postman body modes (raw, urlencoded, formdata, graphql, file) to a Nexus {@link RequestBody}.
   *
   * @param body - Postman request body or undefined.
   * @returns Nexus body with mode and payload fields, or `{ mode: 'none' }` when missing or unsupported.
   */
  private convertBody(body: PostmanBody | undefined): RequestBody {
    if (!body || !body.mode) return { mode: 'none' };

    switch (body.mode) {
      case 'raw': {
        const lang = body.options?.raw?.language ?? 'text';
        let mode: BodyMode = 'text';
        if (lang === 'json' || (body.raw && body.raw.trim().startsWith('{'))) mode = 'json';
        else if (lang === 'xml') mode = 'xml';
        else if (lang === 'html') mode = 'html';
        return { mode, raw: body.raw ?? '' };
      }
      case 'urlencoded':
        return {
          mode: 'x-www-form-urlencoded',
          urlencoded: (body.urlencoded ?? []).map((kv) => ({
            key: kv.key,
            value: kv.value,
            enabled: !kv.disabled,
            description: kv.description,
          })),
        };
      case 'formdata':
        return {
          mode: 'form-data',
          formData: (body.formdata ?? []).map((fd) => ({
            key: fd.key,
            value: fd.value ?? fd.src ?? '',
            type: fd.type,
            enabled: !fd.disabled,
            description: fd.description,
          })),
        };
      case 'graphql':
        return {
          mode: 'graphql',
          graphql: {
            query: body.graphql?.query ?? '',
            variables: body.graphql?.variables,
          },
        };
      case 'file':
        return { mode: 'binary' };
      default:
        return { mode: 'none' };
    }
  }

  /**
   * Converts Postman authentication to Nexus {@link AuthConfig}, pushing warnings for unknown or partial types.
   *
   * @param auth - Postman auth object with `type` and type-specific key/value lists.
   * @returns Equivalent Nexus auth, or `{ type: 'none' }` when unsupported or defaulted.
   */
  private convertAuth(auth: PostmanAuth): AuthConfig {
    const findVal = (arr: PostmanKeyValue[] | undefined, key: string): string =>
      arr?.find((kv) => kv.key === key)?.value ?? '';

    switch (auth.type) {
      case 'bearer':
        return { type: 'bearer', token: findVal(auth.bearer, 'token') };
      case 'basic':
        return { type: 'basic', username: findVal(auth.basic, 'username'), password: findVal(auth.basic, 'password') };
      case 'apikey':
        return {
          type: 'api-key',
          key: findVal(auth.apikey, 'key'),
          value: findVal(auth.apikey, 'value'),
          addTo: findVal(auth.apikey, 'in') === 'query' ? 'query' : 'header',
        };
      case 'oauth2':
        return {
          type: 'oauth2',
          accessToken: findVal(auth.oauth2, 'accessToken'),
          tokenType: findVal(auth.oauth2, 'tokenType') || 'Bearer',
          addTo: findVal(auth.oauth2, 'addTokenTo') === 'queryParams' ? 'query' : 'header',
          authUrl: findVal(auth.oauth2, 'authUrl'),
          tokenUrl: findVal(auth.oauth2, 'accessTokenUrl'),
          clientId: findVal(auth.oauth2, 'clientId'),
          clientSecret: findVal(auth.oauth2, 'clientSecret'),
          scope: findVal(auth.oauth2, 'scope'),
          redirectUri: findVal(auth.oauth2, 'redirect_uri'),
        };
      case 'digest':
        return {
          type: 'digest',
          username: findVal(auth.digest, 'username'),
          password: findVal(auth.digest, 'password'),
          realm: findVal(auth.digest, 'realm'),
          nonce: findVal(auth.digest, 'nonce'),
          algorithm: findVal(auth.digest, 'algorithm') || 'MD5',
          qop: findVal(auth.digest, 'qop'),
          opaque: findVal(auth.digest, 'opaque'),
        };
      case 'awsv4':
        return {
          type: 'aws-sig-v4',
          accessKey: findVal(auth.awsv4, 'accessKey'),
          secretKey: findVal(auth.awsv4, 'secretKey'),
          region: findVal(auth.awsv4, 'region'),
          service: findVal(auth.awsv4, 'service'),
          sessionToken: findVal(auth.awsv4, 'sessionToken'),
        };
      case 'hawk':
        this.warnings.push('Hawk auth imported but not fully supported in Nexus.');
        return { type: 'none' };
      case 'noauth':
        return { type: 'none' };
      default:
        this.warnings.push(`Unknown auth type "${auth.type}" — defaulting to no auth.`);
        return { type: 'none' };
    }
  }

  /**
   * Maps a Postman variable entry to a Nexus {@link Variable} (including secret typing).
   *
   * @param v - Postman variable from collection, environment, or URL.
   * @returns Nexus variable with key, value, enabled flag, type, and optional description.
   */
  private convertVariable(v: PostmanVariable): Variable {
    return {
      key: v.key,
      value: v.value ?? '',
      type: v.type === 'secret' ? 'secret' : 'string',
      enabled: !v.disabled,
      description: v.description,
    };
  }

  /**
   * Selects a Postman event by `listen`, joins its script lines, and transpiles Postman script to Nexus script.
   *
   * @param events - Postman `event` array from collection, folder, or request, or undefined.
   * @param listen - Event name to match (`prerequest` or `test`).
   * @returns Transpiled script source, or empty string when no script is present.
   */
  private extractScript(events: PostmanEvent[] | undefined, listen: string): string {
    if (!events) return '';
    const event = events.find((e) => e.listen === listen);
    if (!event?.script?.exec) return '';
    const rawScript = event.script.exec.join('\n');
    if (!rawScript.trim()) return '';

    const { script, warnings } = transpilePostmanToNexus(rawScript);
    this.warnings.push(...warnings.map((w) => `[Script] ${w}`));
    return script;
  }
}
