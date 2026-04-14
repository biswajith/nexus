import type { KeyValuePair } from '@nexus/core';
import styles from './KeyValueEditor.module.css';

interface KeyValueEditorProps {
  pairs: KeyValuePair[];
  onChange: (pairs: KeyValuePair[]) => void;
  namePlaceholder?: string;
  valuePlaceholder?: string;
}

/** Table-style editor for key/value pairs with enable toggles, descriptions, and add/remove rows. */
export function KeyValueEditor({
  pairs,
  onChange,
  namePlaceholder = 'Key',
  valuePlaceholder = 'Value',
}: KeyValueEditorProps) {
  const updatePair = (index: number, updates: Partial<KeyValuePair>) => {
    const updated = pairs.map((p, i) => (i === index ? { ...p, ...updates } : p));
    onChange(updated);
  };

  const addPair = () => {
    onChange([...pairs, { key: '', value: '', enabled: true }]);
  };

  const removePair = (index: number) => {
    onChange(pairs.filter((_, i) => i !== index));
  };

  const displayPairs = pairs.length === 0
    ? [{ key: '', value: '', enabled: true }]
    : pairs;

  return (
    <div className={styles.editor}>
      <div className={styles.header}>
        <span className={styles.checkCol}></span>
        <span className={styles.keyCol}>Key</span>
        <span className={styles.valueCol}>Value</span>
        <span className={styles.descCol}>Description</span>
        <span className={styles.actionCol}></span>
      </div>
      {displayPairs.map((pair, index) => (
        <div key={index} className={styles.row}>
          <input
            type="checkbox"
            className={styles.checkbox}
            checked={pair.enabled}
            onChange={(e) => {
              if (pairs.length === 0) {
                onChange([{ ...pair, enabled: e.target.checked }]);
              } else {
                updatePair(index, { enabled: e.target.checked });
              }
            }}
          />
          <input
            className={styles.input}
            type="text"
            placeholder={namePlaceholder}
            value={pair.key}
            onChange={(e) => {
              if (pairs.length === 0) {
                onChange([{ ...pair, key: e.target.value }]);
              } else {
                updatePair(index, { key: e.target.value });
              }
            }}
            spellCheck={false}
          />
          <input
            className={styles.input}
            type="text"
            placeholder={valuePlaceholder}
            value={pair.value}
            onChange={(e) => {
              if (pairs.length === 0) {
                onChange([{ ...pair, value: e.target.value }]);
              } else {
                updatePair(index, { value: e.target.value });
              }
            }}
            spellCheck={false}
          />
          <input
            className={styles.input}
            type="text"
            placeholder="Description"
            value={pair.description ?? ''}
            onChange={(e) => {
              if (pairs.length === 0) {
                onChange([{ ...pair, description: e.target.value }]);
              } else {
                updatePair(index, { description: e.target.value });
              }
            }}
            spellCheck={false}
          />
          <button
            className={styles.removeBtn}
            onClick={() => {
              if (pairs.length > 0) removePair(index);
            }}
            aria-label="Remove row"
          >
            ×
          </button>
        </div>
      ))}
      <div className={styles.addRow}>
        <button className={styles.addBtn} onClick={addPair}>+ Add</button>
      </div>
    </div>
  );
}
