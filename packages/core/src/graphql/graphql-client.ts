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

/**
 * HTTP GraphQL client with introspection caching and optional WebSocket subscriptions, built on `EventEmitter`.
 */
export class GraphQLClient extends EventEmitter {
  private schemas = new Map<string, GraphQLSchema>();
  private subscriptionClient: GqlWsClient | null = null;
  private subscriptionUnsubscribe: (() => void) | null = null;

  /**
   * Sends a GraphQL request over HTTP POST and returns the parsed body with timing and response headers.
   * @param opts - Endpoint URL, query, and optional variables, operation name, and request headers.
   * @returns The GraphQL payload, errors (if any), HTTP status, elapsed time, and response headers.
   */
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

  /**
   * Runs an introspection query against the endpoint and returns a `GraphQLSchema`, caching by URL.
   * @param url - GraphQL HTTP endpoint URL.
   * @param headers - Optional headers to send with the introspection request.
   * @returns A client schema built from the introspection result.
   */
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

  /**
   * Drops cached introspection schemas for a single URL or for all URLs when `url` is omitted.
   * @param url - Optional endpoint URL whose cache entry to remove.
   * @returns void
   */
  clearSchemaCache(url?: string): void {
    if (url) {
      this.schemas.delete(url);
    } else {
      this.schemas.clear();
    }
  }

  /**
   * Opens a WebSocket subscription (replacing any prior one) and emits subscription lifecycle events.
   * @param opts - HTTP(S) URL (converted to ws), query, optional variables, operation name, and connection headers.
   * @returns void
   */
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

  /**
   * Stops the active subscription and disposes the underlying `graphql-ws` client.
   * @returns void
   */
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

  /**
   * Fully tears down subscriptions and clears all cached schemas for this client.
   * @returns void
   */
  dispose(): void {
    this.unsubscribe();
    this.schemas.clear();
  }
}
