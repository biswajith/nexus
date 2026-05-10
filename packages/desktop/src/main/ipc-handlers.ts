import type { IpcMain } from 'electron';
import * as os from 'node:os';
import * as path from 'node:path';
import * as fs from 'node:fs/promises';
import {
  sendHttpRequest,
  CollectionManager,
  HistoryManager,
  VariableStore,
  resolveAuth,
  ScriptSandbox,
  TokenStore,
  PostmanImporter,
  OpenApiImporter,
  CurlImporter,
  NexusExporter,
  detectImportFormat,
  CollectionRunner,
  WebSocketClient,
  GraphQLClient,
  generateCurl,
  DocGenerator,
  Visualizer,
} from '@nexus/core';
import type {
  SendOptions, AuthConfig, Variable, ImportFormat,
  NexusCollection, NexusEnvironment, NexusRequest, RunConfig, WsConnectOptions,
  GraphQLRequestOptions,
} from '@nexus/core';

const NEXUS_HOME = path.join(os.homedir(), '.nexus');
const collectionManager = new CollectionManager(path.join(NEXUS_HOME, 'collections'));
const historyManager = new HistoryManager(path.join(NEXUS_HOME, 'history'));
const tokenStore = new TokenStore(path.join(NEXUS_HOME, 'tokens.json'));
const scriptSandbox = new ScriptSandbox(30_000);
const postmanImporter = new PostmanImporter();
const openApiImporter = new OpenApiImporter();
const curlImporter = new CurlImporter();
const nexusExporter = new NexusExporter();
const collectionRunner = new CollectionRunner();
const wsClients = new Map<string, WebSocketClient>();
// MCP client management
import { Client as McpClient } from '@modelcontextprotocol/sdk/client';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';

interface McpConnection {
  client: McpClient;
  transport: StdioClientTransport | StreamableHTTPClientTransport;
}
const mcpConnections = new Map<string, McpConnection>();
const graphqlClient = new GraphQLClient();

interface SendRequestPayload {
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: string | null;
  settings?: {
    followRedirects?: boolean;
    maxRedirects?: number;
    timeout?: number;
    rejectUnauthorized?: boolean;
  };
  auth?: AuthConfig;
  preRequestScript?: string;
  postResponseScript?: string;
  environmentVars?: Variable[];
  collectionVars?: Variable[];
  globalVars?: Variable[];
}

/**
 * Registers all `ipcMain.handle` channels so the renderer can invoke `@nexus/core` services on the main process.
 *
 * @param ipcMain - The Electron main-process `ipcMain` instance to register handlers on.
 * @returns void
 */
