# User Interface Overview

Nexus has a three-pane layout designed for efficient API development.

## Layout Structure

```
┌──────────────────────────────────────────────────────────┐
│  Title Bar                                               │
├──────────────────────────────────────────────────────────┤
│  Toolbar (panels, environment selector, theme, console)  │
├────────────┬─────────────────────────────────────────────┤
│            │  Tab Bar (request tabs)                      │
│            ├─────────────────────────────────────────────┤
│  Sidebar   │  Request Builder (URL, params, headers,     │
│            │  body, auth, scripts)                        │
│            ├─ ─ ─ ─ ─ ─ ─ ─ ─ ─ (draggable divider) ─ ─┤
│            │  Response Viewer (body, headers, tests,      │
│            │  timing, visualize)                          │
├────────────┴─────────────────────────────────────────────┤
│  Console Panel (collapsible)                              │
└──────────────────────────────────────────────────────────┘
```

## Title Bar

A slim bar with the app name; window decorations come from your desktop environment.

## Toolbar

The toolbar spans the full width and contains:

- **Sidebar toggle** — Show/hide the sidebar (`Ctrl+B`).
- **Panel tabs** — Switch between HTTP, GraphQL, WebSocket, MCP, and Runner panels (`Ctrl+1` through `Ctrl+5`).
- **Import button** — Opens the import/export modal for bringing in external, OpenAPI, cURL, or Nexus collections.
- **Docs button** — Opens the API documentation generator.
- **Environment selector** — Dropdown to pick the active environment, with a **Manage** button that opens the environment manager. Appears in the center of the toolbar.
- **Theme button** — Labelled "Theme: Dark", "Theme: Light" or "Theme: System"; click to cycle. The choice is remembered.
- **Console toggle** — Show/hide the console panel (`Ctrl+J`).

## Sidebar

The sidebar (left pane, 300px default, resizable by dragging its edge, collapsible with `Ctrl+B`) has two tabs:

### Collections Tab
- Displays all collections as an expandable tree.
- Nested folders and requests shown with depth-based indentation.
- Requests display their HTTP method with color coding.
- Right-click context menus (or the "More actions" button) for adding requests, saving the current tab, renaming, and deleting.
- Inline input fields for creating and renaming collections.

Environments are managed in a dialog (toolbar **Manage**): list with variable counts; create, rename, duplicate, delete (with confirmation), activate; the editor has a name and a variable grid. Variables support String and Secret types (secrets are masked, with Show / Hide).

### History Tab
- Chronological list of all sent requests.
- Each entry shows method, URL, status code, and timestamp.
- Click to re-open as a new request tab.
- Only the visible rows are drawn, so long histories stay fast.
- Clear button to purge history.

## Main Editor Area

The main area switches between panels:

### HTTP Panel (Default)
- **Tab Bar** — Multiple request tabs with method badges, names, dirty indicators, and close buttons.
- **Request Builder** (top half) — Method selector, URL bar, Send/Cancel buttons, cURL copy, Save. Sub-tabs for Params, Headers, Body, Auth, and Scripts.
- **Response Viewer** (bottom half) — Status badge, response time, size. Tabs for Body, Headers, Tests, Timing, and Visualize.
- A draggable divider between request and response panes allows resizing.

### GraphQL Panel
- URL input with Send, Schema (introspect), and Subscribe buttons.
- Code editors for query and variables, headers table, and a searchable schema browser.
- Response tabs: Response data, Schema SDL, Subscription messages.

### WebSocket Panel
- Connection bar with status indicator, URL input, Connect/Disconnect.
- Message history with sent/received direction indicators.
- Message composer (`Ctrl+Enter` sends), filter box, and a detail pane for the selected message.

### MCP Panel
- Connection configuration for stdio or HTTP transports.
- Server info display with capability badges.
- Tool/Resource/Prompt browser.
- Tool tester with dynamic argument forms generated from JSON Schema.
- Call history log.

### Runner Panel
- Collection (and folder) selector, iteration/delay configuration, stop-on-error and an optional CSV/JSON data file.
- Progress bar with real-time results.
- Per-request rows showing status, timing, and test results.

## Console Panel

The console panel (bottom, collapsible) shows:

- Log entries with timestamps, level badges (INFO/WARN/ERR/DBG), and source labels (Script/HTTP/Var/Sys).
- Level filter chips, text filter, Copy and an Auto-scroll toggle.
- Clear button to reset the console.

## Theming

Nexus supports three color schemes:

- **Dark** (default) — Dark backgrounds (`#1c1c1e`) with light text.
- **Light** — Light backgrounds (`#f5f5f7`) with dark text and adjusted method colors.
- **System** — Follows the operating system preference automatically.

Theme switching is available via the toolbar button or the command palette.

## Color Coding

HTTP methods are consistently color-coded throughout the UI:

| Method | Color |
|---|---|
| GET | Green (`#22c55e`) |
| POST | Yellow (`#eab308`) |
| PUT | Blue (`#3b82f6`) |
| PATCH | Purple (`#a855f7`) |
| DELETE | Red (`#ef4444`) |
| HEAD | Indigo (`#6366f1`) |
| OPTIONS | Pink (`#ec4899`) |

Status codes use semantic colors:

| Range | Meaning | Color |
|---|---|---|
| 2xx | Success | Green |
| 3xx | Redirect | Blue |
| 4xx | Client Error | Yellow |
| 5xx | Server Error | Red |
