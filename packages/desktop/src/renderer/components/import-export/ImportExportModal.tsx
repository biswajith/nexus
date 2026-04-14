import { useState, useRef, useCallback, type DragEvent, type ChangeEvent } from 'react';
import { useCollectionStore } from '../../stores/collection-store.js';
import styles from './ImportExportModal.module.css';

type ModalTab = 'import' | 'export';

interface ImportResult {
  collections: Array<{ id: string; name: string; items: Array<NexusTreeNode> }>;
  environments: Array<{ id: string; name: string }>;
  warnings: string[];
}

interface NexusTreeNode {
  name: string;
  method?: string;
  items?: NexusTreeNode[];
}

type ExportFormat = 'nexus' | 'postman-v2.1';

interface Props {
  open: boolean;
  onClose: () => void;
}

/**
 * Modal with Import and Export tabs: import from files or cURL, export collections as Nexus or Postman JSON.
 */
export function ImportExportModal({ open, onClose }: Props) {
  const [tab, setTab] = useState<ModalTab>('import');

  if (!open) return null;

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.headerTitle}>Import / Export</span>
          <button className={styles.closeBtn} onClick={onClose} aria-label="Close">&times;</button>
        </div>
        <div className={styles.body}>
          <div className={styles.tabRow}>
            {(['import', 'export'] as const).map((t) => (
              <button
                key={t}
                className={`${styles.tabBtn} ${tab === t ? styles.tabBtnActive : ''}`}
                onClick={() => setTab(t)}
              >
                {t === 'import' ? 'Import' : 'Export'}
              </button>
            ))}
          </div>
          {tab === 'import' ? <ImportPane onClose={onClose} /> : <ExportPane />}
        </div>
      </div>
    </div>
  );
}

function ImportPane({ onClose }: { onClose: () => void }) {
  const [dragActive, setDragActive] = useState(false);
  const [curlText, setCurlText] = useState('');
  const [detectedFormat, setDetectedFormat] = useState('');
  const [importResult, setImportResult] = useState<ImportResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const fetchCollections = useCollectionStore((s) => s.fetchCollections);

  const handleFileRead = useCallback(async (content: string) => {
    setLoading(true);
    setError('');
    setImportResult(null);
    setSaved(false);

    try {
      const format = await window.nexus.import.detectFormat(content);
      setDetectedFormat(format);
      const result = await window.nexus.import.file(content, format) as ImportResult;
      setImportResult(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to parse file');
    } finally {
      setLoading(false);
    }
  }, []);

  const handleDrop = useCallback((e: DragEvent) => {
    e.preventDefault();
    setDragActive(false);
    const file = e.dataTransfer.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => handleFileRead(reader.result as string);
    reader.readAsText(file);
  }, [handleFileRead]);

  const handleFileSelect = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => handleFileRead(reader.result as string);
    reader.readAsText(file);
  }, [handleFileRead]);

  const handleCurlImport = useCallback(async () => {
    if (!curlText.trim()) return;
    await handleFileRead(curlText.trim());
  }, [curlText, handleFileRead]);

  const handleSave = useCallback(async () => {
    if (!importResult) return;
    setLoading(true);
    try {
      await window.nexus.import.save(importResult.collections, importResult.environments);
      setSaved(true);
      await fetchCollections();
      setTimeout(onClose, 800);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save');
    } finally {
      setLoading(false);
    }
  }, [importResult, fetchCollections, onClose]);

  return (
    <>
      <input
        ref={fileInputRef}
        type="file"
        accept=".json,.yaml,.yml,.postman_collection,.postman_environment"
        style={{ display: 'none' }}
        onChange={handleFileSelect}
      />

      <div
        className={`${styles.dropZone} ${dragActive ? styles.dropZoneActive : ''}`}
        onDragOver={(e) => { e.preventDefault(); setDragActive(true); }}
        onDragLeave={() => setDragActive(false)}
        onDrop={handleDrop}
        onClick={() => fileInputRef.current?.click()}
      >
        <div className={styles.dropIcon}>&#128193;</div>
        <div className={styles.dropText}>Drop a file here or click to browse</div>
        <div className={styles.dropHint}>
          Postman Collections, Environments, OpenAPI 3.x/Swagger 2.0 (JSON or YAML), Nexus exports
        </div>
      </div>

      <hr className={styles.separator} />

      <div className={styles.dropHint}>Or paste a cURL command:</div>
      <textarea
        className={styles.curlInput}
        placeholder="curl -X GET https://api.example.com/users -H 'Authorization: Bearer token'"
        value={curlText}
        onChange={(e) => setCurlText(e.target.value)}
      />
      {curlText.trim() && (
        <div style={{ marginTop: 8 }}>
          <button type="button" className={styles.secondaryBtn} onClick={handleCurlImport}>
            Import cURL
          </button>
        </div>
      )}

      {loading && (
        <div style={{ textAlign: 'center', padding: 16 }}>
          <span className={styles.spinner} />
        </div>
      )}

      {error && (
        <div className={styles.warningsList} style={{ borderLeftColor: '#ef4444' }}>
          <div className={styles.warningTitle} style={{ color: '#ef4444' }}>Error</div>
          <div className={styles.warningItem}>{error}</div>
        </div>
      )}

      {importResult && (
        <>
          <div className={styles.detectedFormat}>
            <span className={styles.formatBadge}>{formatLabel(detectedFormat)}</span>
            <span>
              {importResult.collections.length} collection{importResult.collections.length !== 1 ? 's' : ''}
              {importResult.environments.length > 0 && `, ${importResult.environments.length} environment${importResult.environments.length !== 1 ? 's' : ''}`}
            </span>
          </div>

          <div className={styles.previewSection}>
            <div className={styles.previewTitle}>Preview</div>
            <div className={styles.previewTree}>
              {importResult.collections.map((col) => (
                <CollectionPreview key={col.id} name={col.name} items={col.items} />
              ))}
            </div>
          </div>

          {importResult.warnings.length > 0 && (
            <div className={styles.warningsList}>
              <div className={styles.warningTitle}>
                {importResult.warnings.length} warning{importResult.warnings.length !== 1 ? 's' : ''} during import
              </div>
              {importResult.warnings.slice(0, 10).map((w, i) => (
                <div key={i} className={styles.warningItem}>{w}</div>
              ))}
              {importResult.warnings.length > 10 && (
                <div className={styles.warningItem}>
                  ...and {importResult.warnings.length - 10} more
                </div>
              )}
            </div>
          )}

          <div className={styles.footer}>
            {saved ? (
              <span style={{ color: '#22c55e', fontWeight: 600 }}>Imported successfully!</span>
            ) : (
              <>
                <button type="button" className={styles.secondaryBtn} onClick={onClose}>
                  Cancel
                </button>
                <button type="button" className={styles.primaryBtn} onClick={handleSave} disabled={loading}>
                  {loading ? 'Saving...' : 'Import'}
                </button>
              </>
            )}
          </div>
        </>
      )}
    </>
  );
}

