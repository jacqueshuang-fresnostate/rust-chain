import { Empty, Spin, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps, RowSelectionProps } from '@douyinfe/semi-ui/lib/es/table';
import { useEffect, useMemo, useState } from 'react';

import { ResizableTable, RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH } from './ResizableTable';
import { containedTableStyle } from './tableLayout';

const { Text } = Typography;

export const DEFAULT_PAGE_SIZE = 10;
const PAGE_SIZE_OPTIONS = [10, 20, 50, 100];

export type DataTableDisplayMode = 'adaptive' | 'compact';

// 由外部驱动的服务端分页：data 即当前页，total 来自服务端。
export type DataTableServerPagination = {
  currentPage: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: number) => void;
  pageSize: number;
  total: number;
};

type DataTableProps<T extends Record<string, unknown>> = {
  columns: Array<ColumnProps<T>>;
  data: T[];
  displayMode?: DataTableDisplayMode;
  error?: Error | null;
  loading?: boolean;
  pagination?: DataTableServerPagination;
  rowKey?: Extract<keyof T, string> | ((record: T) => string | number);
  rowSelection?: RowSelectionProps<T>;
};

function resolveRowKey<T extends Record<string, unknown>>(rowKey: DataTableProps<T>['rowKey']) {
  if (typeof rowKey === 'function') {
    return (record?: T) => (record ? String(rowKey(record)) : '');
  }

  return rowKey ?? 'id';
}

export function normalizeTableColumns<T extends Record<string, unknown>>(columns: Array<ColumnProps<T>>, displayMode: DataTableDisplayMode = 'compact') {
  const normalize = (column: ColumnProps<T>): ColumnProps<T> => {
    if (Array.isArray(column.children) && column.children.length > 0) {
      return {
        ...column,
        children: column.children.map(normalize)
      };
    }
    if (displayMode === 'compact') {
      return {
        ...column,
        width: typeof column.width === 'number' && Number.isFinite(column.width) ? column.width : RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH
      };
    }

    return { ...column };
  };

  return columns.map(normalize);
}

export function DataTable<T extends Record<string, unknown>>({ columns, data, displayMode = 'compact', error, loading, pagination, rowKey, rowSelection }: DataTableProps<T>) {
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
  const tableColumns = useMemo(() => normalizeTableColumns(columns, displayMode), [columns, displayMode]);

  useEffect(() => {
    setCurrentPage(1);
  }, [data]);

  const pageData = useMemo(() => {
    if (pagination) {
      return data;
    }

    const start = (currentPage - 1) * pageSize;
    return data.slice(start, start + pageSize);
  }, [currentPage, data, pageSize, pagination]);
  const tablePagination = useMemo(
    () => ({
      currentPage: pagination ? pagination.currentPage : currentPage,
      pageSize: pagination ? pagination.pageSize : pageSize,
      pageSizeOpts: PAGE_SIZE_OPTIONS,
      showSizeChanger: true,
      total: pagination ? pagination.total : data.length,
      onPageChange: pagination ? pagination.onPageChange : setCurrentPage,
      onPageSizeChange: (nextPageSize: number) => {
        if (pagination) {
          pagination.onPageSizeChange(nextPageSize);
          return;
        }
        setPageSize(nextPageSize);
        setCurrentPage(1);
      }
    }),
    [currentPage, data.length, pageSize, pagination]
  );

  if (loading) {
    return (
      <div aria-live="polite" className="admin-table-state admin-table-loading" role="status">
        <Spin size="large" tip="加载中" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="admin-table-state admin-table-error" role="alert">
        <Text type="danger">加载失败：{error.message}</Text>
        <Text type="tertiary">请检查网络连接后刷新当前资源。</Text>
      </div>
    );
  }

  if (data.length === 0 && !(pagination && pagination.total > 0)) {
    return (
      <div aria-live="polite" className="admin-table-state admin-table-empty" role="status">
        <Empty description="暂无数据" />
        <Text type="tertiary">当前条件下没有可展示的记录，可调整筛选条件后重新查询。</Text>
      </div>
    );
  }

  return (
    <ResizableTable
      bordered
      className={`admin-data-table admin-business-table admin-data-table-${displayMode}`}
      columns={tableColumns}
      dataSource={pageData}
      pagination={tablePagination}
      rowKey={resolveRowKey(rowKey)}
      rowSelection={rowSelection}
      size={displayMode === 'compact' ? 'small' : 'default'}
      style={containedTableStyle}
    />
  );
}
