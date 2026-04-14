import type { NexusCollection, NexusRequest, NexusFolder } from '../types/index.js';
import { isFolder } from '../types/index.js';
import { generateCurl } from '../codegen/curl-generator.js';

interface DocGroup {
  name: string;
  description?: string;
  endpoints: DocEndpoint[];
}

interface DocEndpoint {
  name: string;
  method: string;
  url: string;
  description?: string;
  headers: Array<{ key: string; value: string; description?: string }>;
  params: Array<{ key: string; value: string; description?: string }>;
  bodyMode: string;
  bodyExample?: string;
  curl: string;
}

/** Builds Markdown and HTML API documentation from a Nexus collection. */
export class DocGenerator {
  /**
   * Renders the collection as a Markdown document with a table of contents, grouped endpoints, and cURL examples.
   * @param collection - The Nexus collection to document.
   * @returns The full Markdown string.
   */
  generateMarkdown(collection: NexusCollection): string {
    const groups = this.extractGroups(collection);
    const lines: string[] = [];

    lines.push(`# ${collection.name}`);
    if (collection.description) lines.push('', collection.description);

    const baseUrl = collection.variables.find((v) => v.key === 'base_url')?.value;
    if (baseUrl) lines.push('', `**Base URL:** \`${baseUrl}\``);

    lines.push('', '---', '', '## Table of Contents', '');
    for (const group of groups) {
      lines.push(`- [${group.name}](#${slugify(group.name)})`);
      for (const ep of group.endpoints) {
        lines.push(`  - [${ep.method} ${ep.name}](#${slugify(`${ep.method}-${ep.name}`)})`);
      }
    }

    for (const group of groups) {
      lines.push('', '---', '', `## ${group.name}`);
      if (group.description) lines.push('', group.description);

      for (const ep of group.endpoints) {
        lines.push('', `### ${ep.method} ${ep.name}`, '');
        lines.push(`\`${ep.method} ${ep.url}\``);
        if (ep.description) lines.push('', ep.description);

        if (ep.headers.length > 0) {
          lines.push('', '**Headers:**', '', '| Header | Value | Description |', '|--------|-------|-------------|');
          for (const h of ep.headers) {
            lines.push(`| \`${h.key}\` | \`${h.value}\` | ${h.description ?? ''} |`);
          }
        }

        if (ep.params.length > 0) {
          lines.push('', '**Query Parameters:**', '', '| Parameter | Value | Description |', '|-----------|-------|-------------|');
          for (const p of ep.params) {
            lines.push(`| \`${p.key}\` | \`${p.value}\` | ${p.description ?? ''} |`);
          }
        }

        if (ep.bodyMode !== 'none' && ep.bodyExample) {
          lines.push('', `**Body** (\`${ep.bodyMode}\`):`, '', '```json', ep.bodyExample, '```');
        }

        lines.push('', '**cURL:**', '', '```bash', ep.curl, '```');
      }
    }

    lines.push('', '---', '', `*Generated from "${collection.name}" by Nexus*`);
    return lines.join('\n');
  }

