import type { CSSProperties } from 'react';
import type { Scroll } from '@douyinfe/semi-ui/lib/es/table/interface';

type TableColumnWithWidth = {
  width?: number | string;
};

export const containedTableStyle: CSSProperties = {
  maxWidth: '100%',
  width: '100%'
};

export const containedTableScroll: Scroll = {
  x: 'max-content'
};

export function containedTableScrollForColumns(columns: readonly TableColumnWithWidth[], extraWidth = 0): Scroll {
  const width = columns.reduce((total, column) => total + (typeof column.width === 'number' ? column.width : 0), extraWidth);
  return width > 0 ? { x: width } : containedTableScroll;
}
