import styles from './ResponseVisualizer.module.css';

interface Props {
  html: string;
}

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
