import { describe, it, expect } from 'vitest';
import { VariableStore } from './variable-store.js';
import type { Variable } from '../types/index.js';

function makeVar(key: string, value: string, enabled = true): Variable {
  return { key, value, type: 'string', enabled };
}

describe('VariableStore — variable resolution (issue #2)', () => {
  it('resolves environment variables in {{}} tokens', () => {
    const store = new VariableStore({
      environment: [makeVar('host', 'api.example.com')],
    });

    expect(store.resolve('https://{{host}}/users')).toBe('https://api.example.com/users');
  });

  it('resolves collection variables in {{}} tokens', () => {
    const store = new VariableStore({
      collection: [makeVar('version', 'v2')],
    });

    expect(store.resolve('/api/{{version}}/items')).toBe('/api/v2/items');
  });

  it('resolves global variables in {{}} tokens', () => {
    const store = new VariableStore({
      global: [makeVar('apiKey', 'abc123')],
    });

    expect(store.resolve('?key={{apiKey}}')).toBe('?key=abc123');
  });

  it('respects scope precedence: local > environment > collection > global', () => {
    const store = new VariableStore({
      global: [makeVar('x', 'global')],
      collection: [makeVar('x', 'collection')],
      environment: [makeVar('x', 'environment')],
    });

    expect(store.resolve('{{x}}')).toBe('environment');

    store.setLocalScope({ x: 'local' });
    expect(store.resolve('{{x}}')).toBe('local');
  });

  it('resolves multiple variables in a single URL', () => {
    const store = new VariableStore({
      environment: [
        makeVar('protocol', 'https'),
        makeVar('host', 'api.example.com'),
        makeVar('port', '8443'),
      ],
      collection: [makeVar('basePath', 'api/v1')],
    });

    const url = '{{protocol}}://{{host}}:{{port}}/{{basePath}}/users';
    expect(store.resolve(url)).toBe('https://api.example.com:8443/api/v1/users');
  });

  it('leaves unresolved tokens as-is', () => {
    const store = new VariableStore({});

    expect(store.resolve('https://{{unknown}}/path')).toBe('https://{{unknown}}/path');
  });

  it('ignores disabled variables', () => {
    const store = new VariableStore({
      environment: [makeVar('host', 'disabled.com', false)],
    });

    expect(store.resolve('https://{{host}}/path')).toBe('https://{{host}}/path');
  });

  it('resolves variables in headers', () => {
    const store = new VariableStore({
      environment: [makeVar('token', 'bearer-xyz')],
    });

    const resolved = store.resolveHeaders({
      Authorization: '{{token}}',
      'Content-Type': 'application/json',
    });

    expect(resolved['Authorization']).toBe('bearer-xyz');
    expect(resolved['Content-Type']).toBe('application/json');
  });

  it('resolves dynamic $guid and $timestamp variables', () => {
    const store = new VariableStore({});

    const guid = store.resolve('{{$guid}}');
    expect(guid).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);

    const ts = store.resolve('{{$timestamp}}');
    expect(Number(ts)).toBeGreaterThan(0);
  });

  it('resolves variables with whitespace in token names', () => {
    const store = new VariableStore({
      environment: [makeVar('host', 'example.com')],
    });

    expect(store.resolve('{{ host }}')).toBe('example.com');
    expect(store.resolve('{{  host  }}')).toBe('example.com');
  });

  it('handles empty string as input', () => {
    const store = new VariableStore({});
    expect(store.resolve('')).toBe('');
  });

  it('handles input with no tokens', () => {
    const store = new VariableStore({
      environment: [makeVar('host', 'example.com')],
    });

    expect(store.resolve('https://plain-url.com/path')).toBe('https://plain-url.com/path');
  });

  it('clears local scope properly', () => {
    const store = new VariableStore({
      environment: [makeVar('x', 'env-val')],
    });

    store.setLocalScope({ x: 'local-val' });
    expect(store.resolve('{{x}}')).toBe('local-val');

    store.clearLocalScope();
    expect(store.resolve('{{x}}')).toBe('env-val');
  });
});
