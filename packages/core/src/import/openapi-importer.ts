import { parse as parseYaml } from 'yaml';
import type {
  NexusCollection, NexusRequest, NexusFolder,
  HttpMethod, KeyValuePair, RequestBody,
} from '../types/index.js';
import type { ImportResult } from './types.js';

const HTTP_METHODS = ['get', 'post', 'put', 'patch', 'delete', 'head', 'options'] as const;

interface OpenApiSpec {
  openapi?: string;
  swagger?: string;
  info: { title: string; description?: string; version: string };
  servers?: Array<{ url: string; description?: string }>;
  host?: string;
  basePath?: string;
  schemes?: string[];
  paths?: Record<string, PathItem>;
  components?: { schemas?: Record<string, SchemaObject>; securitySchemes?: Record<string, unknown> };
  definitions?: Record<string, SchemaObject>;
  tags?: Array<{ name: string; description?: string }>;
}

interface PathItem {
  [method: string]: OperationObject | unknown;
  parameters?: ParameterObject[];
}

interface OperationObject {
  summary?: string;
  description?: string;
  operationId?: string;
  tags?: string[];
  parameters?: ParameterObject[];
  requestBody?: RequestBodyObject;
  responses?: Record<string, ResponseObject>;
  security?: Array<Record<string, string[]>>;
  deprecated?: boolean;
  consumes?: string[];
}

interface ParameterObject {
  name: string;
  in: 'query' | 'header' | 'path' | 'cookie' | 'body' | 'formData';
  description?: string;
  required?: boolean;
  schema?: SchemaObject;
  type?: string;
  example?: unknown;
  default?: unknown;
}

interface RequestBodyObject {
  description?: string;
  required?: boolean;
  content?: Record<string, { schema?: SchemaObject; example?: unknown }>;
}

interface ResponseObject {
  description?: string;
  content?: Record<string, { schema?: SchemaObject; example?: unknown }>;
  schema?: SchemaObject;
}

interface SchemaObject {
  type?: string;
  properties?: Record<string, SchemaObject>;
  items?: SchemaObject;
  required?: string[];
  example?: unknown;
  enum?: unknown[];
  $ref?: string;
  format?: string;
  description?: string;
  default?: unknown;
}

/**
 * Parses OpenAPI 2 (Swagger) or OpenAPI 3 documents and turns them into Nexus collections,
 * optional per-server environments, and import warnings.
 */
export class OpenApiImporter {
  private warnings: string[] = [];
  private spec!: OpenApiSpec;

  /**
   * Parses a JSON or YAML OpenAPI document and returns collections, environments, and warnings.
   * @param data Raw spec string (JSON object or YAML).
   * @returns Parsed import result with one collection, derived environments, and accumulated warnings.
   */
  import(data: string): ImportResult {
    this.warnings = [];

    const trimmed = data.trim();
    if (trimmed.startsWith('{')) {
      this.spec = JSON.parse(trimmed) as OpenApiSpec;
    } else {
      this.spec = parseYaml(trimmed) as OpenApiSpec;
    }

    const isSwagger2 = !!this.spec.swagger;
    const baseUrl = this.resolveBaseUrl(isSwagger2);

    const collection: NexusCollection = {
      id: `col_${crypto.randomUUID().slice(0, 8)}`,
      name: this.spec.info.title,
      description: this.spec.info.description,
      variables: [
        { key: 'base_url', value: baseUrl, type: 'string', enabled: true, description: 'API base URL' },
      ],
      items: this.buildItems(isSwagger2),
    };

    return { collections: [collection], environments: this.buildEnvironments(), warnings: [...this.warnings] };
  }

  /**
   * Derives the default API base URL from Swagger 2 host/schemes/basePath or OpenAPI 3 servers.
   * @param isSwagger2 Whether the spec is Swagger 2 (`swagger` field set) vs OpenAPI 3.
   * @returns Resolved base URL string (with sensible fallbacks if fields are missing).
   */
  private resolveBaseUrl(isSwagger2: boolean): string {
    if (isSwagger2) {
      const scheme = this.spec.schemes?.[0] ?? 'https';
      const host = this.spec.host ?? 'localhost';
      const basePath = this.spec.basePath ?? '';
      return `${scheme}://${host}${basePath}`;
    }
    return this.spec.servers?.[0]?.url ?? 'http://localhost';
  }

  /**
   * Builds extra environments when the spec lists multiple servers (each maps `base_url` to that server).
   * @returns Environments for additional servers, or an empty array if none or only one server.
   */
  private buildEnvironments(): import('../types/index.js').NexusEnvironment[] {
    if (!this.spec.servers || this.spec.servers.length <= 1) return [];

    return this.spec.servers.map((server, i) => ({
      id: crypto.randomUUID(),
      name: server.description ?? `Server ${i + 1}`,
      variables: [
        { key: 'base_url', value: server.url, type: 'string' as const, enabled: true, description: server.description },
      ],
    }));
  }

