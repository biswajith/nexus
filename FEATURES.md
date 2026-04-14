# Nexus — Postman Replacement Feature Spec & Implementation Plan

> A local-first, offline API development tool built with TypeScript.
> All collections and environments are stored on disk — no cloud sync, no accounts required.

---

## Table of Contents

1. [Core Architecture](#1-core-architecture)
2. [Request Builder & Sender](#2-request-builder--sender)
3. [Collections & Folders](#3-collections--folders)
4. [Environment & Variable System](#4-environment--variable-system)
5. [Authentication](#5-authentication)
6. [Pre-Request & Post-Response Scripts](#6-pre-request--post-response-scripts)
7. [Test Assertions](#7-test-assertions)
8. [Collection Runner](#8-collection-runner)
9. [GraphQL Client](#9-graphql-client)
10. [WebSocket Client](#10-websocket-client)
11. [Server-Sent Events (SSE) Client](#11-server-sent-events-sse-client)
12. [gRPC Client](#12-grpc-client)
13. [Response Viewer](#13-response-viewer)
14. [Cookie Management](#14-cookie-management)
15. [Proxy & Traffic Capture](#15-proxy--traffic-capture)
16. [Mock Server](#16-mock-server)
17. [Code Snippet Generation](#17-code-snippet-generation)
18. [Import & Export](#18-import--export)
19. [Request History](#19-request-history)
20. [API Documentation Generation](#20-api-documentation-generation)
21. [CLI Runner (Newman Equivalent)](#21-cli-runner-newman-equivalent)
22. [Response Visualization](#22-response-visualization)
23. [Certificate & SSL Management](#23-certificate--ssl-management)
24. [Console & Debugging](#24-console--debugging)

---

## 1. Core Architecture

**What it is:** The foundational design of the entire application — how code is structured, how data flows, and how the UI renders.

**Why it matters:** Every feature below depends on a solid, extensible architecture. Getting this right means features are easy to add and the app stays fast.

### Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Runtime | Node.js 20+ | Native `fetch` via undici, ESM support, stable LTS |
| Language | TypeScript (strict mode) | Type safety across the entire stack |
| Desktop Framework | Electron (or Tauri for lighter binary) | Cross-platform, full OS access for file I/O and proxy |
| UI Framework | React 18+ with Vite | Fast HMR, rich ecosystem, component model |
| State Management | Zustand | Lightweight, TypeScript-friendly, no boilerplate |
| Styling | Tailwind CSS + Radix UI primitives | Utility-first, accessible headless components |
| Data Storage | Local filesystem (JSON/YAML files) | Git-friendly, no database needed, human-readable |
| HTTP Engine | `undici` (Node.js official HTTP client) | Fastest Node.js HTTP client, powers native `fetch`, connection pooling |
| Scripting Sandbox | `vm2` / `isolated-vm` | Secure JS execution for pre-request and test scripts |
| IPC | Electron IPC (main ↔ renderer) | Secure bridge between UI and Node.js backend |

### File-Based Storage Layout

```
~/.nexus/                          # Global config
  settings.json
  history/
    2026-04-14.jsonl               # Daily JSONL history logs

<project>/
  .nexus/
    collections/
      my-api/
        collection.json            # Collection metadata
        requests/
          get-users.json
          create-user.json
        folders/
          admin/
            get-admin.json
    environments/
      dev.json
      staging.json
      prod.json
    globals.json                   # Global variables
    scripts/
      shared-auth.js               # Reusable script modules
```

### Implementation Plan

```
nexus/
├── packages/
│   ├── core/                      # Pure TypeScript engine (no UI dependency)
│   │   ├── src/
│   │   │   ├── http/              # HTTP client wrapper around undici
│   │   │   ├── collections/       # Collection CRUD, file I/O
│   │   │   ├── environments/      # Variable resolution engine
│   │   │   ├── scripts/           # Script sandbox (vm2/isolated-vm)
│   │   │   ├── runner/            # Collection runner engine
│   │   │   ├── auth/              # Auth strategy implementations
│   │   │   ├── mock/              # Mock server engine
│   │   │   └── types/             # Shared TypeScript interfaces
│   │   └── package.json
│   ├── cli/                       # CLI runner (like Newman)
│   │   ├── src/
│   │   │   ├── commands/          # run, export, import commands
│   │   │   └── reporters/         # CLI, JSON, HTML reporters
│   │   └── package.json
│   └── desktop/                   # Electron + React app
│       ├── src/
│       │   ├── main/              # Electron main process
│       │   ├── preload/           # Secure IPC bridge
│       │   └── renderer/          # React UI
│       │       ├── components/
│       │       ├── hooks/
│       │       ├── stores/        # Zustand stores
│       │       └── pages/
│       └── package.json
├── package.json                   # Monorepo root (npm workspaces or turborepo)
└── tsconfig.base.json
```

### Key TypeScript Interfaces

```typescript
interface NexusRequest {
  id: string;
  name: string;
  method: HttpMethod;
  url: string;                     // Supports {{variable}} interpolation
  headers: KeyValuePair[];
  params: KeyValuePair[];
  body: RequestBody;
  auth: AuthConfig;
  preRequestScript?: string;
  postResponseScript?: string;
  settings: RequestSettings;
}

interface NexusCollection {
  id: string;
  name: string;
  description?: string;
  variables: Variable[];
  auth?: AuthConfig;
  preRequestScript?: string;
  postResponseScript?: string;
  items: (NexusRequest | NexusFolder)[];
}

interface NexusEnvironment {
  id: string;
  name: string;
  variables: Variable[];
}

interface Variable {
  key: string;
  value: string;
  type: 'string' | 'secret';
  enabled: boolean;
  description?: string;
}

type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS';

interface KeyValuePair {
  key: string;
  value: string;
  enabled: boolean;
  description?: string;
}
```

---

## 2. Request Builder & Sender

**What it is:** The core UI and engine for composing HTTP requests — method, URL, headers, query params, body — and sending them to a server.

**Why it matters:** This is the primary daily-driver feature. It must handle every HTTP nuance: multipart forms, binary uploads, URL encoding, redirects, timeouts, and SSL configuration.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Method selector | Dropdown for GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS |
| URL bar | Text input with variable interpolation (`{{base_url}}/users`) |
| Query params editor | Key-value table, auto-syncs with URL query string |
| Headers editor | Key-value table with autocomplete for common headers |
| Body editors | `none`, `form-data`, `x-www-form-urlencoded`, `raw` (JSON/XML/Text/HTML), `binary`, `GraphQL` |
| File uploads | Support `multipart/form-data` with file picker |
| Request settings | Follow redirects, timeout, SSL verification toggle, max redirect count |
| Send & cancel | Send button triggers request; cancel aborts in-flight request |
| Response timing | Show DNS, TCP, TLS, TTFB, download, total time |

### Implementation Plan

**HTTP Engine (`packages/core/src/http/`):**

```typescript
// request-sender.ts
import { request, Dispatcher } from 'undici';

interface SendOptions {
  method: HttpMethod;
  url: string;
  headers: Record<string, string>;
  body?: string | Buffer | FormData;
  timeout?: number;
  followRedirects?: boolean;
  maxRedirects?: number;
  rejectUnauthorized?: boolean;    // SSL verification
  signal?: AbortSignal;
}

interface NexusResponse {
  status: number;
  statusText: string;
  headers: Record<string, string | string[]>;
  body: Buffer;
  bodyText: string;
  bodyJson?: unknown;
  timing: ResponseTiming;
  size: number;
  cookies: Cookie[];
}

interface ResponseTiming {
  dns: number;
  tcp: number;
  tls: number;
  ttfb: number;
  download: number;
  total: number;
}

async function sendRequest(opts: SendOptions): Promise<NexusResponse> {
  // 1. Resolve variables in URL, headers, body via VariableResolver
  // 2. Build undici request options with dispatcher for timing hooks
  // 3. Create AbortController for cancellation
  // 4. Use undici.request() with timing interceptor
  // 5. Collect response buffer, parse headers, compute timing
  // 6. Return NexusResponse
}
```

**Variable Interpolation (`packages/core/src/environments/`):**

```typescript
// variable-resolver.ts
function resolveVariables(
  template: string,
  scopes: VariableScope[]     // [local, environment, collection, global] — first match wins
): string {
  return template.replace(/\{\{(.+?)\}\}/g, (_, key) => {
    for (const scope of scopes) {
      const v = scope.get(key.trim());
      if (v !== undefined) return v;
    }
    return `{{${key}}}`;       // Leave unresolved as-is
  });
}
```

**UI Component (`packages/desktop/src/renderer/`):**

- `<RequestBuilder>` — Orchestrator component
  - `<MethodSelector>` — Dropdown, color-coded per method
  - `<UrlBar>` — Input with variable highlight (use CodeMirror with custom tokenizer)
  - `<ParamsEditor>` — Table component with add/delete/toggle rows
  - `<HeadersEditor>` — Same table component with header name autocomplete
  - `<BodyEditor>` — Tabbed: none | form-data | x-www-form-urlencoded | raw | binary
    - Raw mode: CodeMirror with JSON/XML/HTML/Text language modes
    - Form-data: table with file upload support via `<input type="file">`
  - `<SendButton>` — Triggers IPC call to main process, shows spinner, supports cancel

---

## 3. Collections & Folders

**What it is:** Organize requests into named collections with nested folders. Collections are stored as files on disk, making them naturally Git-friendly.

**Why it matters:** Without organization, a developer with hundreds of API endpoints has chaos. Collections group related requests and enable batch operations (run all, export, share).

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Create/rename/delete collections | CRUD operations on collection directories |
| Nested folders | Arbitrary depth folder nesting within a collection |
| Drag & drop reorder | Reorder requests and folders via drag-and-drop |
| Collection-level auth | Inherit auth config from collection to all requests |
| Collection-level scripts | Pre-request and post-response scripts that run for every request |
| Collection variables | Variables scoped to a single collection |
| Duplicate request/folder | Deep-clone a request or folder |
| Search within collection | Full-text search across request names, URLs, bodies |

### Storage Format

```json
// .nexus/collections/my-api/collection.json
{
  "id": "col_a1b2c3",
  "name": "My API",
  "description": "Backend API for the main product",
  "version": "1.0.0",
  "variables": [
    { "key": "base_url", "value": "https://api.example.com", "enabled": true }
  ],
  "auth": { "type": "bearer", "token": "{{auth_token}}" },
  "preRequestScript": "// runs before every request in this collection",
  "postResponseScript": "// runs after every request in this collection",
  "itemOrder": ["folder_users", "req_healthcheck"]
}

// .nexus/collections/my-api/requests/get-users.json
{
  "id": "req_d4e5f6",
  "name": "Get Users",
  "method": "GET",
  "url": "{{base_url}}/users",
  "headers": [
    { "key": "Accept", "value": "application/json", "enabled": true }
  ],
  "params": [],
  "body": { "mode": "none" },
  "auth": { "type": "inherit" },
  "preRequestScript": "",
  "postResponseScript": ""
}
```

### Implementation Plan

```typescript
// packages/core/src/collections/collection-manager.ts

class CollectionManager {
  constructor(private basePath: string) {}

  async listCollections(): Promise<CollectionMeta[]> {
    // Scan basePath for directories containing collection.json
  }

  async loadCollection(id: string): Promise<NexusCollection> {
    // Read collection.json + all request/folder JSON files
    // Assemble into in-memory tree structure
  }

  async saveRequest(collectionId: string, request: NexusRequest): Promise<void> {
    // Write request JSON to disk
    // Update collection.json itemOrder if new
  }

  async createFolder(collectionId: string, folder: NexusFolder): Promise<void> {
    // Create folder directory and metadata JSON
  }

  async moveItem(collectionId: string, itemId: string, targetFolderId: string | null, index: number): Promise<void> {
    // Move file on disk, update itemOrder arrays
  }

  async deleteItem(collectionId: string, itemId: string): Promise<void> {
    // Remove file/directory from disk, update parent itemOrder
  }

  watch(collectionId: string, callback: (event: FileChangeEvent) => void): void {
    // Use chokidar to watch for external file changes (e.g., git pull)
  }
}
```

**UI:** Sidebar tree view using a virtualized list (`react-window`) for performance with large collections. Support keyboard navigation, right-click context menus, and drag-and-drop via `@dnd-kit/core`.

---

## 4. Environment & Variable System

**What it is:** A layered variable system that lets you define key-value pairs at different scopes (global, collection, environment, local) and reference them as `{{variable_name}}` anywhere in requests.

**Why it matters:** This is what makes a single collection work across dev, staging, and production. Instead of hard-coding `localhost:3000`, you use `{{base_url}}` and switch environments.

### Variable Resolution Order (highest to lowest priority)

1. **Local/temporary** — Set during a script run, discarded after
2. **Environment** — Active environment file (e.g., `dev.json`)
3. **Collection** — Defined in `collection.json`
4. **Global** — Defined in `globals.json`

### Dynamic Variables (Built-in)

| Variable | Description | Example |
|---|---|---|
| `{{$timestamp}}` | Current Unix timestamp | `1713100800` |
| `{{$isoTimestamp}}` | ISO 8601 timestamp | `2026-04-14T12:00:00.000Z` |
| `{{$randomUUID}}` | UUID v4 | `a1b2c3d4-...` |
| `{{$randomInt}}` | Random integer 0-1000 | `742` |
| `{{$randomEmail}}` | Faker email | `alice@example.com` |
| `{{$randomFullName}}` | Faker full name | `Jane Smith` |
| `{{$randomCity}}` | Faker city | `San Francisco` |

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Environment CRUD | Create, edit, delete, duplicate environments |
| Quick switch | Dropdown in the toolbar to switch active environment |
| Secret variables | Mask values in the UI, never export them |
| Variable autocomplete | `{{` trigger in URL/header/body shows variable picker |
| Bulk edit | Text mode for pasting many variables at once |
| Variable quick-look | Hover over `{{var}}` to see resolved value |

### Implementation Plan

```typescript
// packages/core/src/environments/variable-store.ts

interface VariableScope {
  get(key: string): string | undefined;
  set(key: string, value: string): void;
  unset(key: string): void;
  toObject(): Record<string, string>;
}

class VariableStore {
  private scopes: Map<ScopeLevel, VariableScope> = new Map();

  constructor(
    private globalVars: Variable[],
    private collectionVars: Variable[],
    private environmentVars: Variable[],
  ) {
    // Initialize scopes from loaded files
  }

  resolve(template: string): string {
    return template.replace(/\{\{(.+?)\}\}/g, (match, key) => {
      const trimmed = key.trim();

      // Handle dynamic variables first
      if (trimmed.startsWith('$')) {
        return this.resolveDynamic(trimmed);
      }

      // Walk scopes: local → environment → collection → global
      for (const level of [ScopeLevel.Local, ScopeLevel.Environment, ScopeLevel.Collection, ScopeLevel.Global]) {
        const scope = this.scopes.get(level);
        const val = scope?.get(trimmed);
        if (val !== undefined) return val;
      }

      return match; // Leave unresolved
    });
  }

  private resolveDynamic(key: string): string {
    switch (key) {
      case '$timestamp': return String(Math.floor(Date.now() / 1000));
      case '$isoTimestamp': return new Date().toISOString();
      case '$randomUUID': return crypto.randomUUID();
      case '$randomInt': return String(Math.floor(Math.random() * 1001));
      case '$randomEmail': return faker.internet.email();
      // ... more dynamic variables using @faker-js/faker
      default: return `{{${key}}}`;
    }
  }
}
```

**Storage:** Each environment is a standalone JSON file in `.nexus/environments/`. Secret variables have `"type": "secret"` and their values are stored in a separate `.nexus/environments/.secrets` file that can be `.gitignore`'d.

---

## 5. Authentication

**What it is:** Built-in auth helpers that automatically generate the correct headers, tokens, or signatures for common auth schemes.

**Why it matters:** Manually constructing OAuth2 tokens or AWS Signature V4 headers is tedious and error-prone. Auth helpers automate this and keep credentials organized.

### Supported Auth Types

| Auth Type | Description |
|---|---|
| No Auth | No authentication |
| API Key | Send key as header or query param |
| Bearer Token | `Authorization: Bearer <token>` |
| Basic Auth | `Authorization: Basic base64(user:pass)` |
| OAuth 2.0 | Full flow: Authorization Code, Client Credentials, Password, Implicit, PKCE |
| Digest Auth | Challenge-response authentication |
| AWS Signature v4 | Sign requests for AWS APIs |
| Hawk | MAC-based auth |
| Custom | User-defined via pre-request scripts |

### Auth Inheritance

Requests inherit auth from their parent folder, which inherits from the collection. Each level can override or set to "No Auth" or "Inherit".

### Implementation Plan

```typescript
// packages/core/src/auth/auth-handler.ts

interface AuthConfig {
  type: AuthType;
  [key: string]: unknown;
}

interface AuthResult {
  headers?: Record<string, string>;
  params?: Record<string, string>;
}

abstract class AuthHandler {
  abstract apply(config: AuthConfig, request: PreparedRequest): Promise<AuthResult>;
}

class BearerTokenAuth extends AuthHandler {
  async apply(config: AuthConfig): Promise<AuthResult> {
    return { headers: { 'Authorization': `Bearer ${config.token}` } };
  }
}

class BasicAuth extends AuthHandler {
  async apply(config: AuthConfig): Promise<AuthResult> {
    const encoded = Buffer.from(`${config.username}:${config.password}`).toString('base64');
    return { headers: { 'Authorization': `Basic ${encoded}` } };
  }
}

class OAuth2Auth extends AuthHandler {
  async apply(config: OAuth2Config, request: PreparedRequest): Promise<AuthResult> {
    // 1. Check if we have a valid (non-expired) token in token store
    // 2. If expired and refresh_token exists, refresh automatically
    // 3. If no token, initiate the appropriate grant flow:
    //    - Authorization Code: Open browser window, start local callback server,
    //      exchange code for token
    //    - Client Credentials: POST to token endpoint
    //    - Password: POST with username/password
    //    - PKCE: Generate code_verifier/challenge, open browser, exchange
    // 4. Store token with expiry for reuse
    // 5. Return Authorization header
  }
}

// OAuth2 callback server for Authorization Code / PKCE flows
class OAuth2CallbackServer {
  private server: http.Server;

  async start(port: number, redirectUri: string): Promise<string> {
    // Start local HTTP server to receive the OAuth2 callback
    // Return the authorization code from the callback URL
  }
}

// Token storage (persisted to disk, encrypted at rest)
class TokenStore {
  private tokens: Map<string, StoredToken> = new Map();
  private filePath: string;

  async getToken(key: string): Promise<StoredToken | null> { /* ... */ }
  async saveToken(key: string, token: StoredToken): Promise<void> { /* ... */ }
  isExpired(token: StoredToken): boolean {
    return Date.now() >= token.expiresAt - 30_000; // 30s buffer
  }
}

class DigestAuth extends AuthHandler {
  async apply(config: AuthConfig, request: PreparedRequest): Promise<AuthResult> {
    // 1. Send initial request without auth
    // 2. Parse WWW-Authenticate header (realm, nonce, qop, algorithm)
    // 3. Compute digest hash (MD5 or SHA-256)
    // 4. Return Authorization header with digest response
  }
}

class AwsSigV4Auth extends AuthHandler {
  async apply(config: AuthConfig, request: PreparedRequest): Promise<AuthResult> {
    // 1. Create canonical request string
    // 2. Create string to sign with date, region, service
    // 3. Calculate signing key from secret + date + region + service
    // 4. Calculate signature using HMAC-SHA256
    // 5. Return Authorization header with AWS4-HMAC-SHA256 credential/signature
  }
}

// Auth handler registry
const authHandlers: Record<AuthType, AuthHandler> = {
  'none': new NoAuth(),
  'api-key': new ApiKeyAuth(),
  'bearer': new BearerTokenAuth(),
  'basic': new BasicAuth(),
  'oauth2': new OAuth2Auth(),
  'digest': new DigestAuth(),
  'aws-sig-v4': new AwsSigV4Auth(),
  'hawk': new HawkAuth(),
};
```

**UI:** Auth tab in the request builder with a type dropdown. Each type shows relevant fields (e.g., OAuth2 shows Grant Type, Auth URL, Token URL, Client ID, Client Secret, Scope). A "Get New Access Token" button for OAuth2 opens the browser flow.

---

## 6. Pre-Request & Post-Response Scripts

**What it is:** JavaScript code that runs before a request is sent (pre-request) and after a response is received (post-response). Scripts can modify variables, set headers dynamically, log data, and chain request data.

**Why it matters:** Scripts are what make Postman a programmable API tool rather than a simple HTTP sender. They enable dynamic data, auth token refresh, data chaining between requests, and custom logic.

### Script Execution Order (within a collection run)

```
Collection Pre-Request Script
  └─ Folder Pre-Request Script
       └─ Request Pre-Request Script
            └─ *** REQUEST IS SENT ***
       └─ Request Post-Response Script
  └─ Folder Post-Response Script
└─ Collection Post-Response Script
```

### Sandbox API (the `nx` object — equivalent to Postman's `pm`)

```typescript
// Available inside scripts as `nx`

nx.environment.get(key)              // Read environment variable
nx.environment.set(key, value)       // Set environment variable
nx.environment.unset(key)            // Remove environment variable

nx.collectionVariables.get(key)
nx.collectionVariables.set(key, value)

nx.globals.get(key)
nx.globals.set(key, value)

nx.variables.get(key)                // Resolve from all scopes (local → env → collection → global)

nx.request.url                       // Current request URL (mutable in pre-request)
nx.request.headers                   // Headers object (mutable in pre-request)
nx.request.body                      // Body (mutable in pre-request)
nx.request.method                    // HTTP method

nx.response.code                     // HTTP status code (post-response only)
nx.response.status                   // Status text
nx.response.headers                  // Response headers
nx.response.json()                   // Parse response body as JSON
nx.response.text()                   // Response body as text
nx.response.responseTime             // Total response time in ms

nx.execution.setNextRequest(name)    // Control collection runner flow
nx.execution.skipRequest()           // Skip current request in runner

nx.sendRequest(requestOptions)       // Send an arbitrary HTTP request from within a script

console.log(...)                     // Logs appear in the Console panel
```

### Implementation Plan

```typescript
// packages/core/src/scripts/sandbox.ts
import ivm from 'isolated-vm';

interface SandboxContext {
  environment: VariableScope;
  collectionVariables: VariableScope;
  globals: VariableScope;
  request: MutableRequest;       // pre-request can modify this
  response?: ResponseData;       // only available in post-response
  execution: ExecutionControl;
}

class ScriptSandbox {
  private isolate: ivm.Isolate;

  constructor(memoryLimitMb: number = 128) {
    this.isolate = new ivm.Isolate({ memoryLimit: memoryLimitMb });
  }

  async execute(script: string, context: SandboxContext): Promise<ScriptResult> {
    const ctx = await this.isolate.createContext();

    // Inject the `nx` object as a frozen global with callbacks to host
    // Each method (nx.environment.get, etc.) is a Reference to a host function
    await this.injectNxObject(ctx, context);

    // Inject console.log that captures to a log buffer
    const logs: LogEntry[] = [];
    await this.injectConsole(ctx, logs);

    // Compile and run the script with a timeout
    const compiledScript = await this.isolate.compileScript(script);
    await compiledScript.run(ctx, { timeout: 30_000 });

    return {
      logs,
      variableChanges: context.getChanges(),
      nextRequest: context.execution.getNextRequest(),
      skipped: context.execution.wasSkipped(),
    };
  }

  private async injectNxObject(ctx: ivm.Context, context: SandboxContext): Promise<void> {
    // Create a JavaScript object inside the isolate that proxies calls
    // back to the host via ivm.Reference callbacks.
    // This ensures scripts can't access the filesystem, network,
    // or any Node.js APIs beyond what we explicitly expose.
  }

  dispose(): void {
    this.isolate.dispose();
  }
}
```

**Script Editor UI:** CodeMirror editor with JavaScript syntax highlighting, autocomplete for the `nx.*` API, and a snippets sidebar (common operations like "Set environment variable", "Parse JSON body", "Assert status code").

---

## 7. Test Assertions

**What it is:** A testing framework embedded in post-response scripts that lets you write assertions about response data — status codes, headers, body content, schema validation, and response time.

**Why it matters:** Automated assertions turn manual spot-checking into repeatable, CI-ready tests. This is what makes the Collection Runner and CLI runner useful.

### Assertion API

```typescript
// Available in post-response scripts

nx.test("Status code is 200", () => {
  nx.expect(nx.response.code).to.equal(200);
});

nx.test("Response has user data", () => {
  const body = nx.response.json();
  nx.expect(body).to.have.property('id');
  nx.expect(body.email).to.be.a('string');
  nx.expect(body.roles).to.include('admin');
});

nx.test("Response time is under 500ms", () => {
  nx.expect(nx.response.responseTime).to.be.below(500);
});

nx.test("Content-Type is JSON", () => {
  nx.expect(nx.response.headers['content-type']).to.include('application/json');
});

nx.test("Response matches schema", () => {
  const schema = {
    type: 'object',
    required: ['id', 'email', 'name'],
    properties: {
      id: { type: 'string' },
      email: { type: 'string', format: 'email' },
      name: { type: 'string' },
    }
  };
  nx.expect(nx.response.json()).to.matchSchema(schema);
});
```

### Implementation Plan

```typescript
// packages/core/src/scripts/assertions.ts
import Ajv from 'ajv';
import addFormats from 'ajv-formats';

interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
  duration: number;
}

class AssertionEngine {
  private results: TestResult[] = [];
  private ajv: Ajv;

  constructor() {
    this.ajv = new Ajv({ allErrors: true });
    addFormats(this.ajv);
  }

  test(name: string, fn: () => void): void {
    const start = performance.now();
    try {
      fn();
      this.results.push({ name, passed: true, duration: performance.now() - start });
    } catch (err) {
      this.results.push({
        name,
        passed: false,
        error: err instanceof Error ? err.message : String(err),
        duration: performance.now() - start,
      });
    }
  }

  expect(value: unknown): ChaiLikeExpect {
    // Return a chainable assertion object supporting:
    // .to.equal(), .to.be.a(), .to.have.property(),
    // .to.include(), .to.be.above(), .to.be.below(),
    // .to.matchSchema(), .to.have.length(), .to.be.empty,
    // .to.match() (regex), .to.be.oneOf()
    return new Expectation(value, this.ajv);
  }

  getResults(): TestResult[] {
    return [...this.results];
  }
}
```

**UI:** A "Test Results" tab in the response panel showing pass/fail with green/red indicators, error messages for failures, and overall summary (X passed, Y failed, Z total).

---

## 8. Collection Runner

**What it is:** Execute all (or selected) requests in a collection sequentially, with configurable iterations, delays, data files (CSV/JSON), and workflow control.

**Why it matters:** The runner transforms a collection from documentation into an executable test suite. Data-driven testing with CSV/JSON files enables testing many scenarios without duplicating requests.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Select requests | Choose which requests to run and in what order |
| Iterations | Run the collection N times |
| Delay | Configurable delay (ms) between each request |
| Data file | CSV or JSON file providing variable values per iteration |
| Environment selection | Pick which environment to use for the run |
| Persist variables | Optionally keep variable changes after the run completes |
| Stop on error | Halt the run if a request fails or a test assertion fails |
| Workflow control | `nx.execution.setNextRequest()` to branch/loop |
| Real-time results | Stream pass/fail results as the run progresses |

### Implementation Plan

```typescript
// packages/core/src/runner/collection-runner.ts

interface RunConfig {
  collectionId: string;
  environmentId?: string;
  requestIds?: string[];          // Subset of requests; null = all
  iterations: number;
  delayMs: number;
  dataFile?: DataFile;            // CSV or JSON
  stopOnError: boolean;
  persistVariables: boolean;
}

interface DataFile {
  type: 'csv' | 'json';
  path: string;
  rows: Record<string, string>[]; // Parsed data rows
}

interface RunEvent {
  type: 'request-start' | 'request-complete' | 'test-result' | 'iteration-start'
       | 'iteration-complete' | 'run-complete' | 'error' | 'console-log';
  data: unknown;
}

class CollectionRunner extends EventEmitter<RunEvent> {
  private sandbox: ScriptSandbox;
  private httpSender: RequestSender;
  private variableStore: VariableStore;
  private abortController: AbortController;

  async run(config: RunConfig): Promise<RunSummary> {
    const collection = await this.loadCollection(config.collectionId);
    const dataRows = config.dataFile ? await this.parseDataFile(config.dataFile) : [{}];

    for (let iteration = 0; iteration < config.iterations; iteration++) {
      this.emit({ type: 'iteration-start', data: { iteration } });

      // Merge data file row into local variables
      const rowData = dataRows[iteration % dataRows.length];
      this.variableStore.setLocalScope(rowData);

      const requestQueue = this.buildRequestQueue(collection, config.requestIds);

      let currentIndex = 0;
      while (currentIndex < requestQueue.length) {
        const req = requestQueue[currentIndex];
        this.emit({ type: 'request-start', data: { request: req } });

        // Execute script hierarchy: collection → folder → request pre-request
        await this.runScriptChain('pre-request', collection, req);

        if (this.executionControl.wasSkipped()) {
          currentIndex++;
          continue;
        }

        // Send the request
        const response = await this.httpSender.send(req, this.abortController.signal);

        // Execute script hierarchy: request → folder → collection post-response
        await this.runScriptChain('post-response', collection, req, response);

        this.emit({ type: 'request-complete', data: { request: req, response, tests: this.getTestResults() } });

        // Handle setNextRequest
        const nextRequest = this.executionControl.getNextRequest();
        if (nextRequest !== undefined) {
          currentIndex = nextRequest === null
            ? requestQueue.length                    // null = stop the run
            : requestQueue.findIndex(r => r.name === nextRequest);
        } else {
          currentIndex++;
        }

        if (config.delayMs > 0) await this.delay(config.delayMs);
        if (config.stopOnError && this.hasFailures()) break;
      }
    }

    return this.buildSummary();
  }

  cancel(): void {
    this.abortController.abort();
  }
}
```

**Data File Parsing:**

```typescript
// CSV parsing with csv-parse
import { parse } from 'csv-parse/sync';

function parseCsv(content: string): Record<string, string>[] {
  return parse(content, { columns: true, skip_empty_lines: true });
}

// JSON data file: expects an array of objects
function parseJsonData(content: string): Record<string, string>[] {
  return JSON.parse(content);
}
```

**UI:** Runner panel with drag-to-reorder request list, iteration/delay inputs, data file upload, a "Run" button, and a real-time results view showing request name, status, response time, and test pass/fail counts per request.

---

## 9. GraphQL Client

**What it is:** A dedicated interface for composing and sending GraphQL queries, mutations, and subscriptions, with schema introspection, an interactive docs explorer, and variable support.

**Why it matters:** GraphQL has different ergonomics from REST — you need a query editor, variable pane, and schema awareness. A generic HTTP sender misses the schema-driven experience.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Query editor | CodeMirror with GraphQL syntax, auto-indent, error highlighting |
| Schema introspection | Fetch schema via introspection query, cache it |
| Docs explorer | Interactive sidebar to browse types, fields, arguments |
| Variables editor | JSON editor for GraphQL variables |
| Auto-complete | Field names, arguments, types from introspected schema |
| Subscriptions | WebSocket-based subscriptions (graphql-ws protocol) |
| Multi-operation | Select which operation to execute in multi-operation documents |
| Headers/Auth | Same auth and header support as REST requests |

### Implementation Plan

```typescript
// packages/core/src/graphql/graphql-client.ts

interface GraphQLRequest {
  url: string;
  query: string;
  variables?: Record<string, unknown>;
  operationName?: string;
  headers: Record<string, string>;
}

class GraphQLClient {
  async send(req: GraphQLRequest): Promise<NexusResponse> {
    // GraphQL over HTTP: POST with JSON body { query, variables, operationName }
    return sendRequest({
      method: 'POST',
      url: req.url,
      headers: { ...req.headers, 'Content-Type': 'application/json' },
      body: JSON.stringify({
        query: req.query,
        variables: req.variables,
        operationName: req.operationName,
      }),
    });
  }

  async introspect(url: string, headers: Record<string, string>): Promise<GraphQLSchema> {
    // Send the standard introspection query
    // Parse the response into a GraphQLSchema using graphql-js
    const introspectionQuery = getIntrospectionQuery();
    const response = await this.send({ url, query: introspectionQuery, headers });
    return buildClientSchema(response.bodyJson.data);
  }

  subscribe(
    url: string,
    query: string,
    variables: Record<string, unknown>,
    headers: Record<string, string>,
    onMessage: (data: unknown) => void,
  ): Subscription {
    // Use graphql-ws library for WebSocket subscriptions
    // Return a Subscription handle with .unsubscribe()
  }
}
```

**Dependencies:**
- `graphql` — Schema parsing, introspection utilities
- `graphql-ws` — WebSocket subscription protocol
- `codemirror-graphql` or `@graphiql/react` — Editor with GraphQL language support

**UI:** Dedicated GraphQL request tab with three-pane layout: query editor (left), variables editor (bottom-left), response (right). Docs explorer as a collapsible sidebar. Schema status indicator (fetched/stale/error).

---

## 10. WebSocket Client

**What it is:** A persistent, bidirectional connection interface for testing WebSocket endpoints. Compose messages, send them, and view the message stream in real time.

**Why it matters:** WebSocket APIs are fundamentally different from REST — they're persistent and bidirectional. You need a dedicated UI for managing the connection lifecycle and message stream.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Connect/disconnect | Open and close WebSocket connections |
| URL with protocols | Specify `ws://` or `wss://` URL and subprotocols |
| Custom headers | Send headers during the WebSocket handshake |
| Message composer | Text/JSON editor for outgoing messages |
| Message history | Scrollable list of sent and received messages with timestamps |
| Binary support | View binary frames as hex dump |
| Auto-reconnect | Optional automatic reconnection on disconnect |
| Connection status | Visual indicator: connecting, open, closing, closed |
| Saved connections | Save WebSocket connections to collections |

### Implementation Plan

```typescript
// packages/core/src/websocket/websocket-client.ts
import WebSocket from 'ws';

interface WsConnection {
  id: string;
  url: string;
  protocols?: string[];
  headers: Record<string, string>;
  status: 'connecting' | 'open' | 'closing' | 'closed';
}

interface WsMessage {
  id: string;
  direction: 'sent' | 'received';
  timestamp: number;
  type: 'text' | 'binary';
  data: string | Buffer;
  size: number;
}

class WebSocketClient extends EventEmitter {
  private socket: WebSocket | null = null;
  private messages: WsMessage[] = [];

  connect(url: string, opts: { headers?: Record<string, string>; protocols?: string[] }): void {
    this.socket = new WebSocket(url, opts.protocols, {
      headers: opts.headers,
      rejectUnauthorized: true,    // Configurable
    });

    this.socket.on('open', () => this.emit('status', 'open'));
    this.socket.on('message', (data, isBinary) => {
      const msg: WsMessage = {
        id: crypto.randomUUID(),
        direction: 'received',
        timestamp: Date.now(),
        type: isBinary ? 'binary' : 'text',
        data: isBinary ? data : data.toString(),
        size: Buffer.byteLength(data),
      };
      this.messages.push(msg);
      this.emit('message', msg);
    });
    this.socket.on('close', (code, reason) => this.emit('close', { code, reason: reason.toString() }));
    this.socket.on('error', (err) => this.emit('error', err));
  }

  send(data: string | Buffer): WsMessage {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected');
    }
    this.socket.send(data);
    const msg: WsMessage = {
      id: crypto.randomUUID(),
      direction: 'sent',
      timestamp: Date.now(),
      type: typeof data === 'string' ? 'text' : 'binary',
      data,
      size: Buffer.byteLength(data),
    };
    this.messages.push(msg);
    return msg;
  }

  disconnect(code?: number, reason?: string): void {
    this.socket?.close(code ?? 1000, reason);
  }
}
```

**Dependency:** `ws` — Robust WebSocket library for Node.js

**UI:** Connection bar at top (URL + Connect/Disconnect button), message composer below, and a message stream panel showing interleaved sent/received messages with color coding (blue = sent, green = received). Each message expandable to show full payload.

---

## 11. Server-Sent Events (SSE) Client

**What it is:** A client for Server-Sent Events endpoints that displays the event stream in real time with event name, data, and ID.

**Why it matters:** SSE is increasingly used for real-time features (AI streaming responses, live feeds). Testing these requires a persistent connection viewer, not a standard request/response cycle.

### Implementation Plan

```typescript
// packages/core/src/sse/sse-client.ts

interface SseEvent {
  id?: string;
  event: string;
  data: string;
  timestamp: number;
  retry?: number;
}

class SseClient extends EventEmitter {
  private abortController: AbortController | null = null;

  async connect(url: string, headers: Record<string, string>): Promise<void> {
    this.abortController = new AbortController();

    const response = await fetch(url, {
      headers: { ...headers, 'Accept': 'text/event-stream' },
      signal: this.abortController.signal,
    });

    if (!response.body) throw new Error('No response body');

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });

      // Parse SSE protocol: split on double newlines
      const events = buffer.split('\n\n');
      buffer = events.pop() ?? '';   // Keep incomplete event in buffer

      for (const raw of events) {
        const event = this.parseEvent(raw);
        if (event) this.emit('event', event);
      }
    }

    this.emit('close');
  }

  private parseEvent(raw: string): SseEvent | null {
    const lines = raw.split('\n');
    const event: Partial<SseEvent> = { timestamp: Date.now(), event: 'message' };

    for (const line of lines) {
      if (line.startsWith('data: ')) event.data = (event.data ?? '') + line.slice(6);
      else if (line.startsWith('event: ')) event.event = line.slice(7);
      else if (line.startsWith('id: ')) event.id = line.slice(4);
      else if (line.startsWith('retry: ')) event.retry = parseInt(line.slice(7), 10);
    }

    return event.data !== undefined ? (event as SseEvent) : null;
  }

  disconnect(): void {
    this.abortController?.abort();
  }
}
```

**UI:** Similar to WebSocket — connection bar at top, event stream below. Each event card shows event type, ID, data (formatted as JSON if parseable), and timestamp. Filter by event type.

---

## 12. gRPC Client

**What it is:** A client for testing gRPC services — load `.proto` files or use server reflection, compose request messages, and send unary, server-streaming, client-streaming, and bidirectional-streaming RPCs.

**Why it matters:** gRPC is the standard for high-performance microservice communication. Without a GUI tool, testing gRPC requires CLI tools like `grpcurl` which lack the ergonomics of a visual client.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Proto file loading | Import `.proto` files from disk |
| Server reflection | Discover services/methods via gRPC reflection |
| Service browser | Tree view of services, methods, and message types |
| Message editor | JSON editor for composing request messages (proto → JSON mapping) |
| Unary RPC | Standard request-response |
| Server streaming | Receive a stream of messages |
| Client streaming | Send a stream of messages |
| Bidirectional streaming | Full-duplex streaming |
| Metadata (headers) | Send custom gRPC metadata |
| Auth | Bearer token, API key, or mTLS for gRPC |
| TLS configuration | Custom CA certs, client certificates |

### Implementation Plan

```typescript
// packages/core/src/grpc/grpc-client.ts
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';

interface GrpcServiceDef {
  name: string;
  methods: GrpcMethodDef[];
}

interface GrpcMethodDef {
  name: string;
  fullPath: string;
  requestType: string;
  responseType: string;
  requestStream: boolean;
  responseStream: boolean;
}

class GrpcClient {
  private packageDef: protoLoader.PackageDefinition | null = null;

  async loadProto(protoPath: string, includeDirs?: string[]): Promise<GrpcServiceDef[]> {
    this.packageDef = await protoLoader.load(protoPath, {
      keepCase: true,
      longs: String,
      enums: String,
      defaults: true,
      oneofs: true,
      includeDirs,
    });
    return this.extractServices();
  }

  async useReflection(address: string, credentials: grpc.ChannelCredentials): Promise<GrpcServiceDef[]> {
    // Use grpc-reflection-js to fetch service definitions from the server
  }

  async unaryCall(
    address: string,
    methodPath: string,
    message: Record<string, unknown>,
    metadata: Record<string, string>,
    credentials: grpc.ChannelCredentials,
  ): Promise<GrpcResponse> {
    // Create client, make unary call, return response
  }

  serverStream(
    address: string,
    methodPath: string,
    message: Record<string, unknown>,
    metadata: Record<string, string>,
    credentials: grpc.ChannelCredentials,
    onMessage: (msg: unknown) => void,
    onEnd: (status: grpc.StatusObject) => void,
  ): { cancel: () => void } {
    // Create streaming call, wire events to callbacks
  }

  // Similar for client streaming and bidirectional streaming
}
```

**Dependencies:**
- `@grpc/grpc-js` — Pure JS gRPC implementation
- `@grpc/proto-loader` — Proto file loading
- `grpc-reflection-js` — Server reflection client

**UI:** Three-pane layout: service browser (left sidebar), request message editor (center, JSON with proto field hints), response viewer (right). Streaming RPCs show a message list similar to WebSocket UI.

---

## 13. Response Viewer

**What it is:** A rich viewer for HTTP responses — formatted body (JSON, XML, HTML, images), headers table, cookies, test results, timing breakdown, and size info.

**Why it matters:** The response viewer is where you spend most of your time understanding API behavior. Good formatting, syntax highlighting, search, and copy capabilities save significant time.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Pretty-printed body | JSON with syntax highlighting and collapsible nodes |
| Raw body | Unformatted text view |
| Preview | HTML rendering, image display |
| Headers table | Response headers with search/filter |
| Cookies tab | Parsed Set-Cookie headers |
| Test results tab | Pass/fail list from assertion scripts |
| Timing waterfall | Visual breakdown: DNS, TCP, TLS, TTFB, Download |
| Size info | Response size (body + headers) |
| Search in response | Ctrl+F search within the response body |
| Copy response | Copy body, headers, or entire response |
| Status badge | Color-coded status (2xx green, 3xx blue, 4xx yellow, 5xx red) |
| Save response as example | Save a response as an example for mock servers |

### Implementation Plan

```typescript
// packages/core/src/response/response-formatter.ts

function detectContentType(headers: Record<string, string>): ResponseFormat {
  const ct = headers['content-type'] ?? '';
  if (ct.includes('json')) return 'json';
  if (ct.includes('xml') || ct.includes('html')) return 'xml';
  if (ct.includes('image/')) return 'image';
  if (ct.includes('pdf')) return 'pdf';
  return 'text';
}

function formatJson(raw: string): { formatted: string; error?: string } {
  try {
    return { formatted: JSON.stringify(JSON.parse(raw), null, 2) };
  } catch (e) {
    return { formatted: raw, error: 'Invalid JSON' };
  }
}

function formatXml(raw: string): string {
  // Use a lightweight XML formatter (e.g., xml-formatter package)
}

// Timing waterfall data structure for the UI
interface TimingWaterfall {
  segments: {
    label: string;       // "DNS", "TCP", "TLS", "TTFB", "Download"
    startMs: number;
    durationMs: number;
    color: string;
  }[];
  totalMs: number;
}
```

**UI Components:**
- `<ResponseBody>` — Tab switching between Pretty (CodeMirror read-only), Raw, Preview
  - JSON: collapsible tree view (`react-json-view` or custom)
  - Images: `<img>` with base64 or blob URL
  - HTML: sandboxed `<iframe>` or `<webview>`
- `<ResponseHeaders>` — Sortable/filterable table
- `<ResponseCookies>` — Parsed cookie details
- `<TestResults>` — Green/red pass/fail cards
- `<TimingWaterfall>` — Horizontal bar chart showing DNS→TCP→TLS→TTFB→Download
- `<StatusBadge>` — `200 OK` in green, `404 Not Found` in yellow, etc.

---

## 14. Cookie Management

**What it is:** A cookie jar that persists across requests, with a UI to view, edit, add, and delete cookies per domain.

**Why it matters:** Many APIs use cookies for session management. Without cookie persistence, you'd have to manually copy `Set-Cookie` values. The cookie jar automates this.

### Implementation Plan

```typescript
// packages/core/src/cookies/cookie-jar.ts
import { CookieJar, Cookie } from 'tough-cookie';

class NexusCookieJar {
  private jar: CookieJar;
  private persistPath: string;

  constructor(persistPath: string) {
    // Load saved cookies from disk
    this.jar = new CookieJar();
    this.persistPath = persistPath;
  }

  async setCookie(rawCookie: string, url: string): Promise<void> {
    await this.jar.setCookie(rawCookie, url);
    await this.persist();
  }

  async getCookies(url: string): Promise<Cookie[]> {
    return this.jar.getCookies(url);
  }

  async getCookieString(url: string): Promise<string> {
    return this.jar.getCookieString(url);
  }

  async getAllCookies(): Promise<Cookie[]> {
    // Return all cookies grouped by domain
    return this.jar.serialize().then(s => s.cookies.map(c => Cookie.fromJSON(c)!));
  }

  async removeCookie(domain: string, path: string, key: string): Promise<void> {
    await this.jar.removeCookie(domain, path, key);
    await this.persist();
  }

  async removeAllCookies(): Promise<void> {
    await this.jar.removeAllCookies();
    await this.persist();
  }

  private async persist(): Promise<void> {
    const serialized = await this.jar.serialize();
    await fs.writeFile(this.persistPath, JSON.stringify(serialized, null, 2));
  }

  // Apply cookies to outgoing request, process Set-Cookie from response
  async applyToRequest(url: string, headers: Record<string, string>): Promise<Record<string, string>> {
    const cookieString = await this.getCookieString(url);
    if (cookieString) {
      headers['Cookie'] = cookieString;
    }
    return headers;
  }

  async processResponse(url: string, responseHeaders: Record<string, string | string[]>): Promise<void> {
    const setCookies = responseHeaders['set-cookie'];
    if (!setCookies) return;

    const cookies = Array.isArray(setCookies) ? setCookies : [setCookies];
    for (const raw of cookies) {
      await this.setCookie(raw, url);
    }
  }
}
```

**Dependency:** `tough-cookie` — RFC-compliant cookie jar implementation

**UI:** Cookie manager modal accessible from the toolbar. Grouped by domain, each cookie shows name, value, path, expiry, and flags (HttpOnly, Secure, SameSite). Add/edit/delete buttons.

---

## 15. Proxy & Traffic Capture

**What it is:** A built-in HTTP/HTTPS proxy that captures API traffic from browsers or mobile devices, letting you inspect requests and save them to a collection.

**Why it matters:** When debugging, you often want to see what requests a browser or mobile app is actually sending. Capturing traffic through a proxy lets you observe and replay real requests.

### Implementation Plan

```typescript
// packages/core/src/proxy/proxy-server.ts
import http from 'http';
import https from 'https';
import { connect } from 'tls';

interface CapturedRequest {
  id: string;
  timestamp: number;
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: Buffer;
  response?: {
    status: number;
    headers: Record<string, string>;
    body?: Buffer;
  };
}

class ProxyServer extends EventEmitter {
  private server: http.Server;
  private captured: CapturedRequest[] = [];
  private filters: ProxyFilter[];

  constructor(private port: number = 5560) {
    super();
  }

  start(): void {
    this.server = http.createServer(this.handleHttp.bind(this));
    this.server.on('connect', this.handleConnect.bind(this));  // HTTPS tunneling
    this.server.listen(this.port);
  }

  private async handleHttp(req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
    // 1. Capture the incoming request (method, url, headers, body)
    // 2. Forward the request to the target server using undici
    // 3. Capture the response
    // 4. Forward the response back to the client
    // 5. Emit 'captured' event with full request/response pair
  }

  private handleConnect(req: http.IncomingMessage, clientSocket: net.Socket, head: Buffer): void {
    // HTTPS CONNECT tunneling:
    // 1. Parse target host:port from req.url
    // 2. If intercepting HTTPS, use a self-signed CA to create dynamic certs
    //    and perform MITM to capture decrypted traffic
    // 3. If passthrough, just pipe the connection
  }

  stop(): void {
    this.server.close();
  }

  setFilters(filters: ProxyFilter[]): void {
    this.filters = filters;
  }

  getCaptured(): CapturedRequest[] {
    return [...this.captured];
  }

  exportToCollection(ids: string[], collectionId: string): Promise<void> {
    // Convert captured requests to NexusRequest format and save
  }
}
```

**UI:** Proxy panel showing capture status, port configuration, domain filters, and a traffic log. Each entry is expandable to show full request/response details. Toolbar actions: Start/Stop, Clear, Save to Collection.

---

## 16. Mock Server

**What it is:** A local HTTP server that returns predefined responses based on URL, method, and optional request matching rules. Useful for frontend development when the backend isn't ready.

**Why it matters:** Mock servers decouple frontend from backend development. Define expected responses from a collection's examples, start a local server, and develop against it.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Create from collection | Generate mock routes from saved request/response examples |
| Custom route handlers | Define response per method + path combination |
| Dynamic variables | Use `{{$randomName}}`, `{{$timestamp}}` in responses |
| Request body matching | Match on request body content for different responses |
| Configurable delay | Simulate latency per route |
| Status code selection | Choose which example response (200, 404, 500) to return |
| CORS headers | Auto-add CORS headers for browser compatibility |

### Implementation Plan

```typescript
// packages/core/src/mock/mock-server.ts
import http from 'http';
import { pathToRegexp, match } from 'path-to-regexp';

interface MockRoute {
  method: string;
  path: string;                    // Express-style with params: /users/:id
  response: MockResponse;
  matchBody?: Record<string, unknown>;  // Optional body matching
  delay?: number;
}

interface MockResponse {
  status: number;
  headers: Record<string, string>;
  body: string;                    // Supports {{$dynamic}} variables
}

class MockServer {
  private server: http.Server | null = null;
  private routes: MockRoute[] = [];
  private variableStore: VariableStore;

  constructor(private port: number = 3100) {}

  addRoute(route: MockRoute): void {
    this.routes.push(route);
  }

  loadFromCollection(collection: NexusCollection): void {
    // Walk the collection tree, find requests with saved example responses
    // Create a MockRoute for each request/example pair
    for (const item of this.walkCollection(collection)) {
      if (item.type === 'request' && item.examples?.length) {
        for (const example of item.examples) {
          this.addRoute({
            method: item.method,
            path: this.extractPath(item.url),
            response: {
              status: example.status,
              headers: example.headers,
              body: example.body,
            },
          });
        }
      }
    }
  }

  start(): void {
    this.server = http.createServer(async (req, res) => {
      const route = this.matchRoute(req);

      if (!route) {
        res.writeHead(404, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'No matching mock found' }));
        return;
      }

      if (route.delay) await this.sleep(route.delay);

      // Resolve dynamic variables in response body
      const body = this.variableStore.resolve(route.response.body);

      // Add CORS headers
      const headers = {
        ...route.response.headers,
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': '*',
        'Access-Control-Allow-Headers': '*',
      };

      res.writeHead(route.response.status, headers);
      res.end(body);
    });

    this.server.listen(this.port);
  }

  private matchRoute(req: http.IncomingMessage): MockRoute | null {
    const url = new URL(req.url!, `http://localhost:${this.port}`);

    for (const route of this.routes) {
      if (route.method.toUpperCase() !== req.method?.toUpperCase()) continue;

      const matcher = match(route.path, { decode: decodeURIComponent });
      if (matcher(url.pathname)) return route;
    }
    return null;
  }

  stop(): void {
    this.server?.close();
  }
}
```

**UI:** Mock server management panel: list of active mocks, port configuration, route table with method, path, status code, and delay. "Create from Collection" button. Log of matched/unmatched incoming requests.

---

## 17. Code Snippet Generation

**What it is:** Convert any request into a ready-to-use code snippet in various languages and HTTP libraries (cURL, Python requests, JavaScript fetch, Go, Rust, etc.).

**Why it matters:** After testing an API call in Nexus, you want to use it in your actual code. Code generation saves the manual translation step and reduces errors.

### Supported Languages

| Language | Libraries |
|---|---|
| cURL | cURL |
| JavaScript | fetch, Axios, XHR |
| TypeScript | fetch, Axios |
| Python | requests, http.client |
| Go | net/http |
| Rust | reqwest |
| Java | OkHttp, HttpClient |
| C# | HttpClient |
| PHP | cURL, Guzzle |
| Ruby | Net::HTTP |
| Swift | URLSession |
| Shell | HTTPie, wget |

### Implementation Plan

```typescript
// packages/core/src/codegen/code-generator.ts

interface CodeGenRequest {
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: { type: string; content: string };
  auth?: { type: string; [key: string]: unknown };
}

interface CodeSnippet {
  language: string;
  library: string;
  code: string;
}

type SnippetGenerator = (req: CodeGenRequest) => string;

const generators: Record<string, SnippetGenerator> = {
  'curl': (req) => {
    let cmd = `curl -X ${req.method} '${req.url}'`;
    for (const [k, v] of Object.entries(req.headers)) {
      cmd += ` \\\n  -H '${k}: ${v}'`;
    }
    if (req.body?.content) {
      cmd += ` \\\n  -d '${req.body.content}'`;
    }
    return cmd;
  },

  'javascript-fetch': (req) => {
    const opts: Record<string, unknown> = { method: req.method };
    if (Object.keys(req.headers).length > 0) opts.headers = req.headers;
    if (req.body?.content) opts.body = req.body.content;

    return `const response = await fetch('${req.url}', ${JSON.stringify(opts, null, 2)});
const data = await response.json();
console.log(data);`;
  },

  'python-requests': (req) => {
    const headers = Object.entries(req.headers)
      .map(([k, v]) => `    '${k}': '${v}'`)
      .join(',\n');

    let code = `import requests\n\nresponse = requests.${req.method.toLowerCase()}(\n    '${req.url}'`;
    if (headers) code += `,\n    headers={\n${headers}\n    }`;
    if (req.body?.content) code += `,\n    json=${req.body.content}`;
    code += `\n)\nprint(response.json())`;
    return code;
  },

  // ... additional generators for each language/library
};

function generateSnippet(language: string, req: CodeGenRequest): CodeSnippet {
  const generator = generators[language];
  if (!generator) throw new Error(`Unsupported language: ${language}`);
  return { language, library: language.split('-')[1] ?? language, code: generator(req) };
}
```

**Extensibility:** Each generator is a pure function `(request) → string`. New languages are added by registering a new generator function. Consider using the open-source `httpsnippet` library by Kong as a base.

**UI:** Slide-over panel triggered by a `</>` button on any request. Language/library dropdown at top, syntax-highlighted code block below, and a copy button.

---

## 18. Import & Export

**What it is:** Import collections from Postman, Insomnia, OpenAPI/Swagger specs, cURL commands, and HAR files. Export Nexus collections and environments as JSON.

**Why it matters:** Migration from Postman is the primary onboarding path. Users won't switch if they can't bring their existing collections.

### Supported Formats

| Direction | Format | Description |
|---|---|---|
| Import | Postman Collection v2.1 | Full collection with requests, folders, scripts, auth |
| Import | Postman Environment | Environment variables |
| Import | OpenAPI 3.0/3.1 | Generate collection from API spec |
| Import | Swagger 2.0 | Older API specs |
| Import | Insomnia v4 | Insomnia export format |
| Import | cURL | Single command → single request |
| Import | HAR | HTTP Archive (browser DevTools export) |
| Export | Nexus Collection JSON | Full collection export |
| Export | Nexus Environment JSON | Environment export |
| Export | Postman Collection v2.1 | For compatibility with Postman users |

### Implementation Plan

```typescript
// packages/core/src/import/postman-importer.ts

interface ImportResult {
  collections: NexusCollection[];
  environments: NexusEnvironment[];
  warnings: string[];
}

class PostmanImporter {
  import(data: unknown): ImportResult {
    const parsed = data as PostmanCollectionV21;
    const warnings: string[] = [];

    const collection: NexusCollection = {
      id: `col_${crypto.randomUUID().slice(0, 8)}`,
      name: parsed.info.name,
      description: parsed.info.description,
      variables: (parsed.variable ?? []).map(this.convertVariable),
      auth: parsed.auth ? this.convertAuth(parsed.auth) : undefined,
      preRequestScript: this.extractScript(parsed.event, 'prerequest'),
      postResponseScript: this.extractScript(parsed.event, 'test'),
      items: parsed.item.map(item => this.convertItem(item, warnings)),
    };

    return { collections: [collection], environments: [], warnings };
  }

  private convertItem(item: PostmanItem, warnings: string[]): NexusRequest | NexusFolder {
    if (item.item) {
      // It's a folder
      return {
        id: `folder_${crypto.randomUUID().slice(0, 8)}`,
        name: item.name,
        items: item.item.map(i => this.convertItem(i, warnings)),
        auth: item.auth ? this.convertAuth(item.auth) : undefined,
        preRequestScript: this.extractScript(item.event, 'prerequest'),
        postResponseScript: this.extractScript(item.event, 'test'),
      };
    }

    // It's a request
    return this.convertRequest(item, warnings);
  }

  private convertRequest(item: PostmanItem, warnings: string[]): NexusRequest {
    const req = item.request;
    return {
      id: `req_${crypto.randomUUID().slice(0, 8)}`,
      name: item.name,
      method: req.method as HttpMethod,
      url: typeof req.url === 'string' ? req.url : this.buildUrl(req.url),
      headers: (req.header ?? []).map(h => ({
        key: h.key, value: h.value, enabled: !h.disabled
      })),
      params: this.extractParams(req.url),
      body: this.convertBody(req.body),
      auth: req.auth ? this.convertAuth(req.auth) : { type: 'inherit' },
      preRequestScript: this.extractScript(item.event, 'prerequest'),
      postResponseScript: this.extractScript(item.event, 'test'),
      settings: {},
    };
  }
}

// packages/core/src/import/openapi-importer.ts
class OpenApiImporter {
  import(spec: OpenAPIV3.Document): ImportResult {
    const collection: NexusCollection = {
      id: `col_${crypto.randomUUID().slice(0, 8)}`,
      name: spec.info.title,
      description: spec.info.description,
      variables: [
        { key: 'base_url', value: spec.servers?.[0]?.url ?? '', enabled: true }
      ],
      items: [],
    };

    // Walk paths and operations
    for (const [path, pathItem] of Object.entries(spec.paths ?? {})) {
      const folder: NexusFolder = { id: `folder_${path}`, name: path, items: [] };

      for (const method of ['get', 'post', 'put', 'patch', 'delete'] as const) {
        const operation = pathItem?.[method];
        if (!operation) continue;

        folder.items.push({
          id: `req_${crypto.randomUUID().slice(0, 8)}`,
          name: operation.summary ?? `${method.toUpperCase()} ${path}`,
          method: method.toUpperCase() as HttpMethod,
          url: `{{base_url}}${path}`,
          headers: this.extractHeaders(operation),
          params: this.extractParams(operation),
          body: this.extractBody(operation),
          auth: { type: 'inherit' },
        });
      }

      collection.items.push(folder);
    }

    return { collections: [collection], environments: [], warnings: [] };
  }
}

// packages/core/src/import/curl-importer.ts
class CurlImporter {
  import(curlCommand: string): NexusRequest {
    // Parse curl flags: -X, -H, -d, --data, -u, -b, -F, -k, -L, etc.
    // Libraries like curlconverter/curlconverter can help
  }
}
```

**Dependencies:**
- `openapi-types` — TypeScript types for OpenAPI specs
- `yaml` — Parse YAML OpenAPI specs
- `@readme/har-to-postman` or custom — HAR parsing

**UI:** Import modal with drag-and-drop zone or file picker. Auto-detect format. Show preview of what will be imported with a confirmation step. Export via right-click on collection → Export.

---

## 19. Request History

**What it is:** Automatically log every request sent (URL, method, timestamp, status, response time) and allow re-opening or replaying past requests.

**Why it matters:** You often need to revisit a request you made earlier without having saved it. History provides a safety net and audit trail.

### Implementation Plan

```typescript
// packages/core/src/history/history-manager.ts

interface HistoryEntry {
  id: string;
  timestamp: number;
  request: {
    method: string;
    url: string;
    headers: Record<string, string>;
    body?: string;
  };
  response: {
    status: number;
    responseTime: number;
    size: number;
  };
  collectionId?: string;
  requestId?: string;
}

class HistoryManager {
  private basePath: string;        // ~/.nexus/history/

  async log(entry: HistoryEntry): Promise<void> {
    const date = new Date(entry.timestamp).toISOString().slice(0, 10);  // 2026-04-14
    const filePath = path.join(this.basePath, `${date}.jsonl`);
    await fs.appendFile(filePath, JSON.stringify(entry) + '\n');
  }

  async query(filters: HistoryFilter): Promise<HistoryEntry[]> {
    // Read JSONL files for the date range
    // Filter by method, URL pattern, status code
    // Return sorted by timestamp descending
  }

  async getEntry(id: string): Promise<HistoryEntry | null> {
    // Search through history files for a specific entry
  }

  async clear(before?: Date): Promise<void> {
    // Delete JSONL files older than the given date
  }
}
```

**Storage:** JSONL (JSON Lines) files organized by date. Each line is one history entry. This format is append-friendly and easy to grep.

**UI:** History sidebar tab with a search bar, method/status filters, and a chronological list. Clicking an entry opens it as a new request tab. "Save to Collection" action to promote a history entry to a saved request.

---

## 20. API Documentation Generation

**What it is:** Auto-generate browsable API documentation from a collection. Documentation includes endpoint descriptions, request parameters, example responses, and authentication details.

**Why it matters:** Good API docs save everyone time. Generating them from collections means the docs are always in sync with what you're actually testing.

### Implementation Plan

```typescript
// packages/core/src/docs/doc-generator.ts

interface DocPage {
  title: string;
  description?: string;
  baseUrl: string;
  groups: DocGroup[];              // Mapped from folders
}

interface DocGroup {
  name: string;
  description?: string;
  endpoints: DocEndpoint[];
}

interface DocEndpoint {
  name: string;
  method: string;
  path: string;
  description?: string;
  headers: { key: string; value: string; description?: string }[];
  params: { key: string; value: string; description?: string }[];
  body?: { type: string; example: string; schema?: object };
  auth?: string;
  responses: DocResponse[];
}

interface DocResponse {
  name: string;
  status: number;
  headers: Record<string, string>;
  body: string;
}

class DocGenerator {
  generateMarkdown(collection: NexusCollection): string {
    // Walk collection tree
    // Generate markdown with:
    //   - Table of contents
    //   - For each endpoint: method badge, URL, description, parameter tables,
    //     request body example, response examples
  }

  generateHtml(collection: NexusCollection): string {
    // Generate a single-page HTML doc using a template engine (EJS or Handlebars)
    // Include syntax-highlighted JSON examples
    // Style with a clean, modern CSS (similar to Swagger UI)
  }

  generateOpenApi(collection: NexusCollection): OpenAPIV3.Document {
    // Reverse-engineer an OpenAPI spec from the collection
    // Useful for sharing API definitions
  }
}
```

**UI:** "Generate Docs" action on a collection. Renders in-app as a browsable documentation page. Export buttons for Markdown, HTML, and OpenAPI formats.

---

## 21. CLI Runner (Newman Equivalent)

**What it is:** A standalone command-line tool that runs Nexus collections outside the desktop app — in CI/CD pipelines, scripts, or terminal workflows.

**Why it matters:** CI/CD integration is essential. You want to run your API tests as part of every deploy. The CLI makes collections executable in any automated environment.

### Commands

```bash
# Run a collection
nexus run ./path/to/collection.json -e ./environments/prod.json

# Run with iterations and data file
nexus run ./collection.json -n 5 --data users.csv -e staging.json

# Run with specific folder
nexus run ./collection.json --folder "User CRUD" -e dev.json

# Run with reporters
nexus run ./collection.json -r cli,json,html --reporter-json-export results.json

# Import a Postman collection
nexus import ./postman-collection.json --format postman-v2.1

# Generate docs
nexus docs ./collection.json --format html --output docs/api.html

# Start a mock server
nexus mock ./collection.json --port 3100
```

### Implementation Plan

```typescript
// packages/cli/src/index.ts
import { Command } from 'commander';

const program = new Command()
  .name('nexus')
  .version('1.0.0')
  .description('Nexus API Client — CLI Runner');

program
  .command('run <collection>')
  .description('Run a collection')
  .option('-e, --environment <path>', 'Environment file')
  .option('-n, --iterations <count>', 'Number of iterations', '1')
  .option('-d, --data <path>', 'Data file (CSV or JSON)')
  .option('--delay <ms>', 'Delay between requests', '0')
  .option('--folder <name>', 'Run only a specific folder')
  .option('--bail', 'Stop on first failure')
  .option('-r, --reporters <types>', 'Reporters: cli, json, html, junit', 'cli')
  .option('--reporter-json-export <path>', 'JSON report output path')
  .option('--reporter-html-export <path>', 'HTML report output path')
  .option('--reporter-junit-export <path>', 'JUnit XML report output path')
  .action(async (collectionPath, opts) => {
    // 1. Load collection from disk
    // 2. Load environment if specified
    // 3. Parse data file if specified
    // 4. Create CollectionRunner instance (from @nexus/core)
    // 5. Wire up reporter(s) to runner events
    // 6. Execute run
    // 7. Exit with code 0 if all tests pass, 1 if any fail
  });

// Reporter interface
interface Reporter {
  onRunStart(info: RunInfo): void;
  onIterationStart(iteration: number): void;
  onRequestStart(request: NexusRequest): void;
  onRequestComplete(request: NexusRequest, response: NexusResponse, tests: TestResult[]): void;
  onRunComplete(summary: RunSummary): void;
}

// CLI Reporter — prints colored output to terminal
class CliReporter implements Reporter {
  onRequestComplete(req, res, tests) {
    const status = res.status;
    const color = status < 400 ? chalk.green : chalk.red;
    console.log(`  ${color(req.method)} ${req.url} [${status}] ${res.timing.total}ms`);
    for (const test of tests) {
      const icon = test.passed ? chalk.green('✓') : chalk.red('✗');
      console.log(`    ${icon} ${test.name}`);
    }
  }
}

// JSON Reporter — writes results to a JSON file
// HTML Reporter — generates an HTML report using a template
// JUnit Reporter — generates JUnit XML for CI systems (Jenkins, GitHub Actions)
```

**Dependencies:**
- `commander` — CLI argument parsing
- `chalk` — Terminal colors
- `ora` — Spinner for progress indication

---

## 22. Response Visualization

**What it is:** A programmable visualization engine that renders response data as tables, charts, or custom HTML inside the response viewer.

**Why it matters:** Raw JSON is hard to scan when dealing with lists, metrics, or tabular data. Visualizations turn response data into meaningful views without leaving the tool.

### Implementation Plan

```typescript
// packages/core/src/visualizer/visualizer.ts
import Handlebars from 'handlebars';

interface VisualizerConfig {
  template: string;                // Handlebars HTML template
  data: unknown;                   // Data to bind to the template
}

// Available in post-response scripts as:
// nx.visualizer.set(template, data)

class Visualizer {
  render(config: VisualizerConfig): string {
    const compiled = Handlebars.compile(config.template);
    return compiled(config.data);
  }
}

// Example usage in a post-response script:
// const template = `
//   <table>
//     <tr><th>Name</th><th>Email</th></tr>
//     {{#each users}}
//       <tr><td>{{name}}</td><td>{{email}}</td></tr>
//     {{/each}}
//   </table>
// `;
// nx.visualizer.set(template, { users: nx.response.json() });
```

**UI:** A "Visualize" tab in the response viewer that renders the Handlebars HTML in a sandboxed `<webview>` or `<iframe>`. Users can load external CSS/JS (e.g., Chart.js) for rich charts. Include a few built-in templates: table view, chart view, timeline view.

---

## 23. Certificate & SSL Management

**What it is:** Configure client certificates (mTLS), custom CA certificates, and SSL verification settings for requests to servers with non-standard TLS configurations.

**Why it matters:** Enterprise APIs and internal services often use self-signed certificates or require mutual TLS. Without proper SSL configuration, requests to these services fail.

### Implementation Plan

```typescript
// packages/core/src/ssl/certificate-manager.ts

interface ClientCertificate {
  id: string;
  name: string;
  host: string;                    // Hostname pattern to match
  certPath: string;                // PEM certificate file
  keyPath: string;                 // PEM private key file
  passphrase?: string;             // Key passphrase if encrypted
  pfxPath?: string;                // PFX/PKCS12 bundle (alternative to cert+key)
}

interface SslConfig {
  rejectUnauthorized: boolean;     // Verify server certificate
  caCertPath?: string;             // Custom CA certificate bundle
  clientCertificates: ClientCertificate[];
}

class CertificateManager {
  private config: SslConfig;
  private configPath: string;      // ~/.nexus/certificates.json

  getAgentOptions(hostname: string): https.AgentOptions {
    const cert = this.config.clientCertificates.find(c =>
      this.matchHost(hostname, c.host)
    );

    return {
      rejectUnauthorized: this.config.rejectUnauthorized,
      ca: this.config.caCertPath ? fs.readFileSync(this.config.caCertPath) : undefined,
      cert: cert?.certPath ? fs.readFileSync(cert.certPath) : undefined,
      key: cert?.keyPath ? fs.readFileSync(cert.keyPath) : undefined,
      passphrase: cert?.passphrase,
      pfx: cert?.pfxPath ? fs.readFileSync(cert.pfxPath) : undefined,
    };
  }

  private matchHost(hostname: string, pattern: string): boolean {
    // Support wildcards: *.example.com matches api.example.com
    const regex = new RegExp('^' + pattern.replace(/\*/g, '.*') + '$');
    return regex.test(hostname);
  }
}
```

**UI:** Settings page → Certificates tab. Table of client certificates with host pattern, cert file path, key file path. "Add Certificate" form with file pickers. Global toggle for SSL verification. Custom CA certificate file picker.

---

## 24. Console & Debugging

**What it is:** A developer console that shows raw request/response data, script logs (`console.log`), variable resolution, and timing for every request sent.

**Why it matters:** When something goes wrong — wrong variable value, unexpected header, script error — the console is where you diagnose it. It's the DevTools of Nexus.

### Feature Breakdown

| Sub-Feature | Description |
|---|---|
| Request log | Full HTTP request as sent (after variable resolution) |
| Response log | Full HTTP response (status, headers, body) |
| Script logs | Output from `console.log()` in pre-request and post-response scripts |
| Variable resolution trace | Show which variables were resolved and from which scope |
| Errors & warnings | Script errors, network errors, SSL issues |
| Timestamps | Precise timestamps for each log entry |
| Filter & search | Filter by type (request, response, log, error), search content |
| Clear | Clear the console log |

### Implementation Plan

```typescript
// packages/core/src/console/nexus-console.ts

type LogLevel = 'info' | 'warn' | 'error' | 'debug';
type LogSource = 'script' | 'http' | 'variable' | 'system';

interface ConsoleEntry {
  id: string;
  timestamp: number;
  level: LogLevel;
  source: LogSource;
  requestId?: string;
  message: string;
  data?: unknown;                  // Structured data (e.g., full request/response object)
}

class NexusConsole extends EventEmitter {
  private entries: ConsoleEntry[] = [];
  private maxEntries: number = 10_000;

  log(level: LogLevel, source: LogSource, message: string, data?: unknown, requestId?: string): void {
    const entry: ConsoleEntry = {
      id: crypto.randomUUID(),
      timestamp: Date.now(),
      level,
      source,
      requestId,
      message,
      data,
    };
    this.entries.push(entry);
    if (this.entries.length > this.maxEntries) this.entries.shift();
    this.emit('entry', entry);
  }

  query(filters: ConsoleFilter): ConsoleEntry[] {
    return this.entries.filter(e => {
      if (filters.level && e.level !== filters.level) return false;
      if (filters.source && e.source !== filters.source) return false;
      if (filters.requestId && e.requestId !== filters.requestId) return false;
      if (filters.search && !e.message.includes(filters.search)) return false;
      return true;
    });
  }

  clear(): void {
    this.entries = [];
    this.emit('clear');
  }
}
```

**Integration points:**
- **HTTP sender** logs the raw request (after variable resolution) and raw response
- **Variable resolver** logs each `{{var}}` → resolved value (debug level)
- **Script sandbox** captures `console.log/warn/error` and routes to NexusConsole
- **Auth handlers** log token refresh, OAuth flow steps

**UI:** Bottom panel (like browser DevTools console). Each entry shows timestamp, colored level badge, source icon, and message. Expandable entries for structured data (request/response objects rendered as collapsible JSON). Filter bar at top.

---

## Technology Stack Summary

| Layer | Technology | Version |
|---|---|---|
| Language | TypeScript | 5.x (strict mode) |
| Runtime | Node.js | 20+ LTS |
| Desktop Shell | Electron | 30+ |
| UI Framework | React | 18+ |
| Build Tool | Vite | 5+ |
| State Management | Zustand | 4+ |
| Styling | Tailwind CSS | 3+ |
| Component Primitives | Radix UI | latest |
| Code Editor | CodeMirror | 6 |
| HTTP Client | undici | 6+ |
| WebSocket | ws | 8+ |
| gRPC | @grpc/grpc-js | 1.9+ |
| Script Sandbox | isolated-vm | 4+ |
| Assertions | Custom (Chai-like) | — |
| JSON Schema | Ajv | 8+ |
| Cookie Jar | tough-cookie | 4+ |
| CLI | commander | 12+ |
| Faker Data | @faker-js/faker | 8+ |
| Monorepo | npm workspaces or Turborepo | — |

---

## Development Phases

### Phase 1 — Foundation (Weeks 1-4)
- Core architecture and monorepo setup
- Request builder & sender (HTTP engine with undici)
- Response viewer (JSON/text/headers/timing)
- Basic collections & folders (file-based storage)
- Environment variables with interpolation

### Phase 2 — Power Features (Weeks 5-8)
- Authentication (Bearer, Basic, API Key, OAuth2)
- Pre-request & post-response scripts with sandbox
- Test assertions engine
- Cookie management
- Request history
- Console & debugging

### Phase 3 — Advanced Protocols (Weeks 9-12)
- GraphQL client with introspection
- WebSocket client
- SSE client
- gRPC client
- Collection Runner with data files

### Phase 4 — Productivity (Weeks 13-16)
- Code snippet generation
- Import (Postman, OpenAPI, cURL, Insomnia)
- Export (Nexus JSON, Postman-compatible)
- Mock server
- API documentation generation
- CLI runner with reporters

### Phase 5 — Polish (Weeks 17-20)
- Proxy & traffic capture
- Certificate & SSL management
- Response visualization
- Keyboard shortcuts & command palette
- Themes (light/dark)
- Performance optimization & memory profiling
