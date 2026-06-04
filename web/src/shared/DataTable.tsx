import { Empty, Spin, Table, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useEffect, useMemo, useState } from 'react';

const { Text } = Typography;

const DEFAULT_PAGE_SIZE = 20;
const PAGE_SIZE_OPTIONS = [10, 20, 50, 100];

type DataTableProps<T extends Record<string, unknown>> = {
  columns: Array<ColumnProps<T>>;
  data: T[];
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

export function DataTable<T extends Record<string, unknown>>({ columns, data, error, loading, rowKey }: DataTableProps<T>) {
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);

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
      className="admin-data-table"
      columns={columns}
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
      rowKey={resolveRowKey(rowKey)}
      scroll={{ x: '100%' }}
      size="small"
    />
  );
}
