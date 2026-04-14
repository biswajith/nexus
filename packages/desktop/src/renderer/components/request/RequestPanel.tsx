import { useState, useCallback, useRef } from 'react';
import { useRequestStore } from '../../stores/request-store.js';
import { TabBar } from './TabBar.js';
import { RequestBuilder } from './RequestBuilder.js';
import { ResponseViewer } from '../response/ResponseViewer.js';
import styles from './RequestPanel.module.css';

const MIN_PANE = 120;
const DEFAULT_SPLIT = 0.45;

export function RequestPanel() {
  const tabs = useRequestStore((s) => s.tabs);
  const activeTabId = useRequestStore((s) => s.activeTabId);
  const activeTab = tabs.find((t) => t.id === activeTabId);

  const containerRef = useRef<HTMLDivElement>(null);
  const [splitRatio, setSplitRatio] = useState(DEFAULT_SPLIT);
  const dragging = useRef(false);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;

    const onMove = (ev: MouseEvent) => {
      if (!dragging.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const totalH = rect.height;
      const y = ev.clientY - rect.top;
      const clamped = Math.max(MIN_PANE, Math.min(y, totalH - MIN_PANE));
      setSplitRatio(clamped / totalH);
    };

    const onUp = () => {
      dragging.current = false;
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };

    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }, []);

  return (
    <div className={styles.panel}>
      <TabBar />
      {activeTab ? (
        <div className={styles.content} ref={containerRef}>
          <div className={styles.requestSection} style={{ flex: `0 0 ${splitRatio * 100}%` }}>
            <RequestBuilder tab={activeTab} />
          </div>
          <div className={styles.divider} onMouseDown={handleMouseDown}>
            <div className={styles.dividerHandle} />
          </div>
          <div className={styles.responseSection}>
            <ResponseViewer tab={activeTab} />
          </div>
        </div>
      ) : (
        <div className={styles.empty}>
          <p>Open a request or create a new one to get started.</p>
        </div>
      )}
    </div>
  );
}
