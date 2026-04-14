# Response Viewer

The Response Viewer displays the result of an HTTP request in the lower half of the request panel.

## Status Bar

The status bar appears at the top of the response viewer and shows:

- **Status badge** — Color-coded HTTP status code and text (e.g., `200 OK` in green, `404 Not Found` in yellow, `500 Internal Server Error` in red).
- **Response time** — Total request duration in milliseconds or seconds.
- **Response size** — Body size in bytes, KB, or MB.

### Status Color Coding

| Range | Color | Meaning |
|---|---|---|
| 2xx | Green | Success |
| 3xx | Blue | Redirect |
| 4xx | Yellow | Client Error |
| 5xx | Red | Server Error |

## Response Tabs

### Body

The body tab has three view modes:

**Pretty** — Formatted, syntax-highlighted output. JSON is pretty-printed with 2-space indentation. Large responses (>500 KB) are truncated with a "Show All" button to load the full content.

**Raw** — Unformatted body text displayed as-is.

**Preview** — For HTML responses, renders the content in a sandboxed iframe. Non-HTML content falls back to the Pretty or Raw view.

A **Copy** button copies the full response body text to the clipboard.

### Headers

A sorted alphabetical table of response headers with:

- **Name** — Header name in monospace, highlighted in accent color.
- **Value** — Header value in monospace. Array values are joined with commas.

### Tests

Displays results from test assertions in post-response scripts. Shows:

- **Summary bar** — Count of passed (✓) and failed (✗) tests.
- **Result list** — Each test shows pass/fail icon, test name, error message (for failures), and execution duration.

When no tests are defined, shows a hint to use `nx.test()` in post-response scripts.

### Timing

A visual waterfall breakdown of request timing:

| Phase | Color | Description |
|---|---|---|
| DNS | Purple | DNS resolution time |
| TCP | Blue | TCP connection time |
| TLS | Yellow | TLS handshake time |
| TTFB | Green | Time to first byte (server processing) |
| Download | Indigo | Response download time |

Each phase is shown as a proportionally-sized horizontal bar. Total time is displayed prominently at the top.

### Visualize

Available when a post-response script calls `nx.visualizer.set(template, data)`. Renders custom HTML output in a sandboxed iframe. See [Response Visualization](../scripting/visualization.md).

## Content Type Detection

Nexus automatically detects the response format from the `Content-Type` header:

| Content-Type | Format | Pretty View Behavior |
|---|---|---|
| `application/json` | JSON | Pretty-printed with indentation |
| `text/html` | HTML | Preview available in iframe |
| `text/xml`, `application/xml` | XML | Displayed as text |
| `image/*` | Image | Displayed as image |
| `application/pdf` | PDF | Identified as PDF |
| `application/octet-stream` | Binary | Identified as binary |
| Other | Text | Displayed as plain text |

## Error Responses

When a request fails at the network level (timeout, DNS failure, connection refused), Nexus displays:

- Status code: `0`
- Status text: Error message describing the failure
- Empty body
- Timing data showing what was available before the failure occurred
