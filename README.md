<h1 align="center">Nexus</h1>

<p align="center">
  <strong>A local-first, offline API development tool.</strong><br>
  No cloud sync. No accounts. Your data stays on your machine as plain JSON files.
</p>

<p align="center">
  Built as a modern <a href="https://www.postman.com">Postman</a> replacement with TypeScript, Electron, React 19, and Adobe React Spectrum S2.
</p>

---

## Why Nexus?

- **Local-first** — Collections and environments are stored as plain JSON files on disk, fully Git-friendly.
- **Zero accounts** — No sign-up, no cloud sync, no telemetry.
- **Postman compatible** — Import Postman collections, environments, and scripts. Automatic `pm.*` → `nx.*` transpilation.
- **Full protocol support** — HTTP, GraphQL, WebSocket, and SSE in one tool.
- **Scriptable** — Pre-request/post-response JavaScript scripts with a sandboxed `nx.*` API and Chai-like assertions.

---

## Features

### Request Building & Sending
- Full HTTP client powered by `undici` with DNS/TCP/TLS/TTFB/download timing breakdown
- Method selector (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS)
- URL bar with `{{variable}}` interpolation
- Key-value editors for headers and query parameters
- Body modes: JSON, XML, text, HTML, URL-encoded, form-data, binary, GraphQL, none
- Request settings: follow redirects, timeout, SSL verification, max redirects

### Collections & Environments
- File-based storage at `~/.nexus/` — Git-friendly, human-readable JSON
- Nested folder organization with collection-level auth and scripts
- Layered variable system: local → environment → collection → global
- 119 dynamic variables (`{{$randomFirstName}}`, `{{$randomEmail}}`, `{{$timestamp}}`, etc.) — full Postman parity via `@faker-js/faker`
- Environment quick-switch from the toolbar

### Authentication
- No Auth, API Key, Bearer Token, Basic Auth
- OAuth 2.0 (Authorization Code, Client Credentials, Password, PKCE)
- Digest Auth, AWS Signature v4
- Auth inheritance: request → folder → collection

### Scripting & Testing
- Pre-request and post-response JavaScript scripts
- Sandboxed execution via `node:vm` with the `nx.*` API
- Chai-like assertion engine: `nx.expect()`, `nx.test()`
- JSON Schema validation via `ajv`
- Script console with `console.log/warn/error` capture
- `nx.variables.replaceIn()` for dynamic variable resolution in scripts
- `nx.visualizer.set(template, data)` for custom response rendering

### Advanced Protocols
- **GraphQL** — query/mutation editor, variables pane, schema introspection, WebSocket subscriptions
- **WebSocket** — connect/disconnect, message composer, bidirectional message history, auto-reconnect
- **Server-Sent Events (SSE)** — real-time event stream viewer with filtering

### Collection Runner
- Execute collections with configurable iterations and delays
- CSV/JSON data file support for data-driven testing
- Stop-on-error, workflow control via `nx.execution.setNextRequest()`
- Real-time progress and test result streaming

### Import & Export
- **Import**: Postman Collection v2.1, Postman Environment, OpenAPI 3.x, Swagger 2.0, cURL commands
- **Export**: Nexus JSON, Postman Collection v2.1
- Bidirectional `pm.*` ↔ `nx.*` script transpilation for Postman compatibility

### Productivity
- cURL code generation from any request
- API documentation generation (Markdown and HTML)
- Response visualization with Mustache-like templates
- Request history with search
- Command palette with fuzzy search (Cmd/Ctrl+K)
- Light / Dark / System theme toggle

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | TypeScript 5 (strict mode) |
| Runtime | Node.js 20+ |
| Desktop | Electron 35 |
| UI | React 19 + Adobe React Spectrum S2 |
| Build | Vite 6 |
| State | Zustand 5 |
| HTTP | undici 7 |
| WebSocket | ws 8 |
| GraphQL | graphql + graphql-ws |
| Editor | CodeMirror 6 |
| Cookies | tough-cookie |
| Faker | @faker-js/faker 9 |
| Schema | Ajv 8 + ajv-formats |
| Monorepo | npm workspaces + Turborepo |

---

## Project Structure

