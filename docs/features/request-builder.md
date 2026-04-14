# Request Builder

The Request Builder is the primary interface for composing and sending HTTP requests.

## URL Bar

The URL bar sits at the top of the request panel and contains:

- **Method selector** — Dropdown for GET, POST, PUT, PATCH, DELETE, HEAD, and OPTIONS. Each method is color-coded for quick visual identification.
- **URL input** — Monospace text input supporting `{{variable}}` interpolation. Press `Enter` to send.
- **Send button** — Sends the request. Transforms into a Cancel button while a request is in flight.
- **cURL copy button** — Generates and copies a cURL command for the current request.
- **Save dropdown** — Saves the request to a collection.

## Tabs

Five sub-tabs appear below the URL bar:

### Params

Key-value editor for query parameters. Enabled parameters are automatically appended to the URL query string. Each row has:

- **Checkbox** — Enable/disable the parameter without deleting it.
- **Key** — Parameter name.
- **Value** — Parameter value. Supports `{{variable}}` interpolation.
- **Description** — Optional documentation for the parameter.
- **Remove button** — Appears on hover.

### Headers

Same key-value editor as Params, used for HTTP request headers. Active header count is shown as a badge on the tab.

### Body

Select a body mode using radio buttons:

| Mode | Description |
|---|---|
| **None** | No request body. |
| **JSON** | Raw JSON editor with monospace textarea. Sets `Content-Type: application/json`. |
| **XML** | Raw XML editor. Sets `Content-Type: application/xml`. |
| **Text** | Plain text editor. Sets `Content-Type: text/plain`. |
| **HTML** | Raw HTML editor. Sets `Content-Type: text/html`. |
| **Form Data** | Multipart form data with key-value pairs. |
| **URL Encoded** | Application/x-www-form-urlencoded key-value pairs. |
| **Binary** | Binary file upload. |
| **GraphQL** | Split editor with Query and Variables panes. Sends as JSON body with `query` and `variables` fields. |

### Auth

Configure authentication for the request. See [Authentication](./authentication.md) for details on all supported auth types.

### Scripts

Pre-request and post-response script editors with snippet sidebar. See [Scripts](../scripting/scripts.md) for the full scripting guide.

## Sending Requests

When you click **Send** (or press `Cmd+Enter`), Nexus:

1. Resolves all `{{variable}}` placeholders in the URL, headers, params, and body using the active environment and variable scopes.
2. Runs the **pre-request script** (if any). The script can modify the URL, method, headers, and body.
3. Resolves authentication — generates the appropriate `Authorization` header or query parameters.
4. Sends the HTTP request using the `undici` HTTP engine.
5. Receives the response and measures timing (TTFB and download).
6. Runs the **post-response script** (if any), including test assertions and visualizer setup.
7. Displays the response in the Response Viewer.
8. Logs the request to history.

## Cancelling Requests

While a request is in flight, the Send button becomes a Cancel button. Clicking it aborts the request via `AbortController`.

## Saving Requests

### First Save

1. Click **Save ▾** to open the save dropdown.
2. Select a collection from the list.
3. Enter a name for the request.
4. Click **Confirm**.

The request is saved and linked to the collection. The tab is no longer marked as dirty.

### Subsequent Saves

Once a request has an origin (linked to a collection), the button changes to **Save**. Clicking it updates the existing request in the collection.

## cURL Generation

Click the `{ }` button next to Send to copy the current request as a cURL command. The generated cURL includes:

- HTTP method (omitted for GET)
- URL with resolved query parameters
- Headers via `-H` flags
- Authentication (Bearer as `-H Authorization`, Basic as `-u`)
- Body (JSON/XML/text as `-d`, urlencoded as `--data-urlencode`, form-data as `-F`)
- Settings (`-L` for redirects, `-k` for insecure, `--max-time`, `--max-redirs`)

## Request Settings

Settings are part of each request's configuration:

| Setting | Default | Description |
|---|---|---|
| `followRedirects` | `true` | Automatically follow HTTP redirects. |
| `maxRedirects` | `10` | Maximum number of redirects to follow. |
| `timeout` | `30000` | Request timeout in milliseconds. |
| `rejectUnauthorized` | `true` | Verify SSL certificates. Set to `false` for self-signed certs. |

## Multiple Tabs

Open multiple request tabs simultaneously. Each tab maintains its own:

- Request configuration (method, URL, headers, body, auth, scripts)
- Response data
- Loading state
- Dirty state (unsaved changes shown as a blue dot)

Switch tabs by clicking them. Close tabs with the × button or `Cmd+W`. Create new tabs with the + button or `Cmd+N`.
