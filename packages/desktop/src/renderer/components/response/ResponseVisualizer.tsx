import styles from './ResponseVisualizer.module.css';

interface Props {
  html: string;
}

/**
 * Embeds custom visualization HTML in a sandboxed iframe (scripts allowed) for the Visualize response tab.
 */
export function ResponseVisualizer({ html }: Props) {
  return (
    <div className={styles.container}>
      <iframe
        className={styles.frame}
        srcDoc={html}
        sandbox="allow-scripts"
        title="Response visualization"
      />
    </div>
  );
}