function ExportPane() {
  const collections = useCollectionStore((s) => s.collections);
  const loadedCollections = useCollectionStore((s) => s.loadedCollections);
  const loadCollection = useCollectionStore((s) => s.loadCollection);
  const [exporting, setExporting] = useState<string | null>(null);

  const handleExport = useCallback(async (collectionMeta: { id: string; name: string; path: string }, format: ExportFormat) => {
    setExporting(collectionMeta.id);

    try {
      let collection = loadedCollections.get(collectionMeta.path);
      if (!collection) {
        await loadCollection(collectionMeta.path);
        collection = useCollectionStore.getState().loadedCollections.get(collectionMeta.path);
      }
      if (!collection) throw new Error('Could not load collection');

      const pmFormat = format === 'postman-v2.1' ? 'postman-v2.1' : 'nexus';
      const data = await window.nexus.export.collection(collection, pmFormat);
      const filename = `${collectionMeta.name.replace(/[^a-zA-Z0-9-_]/g, '_')}.${format === 'postman-v2.1' ? 'postman_collection' : 'nexus'}.json`;

      const blob = new Blob([data as string], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Export failed:', err);
    } finally {
      setExporting(null);
    }
  }, [loadedCollections, loadCollection]);

  if (collections.length === 0) {
    return (
      <div style={{ textAlign: 'center', padding: 20, color: 'var(--spectrum-alias-text-color-secondary, #888)' }}>
        No collections to export. Import or create a collection first.
      </div>
    );
  }

  return (
    <div className={styles.exportSection}>
      {collections.map((col) => (
        <div key={col.id} className={styles.exportItem}>
          <span className={styles.exportItemName}>{col.name}</span>
          <div className={styles.exportActions}>
            {exporting === col.id ? (
              <span className={styles.spinner} />
            ) : (
              <>
                <button
                  type="button"
                  className={styles.secondaryBtn}
                  onClick={() => handleExport(col, 'nexus')}
                >
                  Nexus JSON
                </button>
                <button
                  type="button"
                  className={styles.secondaryBtn}
                  onClick={() => handleExport(col, 'postman-v2.1')}
                >
                  Postman v2.1
                </button>
              </>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

function CollectionPreview({ name, items, depth = 0 }: { name: string; items: NexusTreeNode[]; depth?: number }) {
  return (
    <>
      <div className={styles.treeFolder} style={{ paddingLeft: depth * 16 }}>
        &#128194; {name}
      </div>
      {items.slice(0, 50).map((item, i) =>
        item.items ? (
          <CollectionPreview key={i} name={item.name} items={item.items} depth={depth + 1} />
        ) : (
          <div key={i} className={styles.treeRequest} style={{ paddingLeft: (depth + 1) * 16 }}>
            <span className={`${styles.treeMethod} ${methodColor(item.method)}`}>
              {item.method ?? 'GET'}
            </span>
            {item.name}
          </div>
        ),
      )}
      {items.length > 50 && (
        <div className={styles.treeRequest} style={{ paddingLeft: (depth + 1) * 16, opacity: 0.6 }}>
          ...and {items.length - 50} more items
        </div>
      )}
    </>
  );
}

function methodColor(method?: string): string {
  switch (method?.toUpperCase()) {
    case 'GET': return styles.methodGet ?? '';
    case 'POST': return styles.methodPost ?? '';
    case 'PUT': return styles.methodPut ?? '';
    case 'PATCH': return styles.methodPatch ?? '';
    case 'DELETE': return styles.methodDelete ?? '';
    default: return '';
  }
}

function formatLabel(format: string): string {
  switch (format) {
    case 'postman-collection-v2.1': return 'Postman Collection';
    case 'postman-environment': return 'Postman Environment';
    case 'openapi-3': return 'OpenAPI 3.x';
    case 'swagger-2': return 'Swagger 2.0';
    case 'curl': return 'cURL';
    case 'nexus': return 'Nexus';
    default: return format;
  }
}
