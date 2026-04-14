import type { CSSProperties, ReactElement } from 'react';
import { useMemo } from 'react';
import { List } from 'react-window';

interface VirtualListRowPayload<T> {
  items: T[];
  renderItem: (item: T, index: number, style: CSSProperties) => ReactElement;
}

function VirtualListRow<T>(
  props: { index: number; style: CSSProperties } & VirtualListRowPayload<T>,
): ReactElement {
  return props.renderItem(props.items[props.index]!, props.index, props.style);
}

interface VirtualListProps<T> {
  items: T[];
  rowHeight: number;
  renderItem: (item: T, index: number, style: CSSProperties) => ReactElement;
  className?: string;
  overscanCount?: number;
}

/**
 * Virtualized list using react-window to render only visible rows for large item sets.
 */
export function VirtualList<T>({
  items,
  rowHeight,
  renderItem,
  className,
  overscanCount = 10,
}: VirtualListProps<T>) {
  const rowProps = useMemo<VirtualListRowPayload<T>>(
    () => ({ items, renderItem }),
    [items, renderItem],
  );

  if (items.length === 0) return null;

  return (
    <List<VirtualListRowPayload<T>>
      rowComponent={VirtualListRow<T>}
      rowCount={items.length}
      rowHeight={rowHeight}
      rowProps={rowProps}
      className={className}
      overscanCount={overscanCount}
    />
  );
}