  /**
   * Walks all paths and HTTP operations, builds requests, and groups them into tag folders when present.
   * @param isSwagger2 Whether to use Swagger 2 body/parameter rules when building each request.
   * @returns Folders (one per tag) followed by untagged requests, preserving discovery order.
   */
  private buildItems(isSwagger2: boolean): (NexusRequest | NexusFolder)[] {
    const paths = this.spec.paths ?? {};
    const tagMap = new Map<string, (NexusRequest | NexusFolder)[]>();
    const untagged: NexusRequest[] = [];

    for (const [pathStr, pathItem] of Object.entries(paths)) {
      const pathParams = (pathItem as PathItem).parameters ?? [];

      for (const method of HTTP_METHODS) {
        const operation = (pathItem as Record<string, unknown>)[method] as OperationObject | undefined;
        if (!operation) continue;

        const request = this.buildRequest(pathStr, method, operation, pathParams, isSwagger2);

        const tags = operation.tags ?? [];
        if (tags.length === 0) {
          untagged.push(request);
        } else {
          for (const tag of tags) {
            if (!tagMap.has(tag)) tagMap.set(tag, []);
            tagMap.get(tag)!.push(request);
          }
        }
      }
    }

    const folders: NexusFolder[] = [];
    for (const [tag, items] of tagMap) {
      const tagInfo = this.spec.tags?.find((t) => t.name === tag);
      folders.push({
        id: `folder_${crypto.randomUUID().slice(0, 8)}`,
        name: tag,
        description: tagInfo?.description,
        items,
      });
    }

    return [...folders, ...untagged];
  }

  /**
   * Converts a single path operation into a `NexusRequest` (URL, query, headers, body, metadata).
   * @param path OpenAPI path template (e.g. `/users/{id}`).
   * @param method Lowercase HTTP method name.
   * @param operation Operation object for that path and method.
   * @param pathParams Path-level parameters merged with operation parameters.
   * @param isSwagger2 Whether to build the body using Swagger 2 vs OpenAPI 3 rules.
   * @returns A fully populated Nexus request object.
   */
  private buildRequest(
    path: string,
    method: string,
    operation: OperationObject,
    pathParams: ParameterObject[],
    isSwagger2: boolean,
  ): NexusRequest {
    const allParams = [...pathParams, ...(operation.parameters ?? [])];
    const nexusUrl = `{{base_url}}${path.replace(/\{([^}]+)\}/g, ':$1')}`;

    const queryParams: KeyValuePair[] = allParams
      .filter((p) => p.in === 'query')
      .map((p) => ({
        key: p.name,
        value: this.exampleValue(p),
        enabled: !!p.required,
        description: p.description,
      }));

    const headers: KeyValuePair[] = allParams
      .filter((p) => p.in === 'header')
      .map((p) => ({
        key: p.name,
        value: this.exampleValue(p),
        enabled: !!p.required,
        description: p.description,
      }));

    const body = isSwagger2
      ? this.buildBodySwagger2(allParams, operation)
      : this.buildBodyOpenApi3(operation);

    if (body.mode === 'json' && !headers.some((h) => h.key.toLowerCase() === 'content-type')) {
      headers.push({ key: 'Content-Type', value: 'application/json', enabled: true });
    }

    const name = operation.summary ?? operation.operationId ?? `${method.toUpperCase()} ${path}`;

    return {
      id: `req_${crypto.randomUUID().slice(0, 8)}`,
      name: operation.deprecated ? `[DEPRECATED] ${name}` : name,
      method: method.toUpperCase() as HttpMethod,
      url: nexusUrl,
      headers,
      params: queryParams,
      body,
      auth: { type: 'inherit' },
      settings: {},
      ...(operation.description ? { description: operation.description } : {}),
    } as NexusRequest;
  }

  /**
   * Maps an OpenAPI 3 `requestBody` to a Nexus `RequestBody` (JSON, forms, XML, text, or none).
   * @param operation Operation containing optional `requestBody.content`.
   * @returns Body mode and payload suitable for the first supported content type found.
   */
  private buildBodyOpenApi3(operation: OperationObject): RequestBody {
    if (!operation.requestBody?.content) return { mode: 'none' };

    const content = operation.requestBody.content;

    if (content['application/json']) {
      const schema = content['application/json'].schema;
      const example = content['application/json'].example;
      const body = example ?? (schema ? this.generateExample(schema) : {});
      return { mode: 'json', raw: JSON.stringify(body, null, 2) };
    }

    if (content['application/x-www-form-urlencoded']) {
      const schema = content['application/x-www-form-urlencoded'].schema;
      const pairs = this.schemaToKeyValuePairs(schema);
      return { mode: 'x-www-form-urlencoded', urlencoded: pairs };
    }

    if (content['multipart/form-data']) {
      const schema = content['multipart/form-data'].schema;
      const pairs = this.schemaToKeyValuePairs(schema);
      return {
        mode: 'form-data',
        formData: pairs.map((p) => ({ ...p, type: 'text' as const })),
      };
    }

    if (content['application/xml'] || content['text/xml']) {
      return { mode: 'xml', raw: '<!-- XML body -->' };
    }

    return { mode: 'text', raw: '' };
  }

