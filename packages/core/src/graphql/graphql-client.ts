import { EventEmitter } from 'node:events';
import { request as undiciRequest } from 'undici';
import {
  buildClientSchema,
  getIntrospectionQuery,
  type GraphQLSchema,
  type IntrospectionQuery,
} from 'graphql';
import WebSocket from 'ws';
import { createClient, type Client as GqlWsClient } from 'graphql-ws';

export interface GraphQLRequestOptions {
  url: string;
  query: string;
  variables?: Record<string, unknown>;
  operationName?: string;
  headers?: Record<string, string>;
}

export interface GraphQLResponse {
  data?: unknown;
  errors?: Array<{
    message: string;
    locations?: Array<{ line: number; column: number }>;
    path?: Array<string | number>;
    extensions?: Record<string, unknown>;
  }>;
  extensions?: Record<string, unknown>;
  status: number;
  responseTime: number;
  headers: Record<string, string>;
}

export interface SubscriptionMessage {
  id: string;
  timestamp: number;
  data?: unknown;
  errors?: Array<{ message: string }>;
}

export class GraphQLClient extends EventEmitter {
  private schemas = new Map<string, GraphQLSchema>();
  private subscriptionClient: GqlWsClient | null = null;
  private subscriptionUnsubscribe: (() => void) | null = null;

  async send(opts: GraphQLRequestOptions): Promise<GraphQLResponse> {
    const startTime = Date.now();
    const body = JSON.stringify({
      query: opts.query,
      variables: opts.variables,
      operationName: opts.operationName,
    });

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Accept: 'application/json',
      ...opts.headers,
    };

    const response = await undiciRequest(opts.url, {
      method: 'POST',
      headers,
      body,
    });

    const responseTime = Date.now() - startTime;
    const responseBody = await response.body.text();
    const responseHeaders: Record<string, string> = {};
    for (const [key, value] of Object.entries(response.headers)) {
      if (value) responseHeaders[key] = Array.isArray(value) ? value.join(', ') : value;
    }

    let parsed: {
      data?: unknown;
      errors?: GraphQLResponse['errors'];
      extensions?: Record<string, unknown>;
    };
    try {
      parsed = JSON.parse(responseBody) as typeof parsed;
    } catch {
      parsed = { errors: [{ message: `Invalid JSON response: ${responseBody.slice(0, 200)}` }] };
    }

    return {
      data: parsed.data,
      errors: parsed.errors,
      extensions: parsed.extensions,
      status: response.statusCode,
      responseTime,
      headers: responseHeaders,
    };
  }

  async introspect(url: string, headers?: Record<string, string>): Promise<GraphQLSchema> {
    const cached = this.schemas.get(url);
    if (cached) return cached;

    const response = await this.send({
      url,
      query: getIntrospectionQuery(),
      headers,
    });

    if (response.errors?.length) {
      throw new Error(`Introspection failed: ${response.errors.map((e) => e.message).join(', ')}`);
    }

    if (!response.data) {
      throw new Error('Introspection returned no data');
    }

    const schema = buildClientSchema(response.data as IntrospectionQuery);
    this.schemas.set(url, schema);
    return schema;
  }

  clearSchemaCache(url?: string): void {
    if (url) {
      this.schemas.delete(url);
    } else {
      this.schemas.clear();
    }
  }

  subscribe(opts: {
    url: string;
    query: string;
    variables?: Record<string, unknown>;
    operationName?: string;
    headers?: Record<string, string>;
  }): void {
    this.unsubscribe();

    const wsUrl = opts.url.replace(/^http/, 'ws');

    this.subscriptionClient = createClient({
      url: wsUrl,
      connectionParams: opts.headers,
      webSocketImpl: WebSocket as unknown as typeof globalThis.WebSocket,
    });

    const messages: SubscriptionMessage[] = [];

    this.subscriptionUnsubscribe = this.subscriptionClient.subscribe(
      {
        query: opts.query,
        variables: opts.variables,
        operationName: opts.operationName,
      },
      {
        next: (value) => {
          const msg: SubscriptionMessage = {
            id: crypto.randomUUID(),
            timestamp: Date.now(),
            data: value.data,
            errors: value.errors as SubscriptionMessage['errors'],
          };
          messages.push(msg);
          this.emit('subscription-data', msg);
        },
        error: (err) => {
          this.emit('subscription-error', err);
        },
        complete: () => {
          this.emit('subscription-complete', { messageCount: messages.length });
        },
      },
    );
  }

  unsubscribe(): void {
    if (this.subscriptionUnsubscribe) {
      this.subscriptionUnsubscribe();
      this.subscriptionUnsubscribe = null;
    }
    if (this.subscriptionClient) {
      this.subscriptionClient.dispose();
      this.subscriptionClient = null;
    }
  }

  dispose(): void {
    this.unsubscribe();
    this.schemas.clear();
  }
}
