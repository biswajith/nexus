<h1 align="center">Nexus</h1>

<p align="center">
  <strong>A local-first, offline API development tool.</strong><br>
  No cloud sync. No accounts. Your data stays on your machine as plain JSON files.
</p>

<p align="center">
  A single native binary written in Rust with egui. No Node, no webview, no Electron.
</p>

---

## Why Nexus?

- **Local-first** - Collections and environments are stored as plain JSON files on disk, fully Git-friendly.
- **Zero accounts** - No sign-up, no cloud sync, no telemetry.
- **Light** - One native process, about 22 MB on disk (see the measurements below).
- **Import compatible** - Import collections, environments, and scripts from other tools. Automatic `pm.*` to `nx.*` transpilation.
- **Full protocol support** - HTTP, GraphQL (incl. subscriptions), WebSocket, and an MCP client in one tool.
- **Scriptable** - Pre-request/post-response JavaScript scripts with a sandboxed `nx.*` API and Chai-like assertions.

---

## Features

### Request Building & Sending
- HTTP client (`reqwest` + rustls) with manual redirect handling and DNS / TCP / TLS / TTFB / download timing
- Methods: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS
- URL bar with `{{variable}}` highlighting (resolved vs unresolved, hover shows the resolved URL); pasting a `curl ...` command fills the request
- Query parameters and the URL stay in sync in both directions
- Key-value editors for headers, params and urlencoded bodies
- Body modes: JSON, XML, text, HTML, form-data (text and file rows), URL-encoded, binary file, GraphQL
- Send / Cancel, cURL export of the resolved request, Save back to the collection, unsaved-changes dot on tabs

### Code Editor
- Custom editor widget with syntax highlighting (JSON, JavaScript, GraphQL, XML, HTML), line numbers, bracket matching, auto-close / auto-indent, undo / redo, find, comment toggle, JSON formatting
- Autocomplete for `{{variables}}` and dynamic variables, and for the `nx.*` / `console.*` API in scripts
- Only visible lines are laid out, so a 1 MB body edits at the same speed as a small one

### Collections & Environments
- File-based storage at `~/.nexus/` - Git-friendly, human-readable JSON
- Nested folder organization with collection-level auth and scripts
- Layered variable system: local, environment, collection, global
- 118 dynamic variables (`{{$randomFirstName}}`, `{{$randomEmail}}`, `{{$timestamp}}`, etc.); values are shape-compatible with Postman, not identical
- Environment manager (create, rename, duplicate, delete, activate; secret variables are masked) and a quick-switch in the toolbar

### Authentication
- No Auth, Inherit, API Key, Bearer Token, Basic Auth, Digest Auth, AWS Signature v4
- OAuth 2.0 token type sends a static access token you supply (no grant flows, PKCE or refresh)
- Auth inheritance: request, folder, collection

### Scripting & Testing
- Pre-request and post-response JavaScript scripts in a QuickJS sandbox (`rquickjs`) with the `nx.*` API, a timeout and a memory limit
- Chai-like assertion engine: `nx.expect()`, `nx.test()`; results shown in the response Tests tab
- JSON Schema validation
- Console panel with `console.log/warn/error` capture plus HTTP request / response entries, level and text filter
- `nx.variables.replaceIn()` for dynamic variable resolution in scripts
- `nx.visualizer.set(template, data)` for custom response rendering (opens in the system browser)

### Response Viewer
- Status, time and size; Body (Pretty / Raw / Preview), Headers, Tests, Timing waterfall, Visualize
- Search in the body with next / previous, copy, save to file
- Images are rendered inline; HTML opens in the system browser (scripts blocked)
- Virtualised: a 10 MB JSON response is pretty-printed off the UI thread and scrolls smoothly

### Protocols
- **GraphQL** - query/mutation editor, variables, headers, schema introspection browser (feeds autocomplete), subscriptions (`graphql-transport-ws`)
- **WebSocket** - several connection tabs, headers and subprotocols, message composer, filterable message log, auto-reconnect
- **MCP** - connect to stdio or streamable-HTTP MCP servers; list and call tools (forms generated from the JSON schema), list resources and prompts

