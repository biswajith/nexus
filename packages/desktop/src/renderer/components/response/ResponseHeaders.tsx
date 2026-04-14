import styles from './ResponseHeaders.module.css';

interface ResponseHeadersProps {
  headers: Record<string, string | string[]>;
}

export function ResponseHeaders({ headers }: ResponseHeadersProps) {
  const entries = Object.entries(headers).sort(([a], [b]) => a.localeCompare(b));

  return (
    <div className={styles.headers}>
      <table className={styles.table}>
        <thead>
          <tr>
            <th className={styles.th}>Name</th>
            <th className={styles.th}>Value</th>
          </tr>
        </thead>
        <tbody>
          {entries.map(([name, value]) => (
            <tr key={name} className={styles.row}>
              <td className={styles.name}>{name}</td>
              <td className={styles.value}>{Array.isArray(value) ? value.join(', ') : value}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {entries.length === 0 && (
        <div className={styles.empty}>No headers in response</div>
      )}
    </div>
  );
}
