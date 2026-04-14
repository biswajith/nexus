import type { NexusRequest, NexusFolder, NexusCollection, Variable } from '../types/index.js';
import { isFolder } from '../types/index.js';
import { ScriptSandbox } from './sandbox.js';
import type { ScriptContext, ScriptResult, ResponseAccess, MutableRequest } from './sandbox.js';

interface ScriptChainResult {
  results: ScriptResult[];
  request: MutableRequest;
}

/**
 * Executes pre-request and post-response script chains in a sandbox, merging variable scopes
 * and applying request mutations where scripts allow.
 */
export class ScriptRunner {
  private sandbox: ScriptSandbox;

  /**
   * Creates a runner that uses a script sandbox with the given execution timeout.
   * @param timeoutMs - Maximum time in milliseconds for a single script execution (default 30_000).
   */
  constructor(timeoutMs = 30_000) {
    this.sandbox = new ScriptSandbox(timeoutMs);
  }

  /**
   * Runs pre-request scripts in order (collection → each parent folder → request), applying
   * mutations to a mutable request and stopping early if a script sets execution to skipped.
   * @param collection - Collection whose pre-request script and variables participate in scope.
   * @param request - Request whose pre-request script runs last; its body is copied for mutation.
   * @param parentFolders - Ancestor folders from root toward the request, in traversal order.
   * @param envVars - Environment variables exposed as the environment scope.
   * @param globalVars - Global variables exposed as the globals scope.
   * @returns Per-script results and the final mutable request after the chain (or early exit).
   */
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

  /**
   * Runs post-response scripts in order (request → parent folders inner-to-outer → collection)
   * with shared variable scopes, a mutable request snapshot, and the given response.
   * @param collection - Collection whose post-response script runs last; its variables are in scope.
   * @param request - Request whose post-response script runs first; used for mutable request context.
   * @param parentFolders - Ancestor folders from root toward the request (reversed for execution order).
   * @param envVars - Environment variables exposed as the environment scope.
   * @param globalVars - Global variables exposed as the globals scope.
   * @param response - Response data passed into each post-response script.
   * @returns Results from each executed script, in execution order.
   */
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

  /**
   * Traverses the collection tree to locate a request by id and returns its folder ancestry.
   * @param collection - Collection whose `items` tree is searched.
   * @param requestId - Id of the request whose parent folders are needed.
   * @returns Folders from root to the request's immediate parent, in order; empty if not found.
   */
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

  /**
   * Serializes the request body for script context according to `request.body.mode`.
   * @param request - Request whose body mode and payload determine the returned string.
   * @returns Encoded body string, or `null` when there is no applicable body content.
   */
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