### Collection Runner
- Execute a collection or one folder with configurable iterations and delay
- CSV / JSON data files (file picker): each row feeds `{{variables}}`, iterations default to the row count
- Stop-on-error, workflow control via `nx.execution.setNextRequest()`
- Live progress, per-request results with tests, errors and script logs

### Import & Export
- **Import**: Postman Collection v2.1, Postman Environment, OpenAPI 3.x, Swagger 2.0 (JSON or YAML), cURL commands; file picker, drag a file onto the window, or paste text; preview before importing
- **Export**: Nexus JSON, Postman Collection v2.1, environments
- Bidirectional `pm.*` / `nx.*` script transpilation

### Productivity
- Command palette with fuzzy search over requests, history, panels, environments and actions (Ctrl+K)
- cURL code generation from any request
- API documentation generation (Markdown and HTML)
- Request history
- Light / Dark / System theme (toolbar button "Theme: ..."; the choice is remembered)
- Open tabs, sidebar width, splitters and panel forms are restored on the next start (`~/.nexus/ui-state.json`)

Not implemented: SSE, gRPC, mock server, proxy/traffic capture, cookie manager UI, CLI runner, code folding in the editor, per-request settings UI (timeout / redirects / SSL), MCP resource reading and prompt rendering, a settings dialog.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Engine | Rust (`crates/nexus-core`, no UI dependency) |
| UI | egui / eframe 0.37 (glow, Wayland + X11), custom painted widgets, Inter + JetBrains Mono |
| Accessibility | AccessKit (eframe `accesskit` feature) |
| UI tests | egui_kittest (widgets found by role + label) |
| Async / HTTP | tokio, reqwest (rustls) |
| WebSocket | tokio-tungstenite |
| Scripts | rquickjs (QuickJS) |
| Schema | jsonschema |
| Cookies | cookie_store |
| MCP | rmcp |
| Faker | fake + custom tables |
| File dialogs | rfd (xdg-portal, no GTK) |

---

## Project Structure

```
nexus/
├── Cargo.toml                   # Rust workspace
├── start.sh                     # cargo run --release -p nexus-gui
├── crates/
│   ├── nexus-core/              # Engine library (no UI)
│   │   └── src/
│   │       ├── auth/            # Auth handlers (Bearer, Basic, Digest, AWS SigV4) + token store
│   │       ├── codegen/         # cURL generator
│   │       ├── collections/     # Collection CRUD, file I/O
│   │       ├── console/         # Console logging engine
│   │       ├── cookies/         # Cookie jar (not wired to the UI)
│   │       ├── docs/            # API doc generator (Markdown, HTML)
│   │       ├── environments/    # Variable store, dynamic variables, environment store
│   │       ├── fs/              # Private (0600/0700) file helpers
│   │       ├── graphql/         # GraphQL client + subscriptions
│   │       ├── history/         # Request history manager
│   │       ├── http/            # HTTP sender (reqwest)
│   │       ├── import/          # Importers (Postman, OpenAPI, cURL) + exporter
│   │       ├── mcp/             # MCP client
│   │       ├── response/        # Formatting helpers
│   │       ├── runner/          # Collection runner engine
│   │       ├── scripts/         # QuickJS sandbox + assertion engine
│   │       ├── service/         # `App`: the API the GUI calls (CRUD, http_send pipeline, runner, ws, graphql, mcp)
│   │       ├── types/           # Shared serde types
│   │       ├── visualizer/      # Response visualization engine
│   │       └── websocket/       # WebSocket client
│   └── nexus-gui/               # The egui app (binary `nexus`)
│       ├── assets/              # fonts, nexus.desktop, nexus.png
│       ├── src/
│       │   ├── app.rs           # NexusApp state + frame loop
│       │   ├── runtime.rs       # tokio runtime, results come back as messages
│       │   ├── state/           # plain state structs (+ persisted ui-state)
│       │   ├── panels/          # toolbar, sidebar, tabs, request, response, console, runner, ws, graphql, mcp, dialogs/
│       │   ├── widgets/         # painted widgets, code editor, code view, accessibility helpers
│       │   ├── editor/          # editor model and completion logic
│       │   └── theme.rs         # dark / light palettes
│       └── tests/               # egui_kittest suites
└── FEATURES.md                  # Original feature spec (see its status note)
```

