import styles from './TestResults.module.css';

export interface TestResultItem {
  name: string;
  passed: boolean;
  error?: string;
  duration: number;
}

interface TestResultsProps {
  results: TestResultItem[];
}

export function TestResults({ results }: TestResultsProps) {
  const passed = results.filter((r) => r.passed).length;
  const failed = results.filter((r) => !r.passed).length;
  const total = results.length;

  if (total === 0) {
    return (
      <div className={styles.empty}>
        No test results. Add assertions in your post-response script using <code>nx.test()</code>.
      </div>
    );
  }

  return (
    <div className={styles.results}>
      <div className={styles.summary}>
        <span className={styles.summaryItem}>
          <span className={styles.passIcon}>✓</span> {passed} passed
        </span>
        <span className={styles.summaryItem}>
          <span className={styles.failIcon}>✗</span> {failed} failed
        </span>
        <span className={styles.summaryTotal}>{total} total</span>
      </div>
      <div className={styles.list}>
        {results.map((result, index) => (
          <div
            key={index}
            className={`${styles.item} ${result.passed ? styles.pass : styles.fail}`}
          >
            <span className={styles.icon}>
              {result.passed ? '✓' : '✗'}
            </span>
            <div className={styles.details}>
              <span className={styles.name}>{result.name}</span>
              {result.error && (
                <span className={styles.error}>{result.error}</span>
              )}
            </div>
            <span className={styles.duration}>{result.duration.toFixed(1)}ms</span>
          </div>
        ))}
      </div>
    </div>
  );
}
