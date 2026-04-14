export interface VisualizerConfig {
  template: string;
  data: unknown;
}

/** Holds template and data for rendering Mustache-like HTML snippets and full preview documents. */
export class Visualizer {
  private config: VisualizerConfig | null = null;

  /**
   * Stores the template string and data context for later {@link Visualizer.render} calls.
   * @param template - Source string with `{{}}`, `{{{}}}`, `{{#each}}`, `{{#if}}`, and `{{#unless}}` blocks.
   * @param data - Object (or compatible value) used as the interpolation context.
   * @returns Nothing.
   */
  set(template: string, data: unknown): void {
    this.config = { template, data };
  }

  /**
   * Returns the last configuration passed to {@link Visualizer.set}, or `null` if cleared or never set.
   * @returns The current template and data, or `null` when nothing is configured.
   */
  getConfig(): VisualizerConfig | null {
    return this.config;
  }

  /**
   * Removes any stored template and data so subsequent renders return `null` until {@link Visualizer.set} is called again.
   * @returns Nothing.
   */
  clear(): void {
    this.config = null;
  }

  /**
   * Renders the configured template against the stored data.
   * @returns The rendered HTML fragment, or `null` when no configuration is set.
   */
  render(): string | null {
    if (!this.config) return null;
    return renderTemplate(this.config.template, this.config.data);
  }

  /**
   * Like {@link Visualizer.render}, but wraps the fragment in a full HTML document with built-in dark-theme styles.
   * @returns A complete HTML page string, or `null` when there is nothing to render.
   */
  renderWithWrapper(): string | null {
    const body = this.render();
    if (!body) return null;

    return `<!DOCTYPE html>
<html><head>
<meta charset="UTF-8">
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0d1117; color: #c9d1d9; padding: 16px; font-size: 13px; line-height: 1.5; }
  table { width: 100%; border-collapse: collapse; margin: 8px 0; }
  th { text-align: left; padding: 6px 10px; border-bottom: 2px solid #30363d; color: #8b949e; font-weight: 600; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px; }
  td { padding: 6px 10px; border-bottom: 1px solid #21262d; }
  tr:hover td { background: #161b22; }
  h1, h2, h3 { margin: 12px 0 8px; }
  pre { background: #161b22; padding: 8px 12px; border-radius: 6px; overflow-x: auto; border: 1px solid #30363d; }
  code { font-family: 'SF Mono', 'Fira Code', monospace; font-size: 12px; }
  .badge { display: inline-block; padding: 2px 6px; border-radius: 4px; font-size: 11px; font-weight: 600; }
  .badge-green { background: rgba(63,185,80,.15); color: #3fb950; }
  .badge-red { background: rgba(248,81,73,.15); color: #f85149; }
  .badge-blue { background: rgba(88,166,255,.15); color: #58a6ff; }
  .badge-yellow { background: rgba(210,153,34,.15); color: #d29922; }
</style>
</head><body>${body}</body></html>`;
  }
}

/**
 * Renders a template string using the given context object.
 * @param template - Template source to process.
 * @param context - Root value for variable lookup (coerced to a record for processing).
 * @returns The fully expanded HTML string.
 */
function renderTemplate(template: string, context: unknown): string {
  return processTemplate(template, context as Record<string, unknown>);
}

/**
 * Recursively expands `{{#each}}`, `{{#if}}`, `{{#unless}}`, triple-brace raw output, and escaped `{{}}` placeholders.
 * @param tmpl - Template text still containing any supported constructs.
 * @param ctx - Current scope (may include `@index`, `this`, and nested keys from `each`).
 * @returns The template with all recognized constructs resolved for this scope.
 */
function processTemplate(tmpl: string, ctx: Record<string, unknown>): string {
  let result = tmpl;

  // {{#each items}}...{{/each}}
  result = result.replace(
    /\{\{#each\s+(\w+)\}\}([\s\S]*?)\{\{\/each\}\}/g,
    (_match, key: string, body: string) => {
      const arr = resolve(ctx, key);
      if (!Array.isArray(arr)) return '';
      return arr.map((item, index) => {
        const itemCtx =
          typeof item === 'object' && item !== null
            ? { ...ctx, ...(item as Record<string, unknown>), '@index': index, 'this': item }
            : { ...ctx, '@index': index, 'this': item, '.': item };
        return processTemplate(body, itemCtx);
      }).join('');
    },
  );

  // {{#if condition}}...{{/if}}
  result = result.replace(
    /\{\{#if\s+(\w+)\}\}([\s\S]*?)\{\{\/if\}\}/g,
    (_match, key: string, body: string) => {
      const val = resolve(ctx, key);
      return isTruthy(val) ? processTemplate(body, ctx) : '';
    },
  );

  // {{#unless condition}}...{{/unless}}
  result = result.replace(
    /\{\{#unless\s+(\w+)\}\}([\s\S]*?)\{\{\/unless\}\}/g,
    (_match, key: string, body: string) => {
      const val = resolve(ctx, key);
      return isTruthy(val) ? '' : processTemplate(body, ctx);
    },
  );

  // {{{unescaped}}}
  result = result.replace(/\{\{\{(.+?)\}\}\}/g, (_match, key: string) => {
    const val = resolve(ctx, key.trim());
    return val == null ? '' : String(val);
  });

  // {{escaped}}
  result = result.replace(/\{\{(.+?)\}\}/g, (_match, key: string) => {
    const val = resolve(ctx, key.trim());
    return val == null ? '' : escapeHtml(String(val));
  });

  return result;
}

/**
 * Walks a simple dotted path on a context object; supports `.` and `this` as the current item in `each`.
 * @param ctx - Object (or scope record) to read from.
 * @param path - Property path such as `foo` or `foo.bar`, or `.` / `this`.
 * @returns The value at the path, or `undefined` if missing or traversal fails.
 */
function resolve(ctx: Record<string, unknown>, path: string): unknown {
  if (path === '.' || path === 'this') return ctx['this'] ?? ctx;
  const parts = path.split('.');
  let current: unknown = ctx;
  for (const part of parts) {
    if (current == null || typeof current !== 'object') return undefined;
    current = (current as Record<string, unknown>)[part];
  }
  return current;
}

/**
 * Determines whether `{{#if}}` / `{{#unless}}` should treat a value as true (non-empty arrays count as true).
 * @param val - Value from the template context.
 * @returns `true` if the value should show the `if` branch (or hide `unless`).
 */
function isTruthy(val: unknown): boolean {
  if (Array.isArray(val)) return val.length > 0;
  return !!val;
}

/**
 * Escapes `&`, `<`, `>`, and `"` so interpolated text is safe inside HTML.
 * @param str - Raw string from the template context.
 * @returns The same text with HTML-special characters replaced by entities.
 */
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