---

## Data Storage

All data is stored locally at `~/.nexus/` (directories 0700, files 0600):

```
~/.nexus/
├── collections/       # One directory per collection
├── environments/      # Environment JSON files
├── history/           # Daily JSONL history logs
├── tmp/               # Short-lived HTML previews (swept after a day)
├── ui-state.json      # Open tabs, theme, sidebar width, panel forms
└── tokens.json        # Token store
```

Collections are plain JSON - version control them with Git, share them with your team, or back them up however you like.

---

## Getting Started

### Prerequisites

- **Rust** (stable, via [rustup](https://rustup.rs)). That is all: no Node.js, no webview, no GTK.
- Linux: a Wayland or X11 session with OpenGL (Mesa). The usual runtime libraries (libxkbcommon, libwayland / libX11, libEGL, libGL) are expected to be present.

### Run

```bash
git clone https://github.com/biswajith/nexus.git
cd nexus
./start.sh
```

`start.sh` checks for cargo and runs `cargo run --release -p nexus-gui`.

### Commands

| Command | Description |
|---|---|
| `./start.sh` | Build (release) and run the app |
| `cargo run -p nexus-gui` | Debug build and run (enables the dev flags such as `--seed-demo`) |
| `cargo build --release -p nexus-gui` | Release binary at `target/release/nexus` |
| `cargo test --workspace` | Run all tests (engine and GUI) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint |
| `cargo fmt --all --check` | Check formatting |

Install the desktop entry and icon (optional): copy `crates/nexus-gui/assets/nexus.desktop` to `~/.local/share/applications/`, `nexus.png` to `~/.local/share/icons/hicolor/512x512/apps/`, and put the `nexus` binary on your `PATH`.

Dev flags (all builds): `--home <dir>` uses another data directory, `--screenshot <png>` renders one frame to a file and exits, plus `--size WxH`, `--theme dark|light|system`, `--panel http|graphql|ws|mcp|runner`.

---

## Keyboard Shortcuts

Press `F1` (or `?` outside a text field) in the app for the same list.

| Shortcut | Action |
|---|---|
| `Ctrl+K` | Command palette |
| `Ctrl+N` / `Ctrl+T` | New request tab |
| `Ctrl+W` | Close active tab |
| `Ctrl+S` | Save request |
| `Ctrl+Enter` / `Enter` in the URL bar | Send request |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+J` | Toggle console |
| `Ctrl+L` | Focus URL bar |
| `Ctrl+1` to `Ctrl+5` | Switch panels (HTTP, GraphQL, WebSocket, MCP, Runner) |
| `Ctrl+Shift+E` | Focus environment selector |
| `Ctrl+F` | Find in the response body or the focused editor |
| `Ctrl+Space` | Autocomplete in editors |
| `Ctrl+/` | Toggle comment in editors |
| `Alt+Shift+F` | Format JSON in the body editor |
| `Esc`, then `Tab` (or `Ctrl+M`) | Leave an editor with the keyboard (Tab otherwise indents) |
| `F1` | Shortcut help |
| `Esc` | Close palette / dialogs / search |

---

## Accessibility

- Every interactive widget, including the custom-painted ones (collection tree rows, tabs, method badges, the code editor and viewer), is exposed to AccessKit with a role and a label. The test suite finds widgets only by role and label and has an inventory test that fails on any unnamed interactive node.
- Keyboard: everything in the shortcut table is reachable without a mouse, dialogs trap focus and close with `Esc`, editors have a documented focus escape (`Esc`, then `Tab`).
- Colours: both themes are tested against WCAG AA (4.5:1) for text, method colours and syntax colours.
- **Not verified:** live screen-reader exposure. On Linux the AccessKit tree is only published when the desktop accessibility flag is on (`org.a11y.Status IsEnabled`, normally switched on by starting Orca). The tree is verified in tests, but the app has not been exercised with Orca or another real screen reader.

---

## Dynamic Variables

Use `{{$variableName}}` anywhere in requests. Full Postman-shaped set including:

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

118 dynamic variables total. See `crates/nexus-core/src/environments/dynamic.rs` for the complete list.

---

## License

Private — All rights reserved.
