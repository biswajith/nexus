export interface VisualizerConfig {
  template: string;
  data: unknown;
}

export class Visualizer {
  private config: VisualizerConfig | null = null;

  set(template: string, data: unknown): void {
    this.config = { template, data };
  }

  getConfig(): VisualizerConfig | null {
    return this.config;
  }

  clear(): void {
    this.config = null;
  }

  render(): string | null {
    if (!this.config) return null;
    return renderTemplate(this.config.template, this.config.data);
  }

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

function renderTemplate(template: string, context: unknown): string {
  return processTemplate(template, context as Record<string, unknown>);
}

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

function isTruthy(val: unknown): boolean {
  if (Array.isArray(val)) return val.length > 0;
  return !!val;
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
