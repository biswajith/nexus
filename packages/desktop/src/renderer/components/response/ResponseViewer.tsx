import { useUiStore } from '../../stores/ui-store.js';
import { formatSize, formatDuration, getStatusColor } from '../../lib/format-utils.js';
import { ResponseBody } from './ResponseBody.js';
import { ResponseHeaders } from './ResponseHeaders.js';
import { TimingWaterfall } from './TimingWaterfall.js';
import { TestResults } from './TestResults.js';
import { ResponseVisualizer } from './ResponseVisualizer.js';
import type { TestResultItem } from './TestResults.js';
import styles from './ResponseViewer.module.css';

interface ResponseViewerProps {
  tab: {
    id: string;
    response: {
      status: number;
      statusText: string;
      headers: Record<string, string | string[]>;
      bodyText: string;
      bodyJson?: unknown;
      timing: { dns: number; tcp: number; tls: number; ttfb: number; download: number; total: number };
      size: number;
      testResults?: TestResultItem[];
      visualizerHtml?: string;
    } | null;
    loading: boolean;
  };
}

export function ResponseViewer({ tab }: ResponseViewerProps) {
  const responseTab = useUiStore((s) => s.responseTab);
  const setResponseTab = useUiStore((s) => s.setResponseTab);

  if (tab.loading) {
    return (
      <div className={styles.loading}>
        <div className={styles.spinner} role="status" aria-label="Sending request" />
        <span>Sending request...</span>
      </div>
    );
  }

  if (!tab.response) {
    return (
      <div className={styles.empty}>
        <div className={styles.emptyContent}>
          <span>Hit Send to get a response</span>
          <span className={styles.emptyHint}>Or press Enter in the URL bar</span>
        </div>
      </div>
    );
  }

  const { response } = tab;
  const statusColorClass = getStatusColor(response.status);
  type ResponseTab = 'body' | 'headers' | 'tests' | 'timing' | 'visualize';
  const hasVisualizer = !!response.visualizerHtml;

  const tabs: ResponseTab[] = ['body', 'headers', 'tests', 'timing', ...(hasVisualizer ? ['visualize' as const] : [])];

  return (
    <div className={styles.viewer}>
      <div className={styles.statusBar}>
        <span className={`${styles.statusBadge} ${styles[statusColorClass]}`}>
          {response.status} {response.statusText}
        </span>
        <span className={styles.meta}>{formatDuration(response.timing.total)}</span>
        <span className={styles.meta}>{formatSize(response.size)}</span>
      </div>

      <div className={styles.tabRow}>
        {tabs.map((t) => {
          const testCount = response.testResults?.length ?? 0;
          const passedCount = response.testResults?.filter((r) => r.passed).length ?? 0;
          let label = '';
          if (t === 'body') label = 'Body';
          else if (t === 'headers') label = `Headers (${Object.keys(response.headers).length})`;
          else if (t === 'tests') label = `Tests${testCount > 0 ? ` (${passedCount}/${testCount})` : ''}`;
          else if (t === 'timing') label = 'Timing';
          else if (t === 'visualize') label = 'Visualize';
          return (
            <button
              key={t}
              className={`${styles.tabBtn} ${responseTab === t ? styles.activeTab : ''}`}
              onClick={() => setResponseTab(t)}
            >
              {label}
            </button>
          );
        })}
      </div>

      <div className={styles.tabContent}>
        {responseTab === 'body' && <ResponseBody response={response} />}
        {responseTab === 'headers' && <ResponseHeaders headers={response.headers} />}
        {responseTab === 'tests' && <TestResults results={response.testResults ?? []} />}
        {responseTab === 'timing' && <TimingWaterfall timing={response.timing} />}
        {responseTab === 'visualize' && hasVisualizer && (
          <ResponseVisualizer html={response.visualizerHtml!} />
        )}
      </div>
    </div>
  );
}
