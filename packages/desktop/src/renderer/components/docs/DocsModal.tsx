import { useState, useCallback } from 'react';
import { useCollectionStore } from '../../stores/collection-store.js';
import styles from './DocsModal.module.css';

type DocFormat = 'markdown' | 'html';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function DocsModal({ open, onClose }: Props) {
  const [selectedCollection, setSelectedCollection] = useState('');
  const [format, setFormat] = useState<DocFormat>('html');
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);

  const collections = useCollectionStore((s) => s.collections);
  const loadedCollections = useCollectionStore((s) => s.loadedCollections);
  const loadCollection = useCollectionStore((s) => s.loadCollection);

  const handleGenerate = useCallback(async () => {
    if (!selectedCollection) return;
    setLoading(true);
    setContent('');

    try {
      let collection = loadedCollections.get(selectedCollection);
      if (!collection) {
        await loadCollection(selectedCollection);
        collection = useCollectionStore.getState().loadedCollections.get(selectedCollection);
      }
      if (!collection) return;

      const result = format === 'html'
        ? await window.nexus.docs.html(collection)
        : await window.nexus.docs.markdown(collection);
      setContent(result as string);
    } finally {
      setLoading(false);
    }
  }, [selectedCollection, format, loadedCollections, loadCollection]);

  const handleDownload = useCallback(() => {
    if (!content) return;
    const ext = format === 'html' ? 'html' : 'md';
    const mime = format === 'html' ? 'text/html' : 'text/markdown';
    const name = collections.find((c) => c.path === selectedCollection)?.name ?? 'docs';
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${name.replace(/[^a-zA-Z0-9-_]/g, '_')}-docs.${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  }, [content, format, selectedCollection, collections]);

  const handleCopy = useCallback(async () => {
    if (!content) return;
    await navigator.clipboard.writeText(content);
  }, [content]);

  if (!open) return null;

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.headerTitle}>Generate API Documentation</span>
          <button className={styles.closeBtn} onClick={onClose} aria-label="Close">&times;</button>
        </div>
        <div className={styles.body}>
          <div className={styles.configRow}>
            <span className={styles.configLabel}>Collection</span>
            <select
              className={styles.select}
              value={selectedCollection}
              onChange={(e) => { setSelectedCollection(e.target.value); setContent(''); }}
            >
              <option value="">Select collection...</option>
              {collections.map((c) => (
                <option key={c.id} value={c.path}>{c.name}</option>
              ))}
            </select>
            <span className={styles.configLabel}>Format</span>
            <select
              className={styles.select}
              style={{ minWidth: 120 }}
              value={format}
              onChange={(e) => { setFormat(e.target.value as DocFormat); setContent(''); }}
            >
              <option value="html">HTML</option>
              <option value="markdown">Markdown</option>
            </select>
          </div>

          <div className={styles.actions}>
            <button
              type="button"
              className={styles.primaryBtn}
              onClick={handleGenerate}
              disabled={!selectedCollection || loading}
            >
              {loading ? 'Generating...' : 'Generate'}
            </button>
            {content && (
              <>
                <button type="button" className={styles.secondaryBtn} onClick={handleDownload}>
                  Download
                </button>
                <button type="button" className={styles.secondaryBtn} onClick={handleCopy}>
                  Copy
                </button>
              </>
            )}
          </div>

          {content ? (
            <div className={styles.preview}>
              {format === 'html' ? (
                <iframe
                  className={styles.previewHtml}
                  srcDoc={content}
                  title="API Documentation Preview"
                  sandbox="allow-same-origin"
                />
              ) : (
                <div className={styles.previewMd}>{content}</div>
              )}
            </div>
          ) : (
            <div className={styles.empty}>
              Select a collection and click "Generate" to create documentation
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
