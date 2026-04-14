import { useState, useMemo } from 'react';
import { formatJson, detectContentType } from '../../lib/format-utils.js';
import styles from './ResponseBody.module.css';

interface ResponseBodyProps {
  response: {
    headers: Record<string, string | string[]>;
    bodyText: string;
    bodyJson?: unknown;
  };
}

type ViewMode = 'pretty' | 'raw' | 'preview';

const LARGE_BODY_THRESHOLD = 500_000;

/**
 * Displays the response body with pretty, raw, or preview modes, optional truncation for large payloads, and copy.
 */
export function ResponseBody({ response }: ResponseBodyProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('pretty');
  const [showFull, setShowFull] = useState(false);
  const contentType = detectContentType(response.headers as Record<string, string | string[]>);

  const isLarge = response.bodyText.length > LARGE_BODY_THRESHOLD;
  const displayText = isLarge && !showFull
    ? response.bodyText.slice(0, LARGE_BODY_THRESHOLD)
    : response.bodyText;

  const prettyBody = useMemo(() => {
    if (contentType !== 'json') return null;
    return formatJson(displayText);
  }, [contentType, displayText]);

  return (
    <div className={styles.body}>
      <div className={styles.viewModeRow}>
        {(['pretty', 'raw', 'preview'] as const).map((mode) => (
          <button
            key={mode}
            className={`${styles.viewBtn} ${viewMode === mode ? styles.activeView : ''}`}
            onClick={() => setViewMode(mode)}
          >
            {mode.charAt(0).toUpperCase() + mode.slice(1)}
          </button>
        ))}
        <button
          className={styles.copyBtn}
          onClick={() => navigator.clipboard.writeText(response.bodyText)}
          aria-label="Copy response body"
        >
          Copy
        </button>
      </div>

      <div className={styles.content}>
        {viewMode === 'pretty' && (
          <pre className={styles.pre}>
            <code>{prettyBody?.formatted ?? displayText}</code>
          </pre>
        )}
        {viewMode === 'raw' && (
          <pre className={styles.pre}>
            <code>{displayText}</code>
          </pre>
        )}
        {viewMode === 'preview' && contentType === 'html' && (
          <iframe
            className={styles.preview}
            srcDoc={response.bodyText}
            sandbox=""
            title="Response preview"
          />
        )}
        {viewMode === 'preview' && contentType !== 'html' && (
          <pre className={styles.pre}>
            <code>{prettyBody?.formatted ?? displayText}</code>
          </pre>
        )}
        {isLarge && !showFull && viewMode !== 'preview' && (
          <div className={styles.truncatedNotice}>
            <span>Response truncated ({(response.bodyText.length / 1024).toFixed(0)} KB)</span>
            <button className={styles.showAllBtn} onClick={() => setShowFull(true)}>
              Show All
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