  /**
   * Maps Swagger 2 body, formData, and consumes to a Nexus `RequestBody`.
   * @param params All parameters for the operation (including path-level).
   * @param operation Operation object (used for `consumes` when choosing multipart vs urlencoded).
   * @returns JSON, urlencoded, multipart, or empty body depending on parameters and consumes.
   */
  private buildBodySwagger2(params: ParameterObject[], operation: OperationObject): RequestBody {
    const bodyParam = params.find((p) => p.in === 'body');
    if (!bodyParam?.schema) {
      const formParams = params.filter((p) => p.in === 'formData');
      if (formParams.length > 0) {
        const consumes = operation.consumes ?? [];
        if (consumes.includes('multipart/form-data')) {
          return {
            mode: 'form-data',
            formData: formParams.map((p) => ({
              key: p.name,
              value: this.exampleValue(p),
              type: (p.type === 'file' ? 'file' : 'text') as 'text' | 'file',
              enabled: true,
              description: p.description,
            })),
          };
        }
        return {
          mode: 'x-www-form-urlencoded',
          urlencoded: formParams.map((p) => ({
            key: p.name,
            value: this.exampleValue(p),
            enabled: true,
            description: p.description,
          })),
        };
      }
      return { mode: 'none' };
    }

    const example = this.generateExample(bodyParam.schema);
    return { mode: 'json', raw: JSON.stringify(example, null, 2) };
  }

  /**
   * Turns an object schema’s `properties` into key/value pairs for urlencoded or multipart bodies.
   * @param schema Optional JSON Schema object with `properties` (and optional `required`).
   * @returns Key/value rows with enabled flags aligned to `required` when present.
   */
  private schemaToKeyValuePairs(schema?: SchemaObject): KeyValuePair[] {
    if (!schema?.properties) return [];
    const required = new Set(schema.required ?? []);
    return Object.entries(schema.properties).map(([key, prop]) => ({
      key,
      value: String(prop.example ?? prop.default ?? ''),
      enabled: required.has(key),
      description: prop.description,
    }));
  }

  /**
   * Builds a placeholder JSON value from a schema for examples (warns on unresolved `$ref`).
   * @param schema JSON Schema fragment to materialize.
   * @returns Example value: object, array, primitives, or defaults; empty object if `$ref` only.
   */
  private generateExample(schema: SchemaObject): unknown {
    if (schema.example !== undefined) return schema.example;
    if (schema.$ref) {
      this.warnings.push(`$ref "${schema.$ref}" not resolved — using empty object.`);
      return {};
    }

    switch (schema.type) {
      case 'object': {
        const obj: Record<string, unknown> = {};
        for (const [key, prop] of Object.entries(schema.properties ?? {})) {
          obj[key] = this.generateExample(prop);
        }
        return obj;
      }
      case 'array':
        return schema.items ? [this.generateExample(schema.items)] : [];
      case 'string':
        if (schema.enum) return schema.enum[0];
        if (schema.format === 'date') return '2026-01-01';
        if (schema.format === 'date-time') return '2026-01-01T00:00:00Z';
        if (schema.format === 'email') return 'user@example.com';
        if (schema.format === 'uri' || schema.format === 'url') return 'https://example.com';
        if (schema.format === 'uuid') return '550e8400-e29b-41d4-a716-446655440000';
        return schema.default ?? 'string';
      case 'integer':
      case 'number':
        return schema.default ?? 0;
      case 'boolean':
        return schema.default ?? true;
      default:
        return schema.default ?? null;
    }
  }

  /**
   * Resolves a string example for a parameter from explicit example/default or nested schema hints.
   * @param param OpenAPI parameter (query, header, path, etc.).
   * @returns Best-effort string for UI defaults; may be empty if nothing is defined.
   */
  private exampleValue(param: ParameterObject): string {
    if (param.example !== undefined) return String(param.example);
    if (param.default !== undefined) return String(param.default);
    if (param.schema?.example !== undefined) return String(param.schema.example);
    if (param.schema?.default !== undefined) return String(param.schema.default);
    if (param.schema?.enum?.[0] !== undefined) return String(param.schema.enum[0]);
    return '';
  }
}
