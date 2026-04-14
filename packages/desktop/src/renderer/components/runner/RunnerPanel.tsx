import { useState, useCallback } from 'react';
import { useRunnerStore } from '../../stores/runner-store.js';
import { useCollectionStore } from '../../stores/collection-store.js';
import styles from './RunnerPanel.module.css';

export function RunnerPanel() {
  const [iterations, setIterations] = useState(1);
  const [delayMs, setDelayMs] = useState(0);
  const [stopOnError, setStopOnError] = useState(false);
  const [selectedCollection, setSelectedCollection] = useState('');

  const status = useRunnerStore((s) => s.status);
  const results = useRunnerStore((s) => s.results);
  const summary = useRunnerStore((s) => s.summary);
  const currentIteration = useRunnerStore((s) => s.currentIteration);
  const totalIterations = useRunnerStore((s) => s.totalIterations);
  const startRun = useRunnerStore((s) => s.startRun);
  const cancelRun = useRunnerStore((s) => s.cancelRun);
  const reset = useRunnerStore((s) => s.reset);

  const collections = useCollectionStore((s) => s.collections);
  const loadedCollections = useCollectionStore((s) => s.loadedCollections);
  const loadCollection = useCollectionStore((s) => s.loadCollection);

  const handleStart = useCallback(async () => {
    if (!selectedCollection) return;

    let collection = loadedCollections.get(selectedCollection);
    if (!collection) {
      await loadCollection(selectedCollection);
      collection = useCollectionStore.getState().loadedCollections.get(selectedCollection);
    }
    if (!collection) return;

    await startRun({
      collection,
      iterations,
      delayMs,
      stopOnError,
      persistVariables: false,
    });
  }, [selectedCollection, iterations, delayMs, stopOnError, loadedCollections, loadCollection, startRun]);

  const progress = totalIterations > 0
    ? (results.length / (totalIterations * (summary?.totalRequests ?? (results.length || 1)))) * 100
    : 0;

  return (
    <div className={styles.panel}>
      <div className={styles.config}>
        <div className={styles.configRow}>
          <span className={styles.configLabel}>Collection</span>
          <select
            className={styles.configInput}
            style={{ width: 200 }}
            value={selectedCollection}
            onChange={(e) => setSelectedCollection(e.target.value)}
            disabled={status === 'running'}
          >
            <option value="">Select collection...</option>
            {collections.map((c) => (
              <option key={c.id} value={c.path}>{c.name}</option>
            ))}
          </select>
        </div>
        <div className={styles.configRow}>
          <span className={styles.configLabel}>Iterations</span>
          <input
            type="number" min={1} max={1000}
            className={styles.configInput}
            value={iterations}
            onChange={(e) => setIterations(Math.max(1, parseInt(e.target.value) || 1))}
            disabled={status === 'running'}
          />
          <span className={styles.configLabel}>Delay (ms)</span>
          <input
            type="number" min={0} max={60000}
            className={styles.configInput}
            value={delayMs}
            onChange={(e) => setDelayMs(Math.max(0, parseInt(e.target.value) || 0))}
            disabled={status === 'running'}
          />
          <label style={{ fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}>
            <input
              type="checkbox"
              checked={stopOnError}
              onChange={(e) => setStopOnError(e.target.checked)}
              disabled={status === 'running'}
            />
            Stop on error
          </label>
        </div>
        <div className={styles.actions}>
          {status === 'running' ? (
            <button type="button" className={styles.dangerBtn} onClick={cancelRun}>
              Cancel
            </button>
          ) : (
            <>
              <button
                type="button"
                className={styles.primaryBtn}
                onClick={handleStart}
                disabled={!selectedCollection}
              >
                Run Collection
              </button>
              {status !== 'idle' && (
                <button type="button" className={styles.secondaryBtn} onClick={reset}>
                  Reset
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {status === 'running' && (
        <div className={styles.progress}>
          <span>Iteration {currentIteration}/{totalIterations}</span>
          <div className={styles.progressBar}>
            <div className={styles.progressFill} style={{ width: `${Math.min(100, progress)}%` }} />
          </div>
          <span>{results.length} requests</span>
        </div>
      )}

      {results.length > 0 ? (
        <>
          <div className={styles.results}>
            {results.map((r, i) => {
              const passed = r.testResults.filter((t) => t.passed).length;
              const failed = r.testResults.filter((t) => !t.passed).length;
              return (
                <div key={i} className={styles.resultItem}>
                  <span className={`${styles.resultMethod} ${methodColor(r.method)}`}>{r.method}</span>
                  <span className={styles.resultName}>{r.requestName}</span>
                  {r.error ? (
                    <span className={`${styles.resultStatus} ${styles.statusError}`}>Error</span>
                  ) : (
                    <span className={`${styles.resultStatus} ${r.status < 400 ? styles.statusSuccess : styles.statusError}`}>
                      {r.status}
                    </span>
                  )}
                  <span className={styles.resultTime}>{r.responseTime}ms</span>
                  <div className={styles.resultTests}>
                    {passed > 0 && <span className={`${styles.testBadge} ${styles.testPass}`}>{passed}✓</span>}
                    {failed > 0 && <span className={`${styles.testBadge} ${styles.testFail}`}>{failed}✗</span>}
                  </div>
                </div>
              );
            })}
          </div>

          {summary && (
            <div className={styles.summary}>
              <div className={styles.summaryItem}>
                <span className={styles.summaryValue}>{summary.totalRequests}</span>
                <span className={styles.summaryLabel}>Requests</span>
              </div>
              <div className={styles.summaryItem}>
                <span className={styles.summaryValue} style={{ color: '#22c55e' }}>{summary.passedTests}</span>
                <span className={styles.summaryLabel}>Passed</span>
              </div>
              <div className={styles.summaryItem}>
                <span className={styles.summaryValue} style={{ color: summary.failedTests > 0 ? '#ef4444' : undefined }}>{summary.failedTests}</span>
                <span className={styles.summaryLabel}>Failed</span>
              </div>
              <div className={styles.summaryItem}>
                <span className={styles.summaryValue}>{(summary.totalTime / 1000).toFixed(1)}s</span>
                <span className={styles.summaryLabel}>Total</span>
              </div>
            </div>
          )}
        </>
      ) : (
        status !== 'running' && (
          <div className={styles.empty}>
            Select a collection and click "Run Collection" to start
          </div>
        )
      )}
    </div>
  );
}

function methodColor(method: string): string {
  switch (method) {
    case 'GET': return styles.methodGet ?? '';
    case 'POST': return styles.methodPost ?? '';
    case 'PUT': return styles.methodPut ?? '';
    case 'PATCH': return styles.methodPatch ?? '';
    case 'DELETE': return styles.methodDelete ?? '';
    default: return '';
  }
}