export function registerIpcHandlers(ipcMain: IpcMain): void {
  ipcMain.handle('http:send', async (_event, payload: SendRequestPayload) => {
    const envVars = payload.environmentVars ?? [];
    const collVars = payload.collectionVars ?? [];
    const globalVars = payload.globalVars ?? [];

    let requestUrl = payload.url;
    let requestHeaders = { ...payload.headers };
    let requestBody = payload.body ?? null;
    let requestMethod = payload.method;

    const variableStore = new VariableStore({
      global: globalVars,
      collection: collVars,
      environment: envVars,
    });

    requestUrl = variableStore.resolve(requestUrl);
    requestHeaders = variableStore.resolveHeaders(requestHeaders);
    if (requestBody) {
      requestBody = variableStore.resolve(requestBody);
    }

    const scriptLogs: Array<{ level: string; args: unknown[]; timestamp: number }> = [];

    if (payload.preRequestScript) {
      const envScope = scriptSandbox.createTrackedScope('environment', envVars);
      const collScope = scriptSandbox.createTrackedScope('collection', collVars);
      const globalScope = scriptSandbox.createTrackedScope('global', globalVars);

      const result = scriptSandbox.execute(payload.preRequestScript, {
        environment: envScope,
        collectionVariables: collScope,
        globals: globalScope,
        request: {
          url: requestUrl,
          method: requestMethod,
          headers: requestHeaders,
          body: requestBody,
        },
      });

      scriptLogs.push(...result.logs);

      if (result.requestMutations) {
        if (result.requestMutations.url) requestUrl = result.requestMutations.url;
        if (result.requestMutations.method) requestMethod = result.requestMutations.method;
        if (result.requestMutations.headers) requestHeaders = result.requestMutations.headers;
        if (result.requestMutations.body !== undefined) requestBody = result.requestMutations.body;
      }

      if (result.error) {
        return {
          status: 0,
          statusText: `Pre-request script error: ${result.error}`,
          headers: {},
          bodyText: result.error,
          timing: { dns: 0, tcp: 0, tls: 0, ttfb: 0, download: 0, total: 0 },
          size: 0,
          testResults: [],
          scriptLogs,
        };
      }
    }

    if (payload.auth && payload.auth.type !== 'none' && payload.auth.type !== 'inherit') {
      const prepared = await resolveAuth(payload.auth, {
        method: requestMethod,
        url: requestUrl,
        headers: requestHeaders,
        body: requestBody,
      });
      requestUrl = prepared.url;
      requestHeaders = prepared.headers;
    }

    const sendOpts: SendOptions = {
      method: requestMethod as SendOptions['method'],
      url: requestUrl,
      headers: requestHeaders,
      body: requestBody,
      settings: payload.settings,
    };

    const response = await sendHttpRequest(sendOpts);

    let testResults: Array<{ name: string; passed: boolean; error?: string; duration: number }> = [];
    let visualizerHtml: string | undefined;

    if (payload.postResponseScript) {
      const envScope = scriptSandbox.createTrackedScope('environment', envVars);
      const collScope = scriptSandbox.createTrackedScope('collection', collVars);
      const globalScope = scriptSandbox.createTrackedScope('global', globalVars);

      const parsedHeaders: Record<string, string | string[]> = {};
      for (const [k, v] of Object.entries(response.headers)) {
        if (v !== undefined) parsedHeaders[k] = v;
      }

      const visualizer = new Visualizer();

      const result = scriptSandbox.execute(payload.postResponseScript, {
        environment: envScope,
        collectionVariables: collScope,
        globals: globalScope,
        request: {
          url: requestUrl,
          method: requestMethod,
          headers: requestHeaders,
          body: requestBody,
        },
        response: {
          code: response.status,
          status: response.statusText,
          headers: parsedHeaders,
          responseTime: response.timing.total,
          body: response.bodyText,
          json: () => {
            try { return JSON.parse(response.bodyText); }
            catch { return null; }
          },
          text: () => response.bodyText,
        },
        visualizer: {
          set: (template: string, data: unknown) => visualizer.set(template, data),
        },
      });

      scriptLogs.push(...result.logs);
      testResults = result.testResults;

      const rendered = visualizer.renderWithWrapper();
      if (rendered) {
        visualizerHtml = rendered;
      }
    }

    return {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
      bodyText: response.bodyText,
      bodyJson: response.bodyJson,
      timing: response.timing,
      size: response.size,
      testResults,
      scriptLogs,
      visualizerHtml,
    };
  });

  ipcMain.handle('variables:resolve', async (_event, template: string, scopes: {
    global?: Variable[];
    collection?: Variable[];
    environment?: Variable[];
  }) => {
    const store = new VariableStore(scopes);
    return store.resolve(template);
  });

  ipcMain.handle('collections:list', async () => {
    return collectionManager.listCollections();
  });

  ipcMain.handle('collections:load', async (_event, dirName: string) => {
    return collectionManager.loadCollection(dirName);
  });

  ipcMain.handle('collections:create', async (_event, collection) => {
    return collectionManager.createCollection(collection);
  });

  ipcMain.handle('collections:delete', async (_event, dirName: string) => {
    return collectionManager.deleteCollection(dirName);
  });

  ipcMain.handle('collections:addRequest', async (_event, dirName: string, request: NexusRequest) => {
    return collectionManager.addRequest(dirName, request);
  });

  ipcMain.handle('collections:updateRequest', async (_event, dirName: string, requestId: string, request: NexusRequest) => {
    return collectionManager.updateRequest(dirName, requestId, request);
  });

  ipcMain.handle('collections:deleteRequest', async (_event, dirName: string, requestId: string) => {
    return collectionManager.deleteRequest(dirName, requestId);
  });

  ipcMain.handle('collections:rename', async (_event, dirName: string, newName: string) => {
    const collectionDir = path.join(NEXUS_HOME, 'collections', dirName);
    const metaPath = path.join(collectionDir, 'collection.json');
    const data = JSON.parse(await fs.readFile(metaPath, 'utf-8'));
    data.name = newName;
    await fs.writeFile(metaPath, JSON.stringify(data, null, 2), 'utf-8');
  });

  // Environment CRUD
  ipcMain.handle('environments:list', async () => {
    const envDir = path.join(NEXUS_HOME, 'environments');
    await fs.mkdir(envDir, { recursive: true });
    try {
      const files = await fs.readdir(envDir);
      const environments: NexusEnvironment[] = [];
      for (const file of files) {
        if (!file.endsWith('.json')) continue;
        const data = await fs.readFile(path.join(envDir, file), 'utf-8');
        environments.push(JSON.parse(data) as NexusEnvironment);
      }
      return environments;
    } catch {
      return [];
    }
  });

  ipcMain.handle('environments:create', async (_event, env: NexusEnvironment) => {
    const envDir = path.join(NEXUS_HOME, 'environments');
    await fs.mkdir(envDir, { recursive: true });
    await fs.writeFile(path.join(envDir, `${env.id}.json`), JSON.stringify(env, null, 2), 'utf-8');
    return env;
  });

  ipcMain.handle('environments:update', async (_event, env: NexusEnvironment) => {
    const envDir = path.join(NEXUS_HOME, 'environments');
    await fs.writeFile(path.join(envDir, `${env.id}.json`), JSON.stringify(env, null, 2), 'utf-8');
    return env;
  });

  ipcMain.handle('environments:delete', async (_event, envId: string) => {
    const envDir = path.join(NEXUS_HOME, 'environments');
    const filePath = path.join(envDir, `${envId}.json`);
    await fs.rm(filePath, { force: true });
  });

  ipcMain.handle('history:log', async (_event, entry) => {
    return historyManager.log(entry);
  });

  ipcMain.handle('history:query', async (_event, filters) => {
    return historyManager.query(filters);
  });

  ipcMain.handle('history:clear', async (_event, before?: string) => {
    return historyManager.clear(before ? new Date(before) : undefined);
  });

  ipcMain.handle('tokens:get', async (_event, key: string) => {
    return tokenStore.getToken(key);
  });

  ipcMain.handle('tokens:save', async (_event, key: string, token: unknown) => {
    return tokenStore.saveToken(key, token as Parameters<typeof tokenStore.saveToken>[1]);
  });

  ipcMain.handle('tokens:remove', async (_event, key: string) => {
    return tokenStore.removeToken(key);
  });

  ipcMain.handle('import:detect-format', async (_event, data: string) => {
    return detectImportFormat(data);
  });

  ipcMain.handle('import:file', async (_event, data: string, format?: ImportFormat) => {
    const detectedFormat = format ?? detectImportFormat(data);

    switch (detectedFormat) {
      case 'postman-collection-v2.1': {
        const parsed = JSON.parse(data);
        return postmanImporter.importCollection(parsed);
      }
      case 'postman-environment': {
        const parsed = JSON.parse(data);
        return postmanImporter.importEnvironment(parsed);
      }
      case 'openapi-3':
      case 'swagger-2':
        return openApiImporter.import(data);
      case 'curl': {
        const request = curlImporter.import(data);
        return {
          collections: [{
            id: `col_${crypto.randomUUID().slice(0, 8)}`,
            name: 'Imported cURL Request',
            variables: [],
            items: [request],
          }],
          environments: [],
          warnings: [],
        };
      }
      case 'nexus': {
        const parsed = JSON.parse(data) as NexusCollection;
        return { collections: [parsed], environments: [], warnings: [] };
      }
      default:
        throw new Error(`Unsupported import format: ${detectedFormat}`);
    }
  });

  ipcMain.handle('import:save', async (_event, collections: NexusCollection[], environments: NexusEnvironment[]) => {
    const results: string[] = [];

    for (const collection of collections) {
      await collectionManager.createCollection(collection);
      results.push(`Collection "${collection.name}" imported`);
    }

    const envDir = path.join(NEXUS_HOME, 'environments');
    await fs.mkdir(envDir, { recursive: true });
    for (const env of environments) {
      await fs.writeFile(path.join(envDir, `${env.id}.json`), JSON.stringify(env, null, 2), 'utf-8');
      results.push(`Environment "${env.name}" imported`);
    }

    return results;
  });

  ipcMain.handle('import:curl', async (_event, curlCommand: string) => {
    return curlImporter.import(curlCommand);
  });

  ipcMain.handle('export:collection', async (_event, collection: NexusCollection, format: 'nexus' | 'postman-v2.1') => {
    if (format === 'postman-v2.1') {
      return nexusExporter.exportAsPostmanV21(collection);
    }
    return nexusExporter.exportAsNexusJson(collection);
  });

  ipcMain.handle('export:environment', async (_event, environment: NexusEnvironment, format: 'nexus' | 'postman') => {
    if (format === 'postman') {
      return nexusExporter.exportEnvironmentAsPostman(environment);
    }
    return JSON.stringify(environment, null, 2);
  });

  // Collection Runner
  ipcMain.handle('runner:start', async (event, config: RunConfig) => {
    collectionRunner.removeAllListeners();
    collectionRunner.on('event', (runEvent) => {
      event.sender.send('runner:event', runEvent);
    });
    return collectionRunner.run(config);
  });

  ipcMain.handle('runner:cancel', async () => {
    collectionRunner.cancel();
  });

  // WebSocket
  ipcMain.handle('ws:connect', async (event, id: string, opts: WsConnectOptions) => {
    const existing = wsClients.get(id);
    if (existing) existing.disconnect();

    const client = new WebSocketClient();
    wsClients.set(id, client);

    client.on('status', (status) => event.sender.send('ws:status', { id, status }));
    client.on('message', (msg) => event.sender.send('ws:message', { id, message: msg }));
    client.on('close', (info) => event.sender.send('ws:close', { id, ...info }));
    client.on('error', (err) => event.sender.send('ws:error', { id, error: String(err) }));

    client.connect(opts);
  });

  ipcMain.handle('ws:send', async (_event, id: string, data: string) => {
    const client = wsClients.get(id);
    if (!client) throw new Error(`No WebSocket connection with id: ${id}`);
    return client.send(data);
  });

  ipcMain.handle('ws:disconnect', async (_event, id: string) => {
    const client = wsClients.get(id);
    if (client) {
      client.disconnect();
      wsClients.delete(id);
    }
  });

  ipcMain.handle('ws:messages', async (_event, id: string) => {
    return wsClients.get(id)?.getMessages() ?? [];
  });

  // MCP Tester
  ipcMain.handle('mcp:connect', async (_event, id: string, config: {
    type: 'stdio' | 'http';
    command?: string;
    args?: string[];
    env?: Record<string, string>;
    url?: string;
  }) => {
    const existing = mcpConnections.get(id);
    if (existing) {
      try { await existing.client.close(); } catch { /* ignore */ }
      mcpConnections.delete(id);
    }

    const client = new McpClient({ name: 'nexus-mcp-tester', version: '0.1.0' });

    let transport: StdioClientTransport | StreamableHTTPClientTransport;
    if (config.type === 'stdio') {
      if (!config.command) throw new Error('Command is required for stdio transport');
      transport = new StdioClientTransport({
        command: config.command,
        args: config.args ?? [],
        env: config.env,
      });
    } else {
      if (!config.url) throw new Error('URL is required for HTTP transport');
      transport = new StreamableHTTPClientTransport(new URL(config.url));
    }

    await client.connect(transport);

    mcpConnections.set(id, { client, transport });

    const serverInfo = client.getServerVersion();
    const capabilities = client.getServerCapabilities();
    return { serverInfo, capabilities };
  });

  ipcMain.handle('mcp:listTools', async (_event, id: string) => {
    const conn = mcpConnections.get(id);
    if (!conn) throw new Error('Not connected to MCP server');
    const result = await conn.client.listTools();
    return result.tools;
  });

  ipcMain.handle('mcp:callTool', async (_event, id: string, toolName: string, args: Record<string, unknown>) => {
    const conn = mcpConnections.get(id);
    if (!conn) throw new Error('Not connected to MCP server');
    const result = await conn.client.callTool({ name: toolName, arguments: args });
    return result;
  });

  ipcMain.handle('mcp:listResources', async (_event, id: string) => {
    const conn = mcpConnections.get(id);
    if (!conn) throw new Error('Not connected to MCP server');
    const result = await conn.client.listResources();
    return result.resources;
  });

  ipcMain.handle('mcp:listPrompts', async (_event, id: string) => {
    const conn = mcpConnections.get(id);
    if (!conn) throw new Error('Not connected to MCP server');
    const result = await conn.client.listPrompts();
    return result.prompts;
  });

  ipcMain.handle('mcp:disconnect', async (_event, id: string) => {
    const conn = mcpConnections.get(id);
    if (conn) {
      try { await conn.client.close(); } catch { /* ignore */ }
      mcpConnections.delete(id);
    }
  });

  // GraphQL
  ipcMain.handle('graphql:send', async (_event, opts: GraphQLRequestOptions) => {
    return graphqlClient.send(opts);
  });

  ipcMain.handle('graphql:introspect', async (_event, url: string, headers?: Record<string, string>) => {
    const schema = await graphqlClient.introspect(url, headers);
    const { printSchema } = await import('graphql');
    return printSchema(schema);
  });

  ipcMain.handle('graphql:subscribe', async (event, opts: {
    url: string; query: string; variables?: Record<string, unknown>;
    operationName?: string; headers?: Record<string, string>;
  }) => {
    graphqlClient.removeAllListeners();
    graphqlClient.on('subscription-data', (msg) => event.sender.send('graphql:subscription-data', msg));
    graphqlClient.on('subscription-error', (err) => event.sender.send('graphql:subscription-error', String(err)));
    graphqlClient.on('subscription-complete', (info) => event.sender.send('graphql:subscription-complete', info));
    graphqlClient.subscribe(opts);
  });

  ipcMain.handle('graphql:unsubscribe', async () => {
    graphqlClient.unsubscribe();
  });

  // Code generation
  ipcMain.handle('codegen:curl', async (_event, request: NexusRequest) => {
    return generateCurl(request);
  });

  // Documentation generation
  const docGenerator = new DocGenerator();

  ipcMain.handle('docs:markdown', async (_event, collection: NexusCollection) => {
    return docGenerator.generateMarkdown(collection);
  });

  ipcMain.handle('docs:html', async (_event, collection: NexusCollection) => {
    return docGenerator.generateHtml(collection);
  });
}
