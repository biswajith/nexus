import type { NexusRequest, NexusFolder, NexusCollection, Variable } from '../types/index.js';
import { isFolder } from '../types/index.js';
import { ScriptSandbox } from './sandbox.js';
import type { ScriptContext, ScriptResult, ResponseAccess, MutableRequest } from './sandbox.js';

interface ScriptChainResult {
  results: ScriptResult[];
  request: MutableRequest;
}

export class ScriptRunner {
  private sandbox: ScriptSandbox;

  constructor(timeoutMs = 30_000) {
    this.sandbox = new ScriptSandbox(timeoutMs);
  }

  runPreRequestChain(
    collection: NexusCollection,
    request: NexusRequest,
    parentFolders: NexusFolder[],
    envVars: Variable[],
    globalVars: Variable[],
  ): ScriptChainResult {
    const envScope = this.sandbox.createTrackedScope('environment', envVars);
    const collScope = this.sandbox.createTrackedScope('collection', collection.variables);
    const globalScope = this.sandbox.createTrackedScope('global', globalVars);

    const mutableRequest: MutableRequest = {
      url: request.url,
      method: request.method,
      headers: Object.fromEntries(
        request.headers.filter((h) => h.enabled).map((h) => [h.key, h.value]),
      ),
      body: this.extractBody(request),
    };

    const results: ScriptResult[] = [];

    const scripts: string[] = [];
    if (collection.preRequestScript) scripts.push(collection.preRequestScript);
    for (const folder of parentFolders) {
      if (folder.preRequestScript) scripts.push(folder.preRequestScript);
    }
    if (request.preRequestScript) scripts.push(request.preRequestScript);

    for (const script of scripts) {
      const context: ScriptContext = {
        environment: envScope,
        collectionVariables: collScope,
        globals: globalScope,
        request: mutableRequest,
      };

      const result = this.sandbox.execute(script, context);
      results.push(result);

      if (result.requestMutations) {
        if (result.requestMutations.url !== undefined) mutableRequest.url = result.requestMutations.url;
        if (result.requestMutations.method !== undefined) mutableRequest.method = result.requestMutations.method;
        if (result.requestMutations.headers !== undefined) mutableRequest.headers = result.requestMutations.headers;
        if (result.requestMutations.body !== undefined) mutableRequest.body = result.requestMutations.body;
      }

      if (result.executionControl.skipped) break;
    }

    return { results, request: mutableRequest };
  }

  runPostResponseChain(
    collection: NexusCollection,
    request: NexusRequest,
    parentFolders: NexusFolder[],
    envVars: Variable[],
    globalVars: Variable[],
    response: ResponseAccess,
  ): ScriptResult[] {
    const envScope = this.sandbox.createTrackedScope('environment', envVars);
    const collScope = this.sandbox.createTrackedScope('collection', collection.variables);
    const globalScope = this.sandbox.createTrackedScope('global', globalVars);

    const mutableRequest: MutableRequest = {
      url: request.url,
      method: request.method,
      headers: Object.fromEntries(
        request.headers.filter((h) => h.enabled).map((h) => [h.key, h.value]),
      ),
      body: this.extractBody(request),
    };

    const results: ScriptResult[] = [];

    const scripts: string[] = [];
    if (request.postResponseScript) scripts.push(request.postResponseScript);
    for (const folder of [...parentFolders].reverse()) {
      if (folder.postResponseScript) scripts.push(folder.postResponseScript);
    }
    if (collection.postResponseScript) scripts.push(collection.postResponseScript);

    for (const script of scripts) {
      const context: ScriptContext = {
        environment: envScope,
        collectionVariables: collScope,
        globals: globalScope,
        request: mutableRequest,
        response,
      };

      const result = this.sandbox.execute(script, context);
      results.push(result);
    }

    return results;
  }

  findParentFolders(
    collection: NexusCollection,
    requestId: string,
  ): NexusFolder[] {
    const path: NexusFolder[] = [];

    function search(items: (NexusRequest | NexusFolder)[], trail: NexusFolder[]): boolean {
      for (const item of items) {
        if (isFolder(item)) {
          if (search(item.items, [...trail, item])) return true;
        } else if (item.id === requestId) {
          path.push(...trail);
          return true;
        }
      }
      return false;
    }

    search(collection.items, []);
    return path;
  }

  private extractBody(request: NexusRequest): string | null {
    switch (request.body.mode) {
      case 'json':
      case 'xml':
      case 'text':
      case 'html':
        return request.body.raw ?? null;
      case 'x-www-form-urlencoded':
        if (!request.body.urlencoded) return null;
        return request.body.urlencoded
          .filter((p) => p.enabled)
          .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
          .join('&');
      case 'graphql':
        if (!request.body.graphql) return null;
        return JSON.stringify({
          query: request.body.graphql.query,
          variables: request.body.graphql.variables
            ? JSON.parse(request.body.graphql.variables)
            : undefined,
        });
      default:
        return null;
    }
  }
}
