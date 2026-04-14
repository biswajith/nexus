import type { RequestBody, BodyMode } from '@nexus/core';
import { KeyValueEditor } from './KeyValueEditor.js';
import styles from './BodyEditor.module.css';

interface BodyEditorProps {
  body: RequestBody;
  onChange: (body: RequestBody) => void;
}

const BODY_MODES: { value: BodyMode; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'json', label: 'JSON' },
  { value: 'xml', label: 'XML' },
  { value: 'text', label: 'Text' },
  { value: 'html', label: 'HTML' },
  { value: 'form-data', label: 'Form Data' },
  { value: 'x-www-form-urlencoded', label: 'URL Encoded' },
  { value: 'binary', label: 'Binary' },
  { value: 'graphql', label: 'GraphQL' },
];

/** Selects body mode and renders the matching editor (raw text, form fields, GraphQL, etc.). */
export function BodyEditor({ body, onChange }: BodyEditorProps) {
  const setMode = (mode: BodyMode) => {
    onChange({ ...body, mode });
  };

  return (
    <div className={styles.editor}>
      <div className={styles.modeRow}>
        {BODY_MODES.map((m) => (
          <label key={m.value} className={styles.modeLabel}>
            <input
              type="radio"
              name="bodyMode"
              value={m.value}
              checked={body.mode === m.value}
              onChange={() => setMode(m.value)}
              className={styles.modeRadio}
            />
            <span className={body.mode === m.value ? styles.modeActive : ''}>{m.label}</span>
          </label>
        ))}
      </div>

      <div className={styles.modeContent}>
        {body.mode === 'none' && (
          <div className={styles.noneMessage}>
            This request does not have a body.
          </div>
        )}

        {(body.mode === 'json' || body.mode === 'xml' || body.mode === 'text' || body.mode === 'html') && (
          <textarea
            className={styles.rawEditor}
            value={body.raw ?? ''}
            onChange={(e) => onChange({ ...body, raw: e.target.value })}
            placeholder={body.mode === 'json' ? '{\n  "key": "value"\n}' : 'Enter body content...'}
            spellCheck={false}
          />
        )}

        {body.mode === 'x-www-form-urlencoded' && (
          <KeyValueEditor
            pairs={body.urlencoded ?? []}
            onChange={(urlencoded) => onChange({ ...body, urlencoded })}
            namePlaceholder="Field name"
            valuePlaceholder="Value"
          />
        )}

        {body.mode === 'form-data' && (
          <div className={styles.noneMessage}>
            Form data editor with file uploads — coming in Phase 2
          </div>
        )}

        {body.mode === 'binary' && (
          <div className={styles.noneMessage}>
            Binary file upload — coming in Phase 2
          </div>
        )}

        {body.mode === 'graphql' && (
          <div className={styles.graphqlEditor}>
            <div className={styles.graphqlSection}>
              <label className={styles.graphqlLabel}>Query</label>
              <textarea
                className={styles.rawEditor}
                value={body.graphql?.query ?? ''}
                onChange={(e) =>
                  onChange({ ...body, graphql: { ...body.graphql, query: e.target.value, variables: body.graphql?.variables } })
                }
                placeholder="query { ... }"
                spellCheck={false}
              />
            </div>
            <div className={styles.graphqlSection}>
              <label className={styles.graphqlLabel}>Variables</label>
              <textarea
                className={styles.rawEditor}
                value={body.graphql?.variables ?? ''}
                onChange={(e) =>
                  onChange({ ...body, graphql: { query: body.graphql?.query ?? '', variables: e.target.value } })
                }
                placeholder='{ "key": "value" }'
                spellCheck={false}
                style={{ height: '100px' }}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