  /**
   * Renders the collection as a single-page styled HTML document with navigation and endpoint details.
   * @param collection - The Nexus collection to document.
   * @returns The full HTML document as a string.
   */
  generateHtml(collection: NexusCollection): string {
    const groups = this.extractGroups(collection);

    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${esc(collection.name)} — API Documentation</title>
  <style>
    :root { --bg: #0d1117; --fg: #c9d1d9; --muted: #8b949e; --border: #30363d; --accent: #58a6ff; --green: #3fb950; --yellow: #d29922; --red: #f85149; --blue: #58a6ff; --purple: #bc8cff; --surface: #161b22; }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif; background: var(--bg); color: var(--fg); line-height: 1.6; }
    .container { max-width: 960px; margin: 0 auto; padding: 40px 24px; }
    h1 { font-size: 28px; margin-bottom: 8px; }
    h2 { font-size: 22px; margin: 40px 0 12px; padding-bottom: 8px; border-bottom: 1px solid var(--border); }
    h3 { font-size: 16px; margin: 24px 0 8px; }
    p { margin: 8px 0; color: var(--muted); }
    .base-url { background: var(--surface); padding: 8px 14px; border-radius: 6px; font-family: 'SF Mono', monospace; font-size: 13px; border: 1px solid var(--border); display: inline-block; margin: 8px 0; }
    .endpoint { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 16px 20px; margin: 16px 0; }
    .method-badge { display: inline-block; padding: 2px 8px; border-radius: 4px; font-weight: 700; font-size: 12px; margin-right: 8px; }
    .method-GET { background: rgba(63,185,80,.15); color: var(--green); }
    .method-POST { background: rgba(210,153,34,.15); color: var(--yellow); }
    .method-PUT { background: rgba(88,166,255,.15); color: var(--blue); }
    .method-PATCH { background: rgba(188,140,255,.15); color: var(--purple); }
    .method-DELETE { background: rgba(248,81,73,.15); color: var(--red); }
    .url { font-family: 'SF Mono', monospace; font-size: 13px; color: var(--fg); }
    table { width: 100%; border-collapse: collapse; margin: 12px 0; font-size: 13px; }
    th { text-align: left; padding: 6px 10px; border-bottom: 1px solid var(--border); color: var(--muted); font-weight: 600; }
    td { padding: 6px 10px; border-bottom: 1px solid var(--border); }
    td code { background: var(--surface); padding: 1px 5px; border-radius: 3px; font-size: 12px; }
    pre { background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 12px 16px; overflow-x: auto; font-size: 12px; line-height: 1.5; margin: 8px 0; }
    code { font-family: 'SF Mono', 'Fira Code', monospace; }
    .label { font-size: 12px; font-weight: 600; color: var(--muted); text-transform: uppercase; letter-spacing: 0.5px; margin: 12px 0 4px; }
    .footer { margin-top: 48px; padding-top: 16px; border-top: 1px solid var(--border); font-size: 12px; color: var(--muted); }
    .toc { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 16px 20px; margin: 20px 0; }
    .toc ul { list-style: none; padding-left: 16px; }
    .toc a { color: var(--accent); text-decoration: none; }
    .toc a:hover { text-decoration: underline; }
  </style>
</head>
<body>
  <div class="container">
    <h1>${esc(collection.name)}</h1>
    ${collection.description ? `<p>${esc(collection.description)}</p>` : ''}
    ${collection.variables.find((v) => v.key === 'base_url') ? `<div class="base-url">${esc(collection.variables.find((v) => v.key === 'base_url')!.value)}</div>` : ''}

    <div class="toc">
      <strong>Endpoints</strong>
      <ul>
${groups.map((g) => `        <li><strong>${esc(g.name)}</strong><ul>${g.endpoints.map((ep) => `<li><a href="#${slugify(`${ep.method}-${ep.name}`)}"><span class="method-badge method-${ep.method}">${ep.method}</span>${esc(ep.name)}</a></li>`).join('')}</ul></li>`).join('\n')}
      </ul>
    </div>

${groups.map((g) => `
    <h2>${esc(g.name)}</h2>
    ${g.description ? `<p>${esc(g.description)}</p>` : ''}
${g.endpoints.map((ep) => `
    <div class="endpoint" id="${slugify(`${ep.method}-${ep.name}`)}">
      <h3><span class="method-badge method-${ep.method}">${ep.method}</span><span class="url">${esc(ep.url)}</span></h3>
      ${ep.description ? `<p>${esc(ep.description)}</p>` : ''}
      ${ep.headers.length > 0 ? `<div class="label">Headers</div><table><tr><th>Header</th><th>Value</th><th>Description</th></tr>${ep.headers.map((h) => `<tr><td><code>${esc(h.key)}</code></td><td><code>${esc(h.value)}</code></td><td>${esc(h.description ?? '')}</td></tr>`).join('')}</table>` : ''}
      ${ep.params.length > 0 ? `<div class="label">Query Parameters</div><table><tr><th>Parameter</th><th>Value</th><th>Description</th></tr>${ep.params.map((p) => `<tr><td><code>${esc(p.key)}</code></td><td><code>${esc(p.value)}</code></td><td>${esc(p.description ?? '')}</td></tr>`).join('')}</table>` : ''}
      ${ep.bodyMode !== 'none' && ep.bodyExample ? `<div class="label">Body (${esc(ep.bodyMode)})</div><pre><code>${esc(ep.bodyExample)}</code></pre>` : ''}
      <div class="label">cURL</div><pre><code>${esc(ep.curl)}</code></pre>
    </div>`).join('\n')}`).join('\n')}

    <div class="footer">Generated from "${esc(collection.name)}" by Nexus</div>
  </div>
</body>
</html>`;
  }

  /**
   * Partitions collection items into folder-backed groups and a final group for ungrouped top-level requests.
   * @param collection - The collection whose `items` are grouped.
   * @returns Groups each containing a name, optional description, and endpoint list.
   */
  private extractGroups(collection: NexusCollection): DocGroup[] {
    const groups: DocGroup[] = [];
    const ungrouped: DocEndpoint[] = [];

    for (const item of collection.items) {
      if (isFolder(item)) {
        groups.push({
          name: item.name,
          description: item.description,
          endpoints: this.flattenEndpoints(item.items),
        });
      } else {
        ungrouped.push(this.toEndpoint(item));
      }
    }

    if (ungrouped.length > 0) {
      groups.push({ name: 'Requests', endpoints: ungrouped });
    }

    return groups;
  }

  /**
   * Walks a nested folder/request tree and collects every leaf request as a flat endpoint list.
   * @param items - Folder and request nodes to traverse.
   * @returns All endpoints under `items`, in traversal order.
   */
  private flattenEndpoints(items: (NexusRequest | NexusFolder)[]): DocEndpoint[] {
    const result: DocEndpoint[] = [];
    for (const item of items) {
      if (isFolder(item)) {
        result.push(...this.flattenEndpoints(item.items));
      } else {
        result.push(this.toEndpoint(item));
      }
    }
    return result;
  }

  /**
   * Converts a `NexusRequest` into the shape used for Markdown/HTML output (filters enabled fields, builds cURL).
   * @param req - The request to normalize for documentation.
   * @returns Endpoint metadata including URL, headers, query params, body snippet, and generated cURL.
   */
  private toEndpoint(req: NexusRequest): DocEndpoint {
    return {
      name: req.name,
      method: req.method,
      url: req.url,
      description: undefined,
      headers: req.headers.filter((h) => h.enabled).map((h) => ({
        key: h.key, value: h.value, description: h.description,
      })),
      params: req.params.filter((p) => p.enabled).map((p) => ({
        key: p.key, value: p.value, description: p.description,
      })),
      bodyMode: req.body.mode,
      bodyExample: req.body.raw ?? (req.body.graphql ? req.body.graphql.query : undefined),
      curl: generateCurl(req),
    };
  }
}

/**
 * Lowercases `text` and replaces non-alphanumeric runs with hyphens for stable anchor and `id` values.
 * @param text - Arbitrary label text to turn into a fragment slug.
 * @returns A hyphenated slug with leading/trailing hyphens removed.
 */
function slugify(text: string): string {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '');
}

/**
 * Escapes `&`, `<`, `>`, and `"` so strings are safe to embed in HTML text and attributes.
 * @param str - Raw user or collection content to escape.
 * @returns The HTML-escaped string.
 */
function esc(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
