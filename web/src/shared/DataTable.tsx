import { Empty, Spin, Table, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useEffect, useMemo, useState } from 'react';

import { containedTableScroll, containedTableStyle } from './tableLayout';

const { Text } = Typography;

const DEFAULT_COLUMN_WIDTH = 160;
const DEFAULT_PAGE_SIZE = 10;
const PAGE_SIZE_OPTIONS = [10, 20, 50, 100];
const adaptiveTableScroll = { x: '100%' };

export type DataTableDisplayMode = 'adaptive' | 'compact';

type DataTableProps<T extends Record<string, unknown>> = {
  columns: Array<ColumnProps<T>>;
  data: T[];
  displayMode?: DataTableDisplayMode;
  error?: Error | null;
  loading?: boolean;
  rowKey?: Extract<keyof T, string> | ((record: T) => string | number);
};

function resolveRowKey<T extends Record<string, unknown>>(rowKey: DataTableProps<T>['rowKey']) {
  if (typeof rowKey === 'function') {
    return (record?: T) => (record ? String(rowKey(record)) : '');
  }

  return rowKey ?? 'id';
}

export function normalizeTableColumns<T extends Record<string, unknown>>(columns: Array<ColumnProps<T>>, displayMode: DataTableDisplayMode = 'compact') {
  return columns.map((column) => {
    if (displayMode === 'compact') {
      return {
        ...column,
        width: typeof column.width === 'number' ? column.width : DEFAULT_COLUMN_WIDTH
      };
    }

    return { ...column };
  });
}

export function DataTable<T extends Record<string, unknown>>({ columns, data, displayMode = 'compact', error, loading, rowKey }: DataTableProps<T>) {
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
  const tableColumns = useMemo(() => normalizeTableColumns(columns, displayMode), [columns, displayMode]);

  useEffect(() => {
    setCurrentPage(1);
  }, [data]);

  const pageData = useMemo(() => {
    const start = (currentPage - 1) * pageSize;
    return data.slice(start, start + pageSize);
  }, [currentPage, data, pageSize]);

  if (loading) {
    return (
      <div style={{ display: 'grid', minHeight: 220, placeItems: 'center' }}>
        <Spin size="large" tip="加载中" />
      </div>
    );
  }

  if (error) {
    return (
      <div role="alert" style={{ padding: 24 }}>
        <Text type="danger">加载失败：{error.message}</Text>
      </div>
    );
  }

  if (data.length === 0) {
    return <Empty description="暂无数据" />;
  }

  return (
    <Table
      bordered
      className={`admin-data-table admin-business-table admin-data-table-${displayMode}`}
      columns={tableColumns}
      dataSource={pageData}
      pagination={{
        currentPage,
        pageSize,
        pageSizeOpts: PAGE_SIZE_OPTIONS,
        showSizeChanger: true,
        total: data.length,
        onPageChange: setCurrentPage,
        onPageSizeChange: (nextPageSize) => {
          setPageSize(nextPageSize);
          setCurrentPage(1);
        }
      }}
      resizable
      rowKey={resolveRowKey(rowKey)}
      scroll={displayMode === 'compact' ? containedTableScroll : adaptiveTableScroll}
      size={displayMode === 'compact' ? 'small' : 'default'}
      style={containedTableStyle}
    />
  );
}
