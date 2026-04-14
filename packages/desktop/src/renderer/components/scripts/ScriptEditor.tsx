import { useEffect, useRef } from 'react';
import { EditorView, keymap, placeholder as cmPlaceholder } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { javascript } from '@codemirror/lang-javascript';
import { oneDark } from '@codemirror/theme-one-dark';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
import {
  autocompletion,
  completionKeymap,
  type CompletionContext,
  type Completion,
} from '@codemirror/autocomplete';
import {
  syntaxHighlighting,
  defaultHighlightStyle,
  bracketMatching,
  foldGutter,
  foldKeymap,
} from '@codemirror/language';
import styles from './ScriptEditor.module.css';

interface ScriptEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

const NX_COMPLETIONS: Completion[] = [
  { label: 'nx.environment.get', type: 'function', info: 'Get environment variable' },
  { label: 'nx.environment.set', type: 'function', info: 'Set environment variable' },
  { label: 'nx.environment.unset', type: 'function', info: 'Remove environment variable' },
  { label: 'nx.collectionVariables.get', type: 'function', info: 'Get collection variable' },
  { label: 'nx.collectionVariables.set', type: 'function', info: 'Set collection variable' },
  { label: 'nx.globals.get', type: 'function', info: 'Get global variable' },
  { label: 'nx.globals.set', type: 'function', info: 'Set global variable' },
  { label: 'nx.variables.get', type: 'function', info: 'Get variable from any scope' },
  { label: 'nx.request.url', type: 'property', info: 'Request URL (mutable)' },
  { label: 'nx.request.method', type: 'property', info: 'HTTP method (mutable)' },
  { label: 'nx.request.headers', type: 'property', info: 'Request headers (mutable)' },
  { label: 'nx.request.body', type: 'property', info: 'Request body (mutable)' },
  { label: 'nx.response.code', type: 'property', info: 'HTTP status code' },
  { label: 'nx.response.status', type: 'property', info: 'Status text' },
  { label: 'nx.response.headers', type: 'property', info: 'Response headers' },
  { label: 'nx.response.responseTime', type: 'property', info: 'Response time in ms' },
  { label: 'nx.response.json()', type: 'function', info: 'Parse response body as JSON' },
  { label: 'nx.response.text()', type: 'function', info: 'Response body as text' },
  { label: 'nx.test', type: 'function', info: 'Define a test assertion', apply: 'nx.test("", () => {\n  \n});' },
  { label: 'nx.expect', type: 'function', info: 'Create an expectation', apply: 'nx.expect()' },
  { label: 'nx.execution.setNextRequest', type: 'function', info: 'Set next request in runner' },
  { label: 'nx.execution.skipRequest', type: 'function', info: 'Skip current request' },
  { label: 'console.log', type: 'function', info: 'Log to console' },
];

function nxAutoComplete(context: CompletionContext) {
  const word = context.matchBefore(/[\w.]*$/);
  if (!word || (word.from === word.to && !context.explicit)) return null;

  return {
    from: word.from,
    options: NX_COMPLETIONS.filter((c) =>
      c.label.toLowerCase().startsWith(word.text.toLowerCase()),
    ),
  };
}

/**
 * CodeMirror editor for Nexus scripts: JavaScript syntax, dark theme, and nx.* autocomplete.
 */
export function ScriptEditor({ value, onChange, placeholder }: ScriptEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!containerRef.current) return;

    const extensions = [
      javascript(),
      oneDark,
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      bracketMatching(),
      foldGutter(),
      history(),
      highlightSelectionMatches(),
      autocompletion({
        override: [nxAutoComplete],
      }),
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        ...foldKeymap,
        ...searchKeymap,
        ...completionKeymap,
      ]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onChangeRef.current(update.state.doc.toString());
        }
      }),
      EditorView.theme({
        '&': { height: '100%', fontSize: '12px' },
        '.cm-scroller': { overflow: 'auto', fontFamily: "'SF Mono', 'Fira Code', 'Consolas', monospace" },
        '.cm-content': { padding: '8px 0' },
      }),
    ];

    if (placeholder) {
      extensions.push(cmPlaceholder(placeholder));
    }

    const state = EditorState.create({
      doc: value,
      extensions,
    });

    const view = new EditorView({
      state,
      parent: containerRef.current,
    });

    viewRef.current = view;

    return () => {
      view.destroy();
    };
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const currentValue = view.state.doc.toString();
    if (currentValue !== value) {
      view.dispatch({
        changes: { from: 0, to: currentValue.length, insert: value },
      });
    }
  }, [value]);

  return <div ref={containerRef} className={styles.editor} />;
}
