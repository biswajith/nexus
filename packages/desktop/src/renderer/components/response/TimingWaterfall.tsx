import { buildTimingWaterfall, formatDuration } from '../../lib/format-utils.js';
import styles from './TimingWaterfall.module.css';

interface TimingWaterfallProps {
  timing: {
    dns: number;
    tcp: number;
    tls: number;
    ttfb: number;
    download: number;
    total: number;
  };
}

export function TimingWaterfall({ timing }: TimingWaterfallProps) {
  const segments = buildTimingWaterfall(timing);
  const maxMs = timing.total || 1;

  return (
    <div className={styles.waterfall}>
      <div className={styles.totalRow}>
        <span className={styles.totalLabel}>Total</span>
        <span className={styles.totalValue}>{formatDuration(timing.total)}</span>
      </div>
      <div className={styles.segments}>
        {segments.map((seg) => (
          <div key={seg.label} className={styles.segmentRow}>
            <span className={styles.segLabel}>{seg.label}</span>
            <div className={styles.barContainer}>
              <div
                className={styles.bar}
                style={{
                  left: `${(seg.startMs / maxMs) * 100}%`,
                  width: `${Math.max((seg.durationMs / maxMs) * 100, 1)}%`,
                  backgroundColor: seg.color,
                }}
              />
            </div>
            <span className={styles.segValue}>{formatDuration(seg.durationMs)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
