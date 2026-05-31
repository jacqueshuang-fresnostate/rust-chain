import { Button, Card, Space, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { type ReactNode, useEffect, useMemo, useState } from 'react';

import { listAdminResource } from '../../api/adminResources';
import type { ApiRecord } from '../../api/types';
import { AmountText } from '../../shared/AmountText';
import { DataTable } from '../../shared/DataTable';
import { FilterBar, type FilterField, type FilterValues } from '../../shared/FilterBar';
import { JsonDrawer } from '../../shared/JsonDrawer';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';

const { Title, Text } = Typography;

export type AdminResourceColumn<T extends ApiRecord> = {
  asset?: string;
  key: Extract<keyof T, string>;
  title: string;
  type?: 'amount' | 'json' | 'status' | 'text' | 'timestamp';
};

type AdminResourcePageProps<T extends ApiRecord> = {
  actions?: ReactNode;
  columns: Array<AdminResourceColumn<T>>;
  endpoint: string;
  filters?: FilterField[];
  responseKey: string;
  title: string;
};

function renderCell<T extends ApiRecord>(column: AdminResourceColumn<T>, value: T[Extract<keyof T, string>]) {
  if (column.type === 'timestamp') {
    return <TimestampText value={typeof value === 'number' ? value : null} />;
  }

  if (column.type === 'amount') {
    return <AmountText asset={column.asset} value={typeof value === 'string' ? value : value === null || value === undefined ? null : String(value)} />;
  }

  if (column.type === 'status') {
    return <StatusTag value={value as boolean | number | string | null | undefined} />;
  }

  if (column.type === 'json') {
    return <Text code>{JSON.stringify(value)}</Text>;
  }

  return <span>{value === null || value === undefined || value === '' ? '-' : String(value)}</span>;
}

export function AdminResourcePage<T extends ApiRecord>({ actions, columns, endpoint, filters, responseKey, title }: AdminResourcePageProps<T>) {
  const [drawerRow, setDrawerRow] = useState<T | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [filterValues, setFilterValues] = useState<FilterValues>({});
  const [loading, setLoading] = useState(true);
  const [rows, setRows] = useState<T[]>([]);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);

    listAdminResource<T>(endpoint, responseKey, filterValues)
      .then((result) => {
        if (!active) {
          return;
        }
        setRows(result.rows);
      })
      .catch((caught: unknown) => {
        if (!active) {
          return;
        }
        setError(caught instanceof Error ? caught : new Error('加载失败'));
        setRows([]);
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [endpoint, filterValues, responseKey]);

  const tableColumns = useMemo<Array<ColumnProps<T>>>(() => {
    const resourceColumns = columns.map<ColumnProps<T>>((column) => ({
      dataIndex: column.key,
      render: (value: T[Extract<keyof T, string>]) => renderCell(column, value),
      title: column.title
    }));

    return [
      ...resourceColumns,
      {
        render: (_value: unknown, record: T) => (
          <Button onClick={() => setDrawerRow(record)} size="small" theme="borderless">
            查看JSON
          </Button>
        ),
        title: '操作'
      }
    ];
  }, [columns]);

  return (
    <main className="exchange-page">
      <Card bordered={false} shadows="always">
        <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
          <div className="admin-resource-header">
            <div>
              <Title heading={3} style={{ marginBottom: 8 }}>
                {title}
              </Title>
              <Text type="secondary">后台资源检索视图，敏感操作需走二次确认。</Text>
            </div>
            {actions ? <div className="admin-resource-actions">{actions}</div> : null}
          </div>
          <FilterBar fields={filters} loading={loading} onChange={setFilterValues} value={filterValues} />
          <DataTable columns={tableColumns} data={rows} error={error} loading={loading} />
        </Space>
      </Card>
      <JsonDrawer data={drawerRow} onClose={() => setDrawerRow(null)} visible={drawerRow !== null} />
    </main>
  );
}