```
nexus/
├── packages/
│   ├── core/                    # Pure TypeScript engine (no UI)
│   │   └── src/
│   │       ├── auth/            # Auth handlers (Bearer, Basic, OAuth2, Digest, AWS)
│   │       ├── codegen/         # cURL generator
│   │       ├── collections/     # Collection CRUD, file I/O
│   │       ├── console/         # Console logging engine
│   │       ├── cookies/         # Cookie jar (tough-cookie)
│   │       ├── docs/            # API doc generator (Markdown, HTML)
│   │       ├── environments/    # Variable store + 119 dynamic variables
│   │       ├── graphql/         # GraphQL client
│   │       ├── history/         # Request history manager
│   │       ├── http/            # HTTP sender (undici)
│   │       ├── import/          # Importers (Postman, OpenAPI, cURL) + exporter
│   │       ├── runner/          # Collection runner engine
│   │       ├── scripts/         # Script sandbox + assertion engine
│   │       ├── types/           # Shared TypeScript interfaces
│   │       ├── visualizer/      # Response visualization engine
│   │       └── websocket/       # WebSocket client
│   └── desktop/                 # Electron + React app
│       └── src/
│           ├── main/            # Electron main process + IPC handlers
│           ├── preload/         # Secure IPC bridge (contextBridge)
│           └── renderer/        # React UI
│               ├── components/  # All UI components
│               ├── hooks/       # Custom React hooks
│               ├── stores/      # Zustand stores
│               └── styles/      # Global CSS + themes
├── package.json                 # Monorepo root
├── turbo.json                   # Turborepo config
├── tsconfig.base.json           # Shared TS config
└── FEATURES.md                  # Full feature specification
```

---

## Data Storage

All data is stored locally at `~/.nexus/`:

```
~/.nexus/
├── collections/       # Collection JSON files
├── environments/      # Environment JSON files
├── history/           # Daily JSONL history logs
├── tokens.json        # OAuth token store
└── settings.json      # Global settings
```

Collections are plain JSON — version control them with Git, share them with your team, or back them up however you like.

---

## Getting Started

### Prerequisites

- **Node.js** 20 or later
- **npm** 9 or later

### Quick Start

```bash
# Clone the repository
git clone https://github.com/biswajith/nexus.git
cd nexus

# Install dependencies
npm install

# Build the core package
npm run build:core

# Start the desktop app in dev mode
npm run dev
```

### Available Scripts

| Command | Description |
|---|---|
| `npm run dev` | Start Vite dev server for the renderer |
| `npm run build` | Build core + desktop for production |
| `npm run build:core` | Build the core package only |
| `npm run build:desktop` | Build the desktop package only |
| `npm run typecheck` | Type-check all packages |
| `npm run lint` | Lint all packages |
| `npm run clean` | Remove dist folders and Vite cache |

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Cmd/Ctrl+K` | Command palette |
| `Cmd/Ctrl+N` | New request tab |
| `Cmd/Ctrl+W` | Close active tab |
| `Cmd/Ctrl+Enter` | Send request |
| `Cmd/Ctrl+B` | Toggle sidebar |
| `Cmd/Ctrl+J` | Toggle console |
| `Cmd/Ctrl+L` | Focus URL bar |
| `Cmd/Ctrl+1-5` | Switch panels (HTTP, GraphQL, WebSocket, SSE, Runner) |
| `Cmd/Ctrl+Shift+E` | Focus environment selector |
| `Escape` | Close palette / modals |

---

## Dynamic Variables

Use `{{$variableName}}` anywhere in requests. Full Postman-compatible set including:

| Category | Examples |
|---|---|
| **Names** | `$randomFirstName`, `$randomLastName`, `$randomFullName` |
| **Contact** | `$randomEmail`, `$randomPhoneNumber`, `$randomUserName`, `$randomUrl` |
| **Address** | `$randomCity`, `$randomStreetAddress`, `$randomCountry`, `$randomLatitude` |
| **Business** | `$randomCompanyName`, `$randomJobTitle`, `$randomBs`, `$randomCatchPhrase` |
| **Finance** | `$randomBankAccount`, `$randomCurrencyCode`, `$randomBitcoin` |
| **Text** | `$randomWord`, `$randomWords`, `$randomLoremSentence`, `$randomLoremParagraph` |
| **Common** | `$guid`, `$timestamp`, `$isoTimestamp`, `$randomUUID`, `$randomInt`, `$randomBoolean` |
| **Internet** | `$randomIP`, `$randomIPV6`, `$randomMACAddress`, `$randomPassword` |

119 dynamic variables total. See `packages/core/src/environments/variable-store.ts` for the complete list.

---

## License

Private — All rights reserved.
