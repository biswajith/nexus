import { describe, it, expect, beforeEach } from 'vitest';
import { Visualizer } from './visualizer.js';

describe('Visualizer', () => {
  let viz: Visualizer;

  beforeEach(() => {
    viz = new Visualizer();
  });

  describe('set / getConfig / clear', () => {
    it('starts with null config', () => {
      expect(viz.getConfig()).toBeNull();
    });

    it('stores template and data via set', () => {
      viz.set('<h1>{{title}}</h1>', { title: 'Hello' });
      const cfg = viz.getConfig();
      expect(cfg).not.toBeNull();
      expect(cfg!.template).toBe('<h1>{{title}}</h1>');
      expect(cfg!.data).toEqual({ title: 'Hello' });
    });

    it('overwrites previous config on second set', () => {
      viz.set('first', { a: 1 });
      viz.set('second', { b: 2 });
      expect(viz.getConfig()!.template).toBe('second');
    });

    it('clears config', () => {
      viz.set('tmpl', {});
      viz.clear();
      expect(viz.getConfig()).toBeNull();
    });
  });

  describe('render', () => {
    it('returns null when no config is set', () => {
      expect(viz.render()).toBeNull();
    });

    it('returns null after clear', () => {
      viz.set('hi', {});
      viz.clear();
      expect(viz.render()).toBeNull();
    });

    it('interpolates simple {{}} placeholders with HTML escaping', () => {
      viz.set('<p>{{name}}</p>', { name: 'Alice & Bob' });
      expect(viz.render()).toBe('<p>Alice &amp; Bob</p>');
    });

    it('interpolates {{{  }}} for raw/unescaped output', () => {
      viz.set('<div>{{{html}}}</div>', { html: '<b>bold</b>' });
      expect(viz.render()).toBe('<div><b>bold</b></div>');
    });

    it('escapes < and > in double-brace placeholders', () => {
      viz.set('{{val}}', { val: '<script>alert(1)</script>' });
      expect(viz.render()).toBe('&lt;script&gt;alert(1)&lt;/script&gt;');
    });

    it('escapes quotes in double-brace placeholders', () => {
      viz.set('attr="{{val}}"', { val: 'say "hello"' });
      expect(viz.render()).toBe('attr="say &quot;hello&quot;"');
    });

    it('renders empty string for missing keys', () => {
      viz.set('Hello {{missing}}!', {});
      expect(viz.render()).toBe('Hello !');
    });

    it('renders empty string for null/undefined values', () => {
      viz.set('{{a}}{{b}}', { a: null, b: undefined });
      expect(viz.render()).toBe('');
    });
  });

  describe('render with {{#each}}', () => {
    it('iterates over arrays of objects', () => {
      viz.set('{{#each items}}<li>{{name}}</li>{{/each}}', {
        items: [{ name: 'A' }, { name: 'B' }],
      });
      expect(viz.render()).toBe('<li>A</li><li>B</li>');
    });

    it('provides @index in each block', () => {
      viz.set('{{#each items}}{{@index}}{{/each}}', {
        items: ['a', 'b', 'c'],
      });
      expect(viz.render()).toBe('012');
    });

    it('handles primitive array items via this/dot', () => {
      viz.set('{{#each tags}}{{this}},{{/each}}', {
        tags: ['x', 'y', 'z'],
      });
      expect(viz.render()).toBe('x,y,z,');
    });

    it('renders empty for non-array values', () => {
      viz.set('{{#each notArr}}item{{/each}}', { notArr: 'string' });
      expect(viz.render()).toBe('');
    });

    it('renders empty for missing key', () => {
      viz.set('{{#each missing}}item{{/each}}', {});
      expect(viz.render()).toBe('');
    });

    it('renders empty for empty array', () => {
      viz.set('{{#each items}}item{{/each}}', { items: [] });
      expect(viz.render()).toBe('');
    });
  });

  describe('render with {{#if}} and {{#unless}}', () => {
    it('renders if block when value is truthy', () => {
      viz.set('{{#if show}}visible{{/if}}', { show: true });
      expect(viz.render()).toBe('visible');
    });

    it('skips if block when value is falsy', () => {
      viz.set('{{#if show}}visible{{/if}}', { show: false });
      expect(viz.render()).toBe('');
    });

    it('treats non-empty array as truthy', () => {
      viz.set('{{#if items}}has items{{/if}}', { items: [1] });
      expect(viz.render()).toBe('has items');
    });

    it('treats empty array as falsy', () => {
      viz.set('{{#if items}}has items{{/if}}', { items: [] });
      expect(viz.render()).toBe('');
    });

    it('treats missing key as falsy', () => {
      viz.set('{{#if missing}}visible{{/if}}', {});
      expect(viz.render()).toBe('');
    });

    it('renders unless block when value is falsy', () => {
      viz.set('{{#unless hidden}}shown{{/unless}}', { hidden: false });
      expect(viz.render()).toBe('shown');
    });

    it('skips unless block when value is truthy', () => {
      viz.set('{{#unless hidden}}shown{{/unless}}', { hidden: true });
      expect(viz.render()).toBe('');
    });
  });

  describe('render with dotted paths', () => {
    it('resolves nested properties', () => {
      viz.set('{{user.name}}', { user: { name: 'Eve' } });
      expect(viz.render()).toBe('Eve');
    });

    it('returns empty for broken path', () => {
      viz.set('{{a.b.c}}', { a: { b: null } });
      expect(viz.render()).toBe('');
    });
  });

  describe('renderWithWrapper', () => {
    it('returns null when no config is set', () => {
      expect(viz.renderWithWrapper()).toBeNull();
    });

    it('wraps rendered content in a full HTML document', () => {
      viz.set('<p>Hello</p>', {});
      const html = viz.renderWithWrapper()!;
      expect(html).toContain('<!DOCTYPE html>');
      expect(html).toContain('<body><p>Hello</p></body>');
      expect(html).toContain('</html>');
    });

    it('includes dark theme CSS', () => {
      viz.set('test', {});
      const html = viz.renderWithWrapper()!;
      expect(html).toContain('background: #0d1117');
    });

    it('applies template rendering inside the wrapper', () => {
      viz.set('<span>{{name}}</span>', { name: 'Test' });
      const html = viz.renderWithWrapper()!;
      expect(html).toContain('<span>Test</span>');
    });
  });

  describe('complex template composition', () => {
    it('handles each + if together', () => {
      viz.set(
        '{{#each rows}}{{#if active}}{{name}}|{{/if}}{{/each}}',
        { rows: [{ name: 'A', active: true }, { name: 'B', active: false }, { name: 'C', active: true }] },
      );
      expect(viz.render()).toBe('A|C|');
    });

    it('handles static text with no placeholders', () => {
      viz.set('<h1>Static Content</h1>', {});
      expect(viz.render()).toBe('<h1>Static Content</h1>');
    });
  });
});
