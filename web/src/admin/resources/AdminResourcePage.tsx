import { Button, Card, Space, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';

import { listAdminResource } from '../../api/adminResources';
import type { ApiRecord } from '../../api/types';
import { AmountText } from '../../shared/AmountText';
import { DataTable } from '../../shared/DataTable';
import { FilterBar, type FilterField, type FilterValues } from '../../shared/FilterBar';
import { DetailDrawer, type DetailDrawerData } from '../../shared/DetailDrawer';
import { formatAdminDisplayValue } from '../../shared/numberFormat';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';

const { Title, Text } = Typography;

export type AdminResourceColumn<T extends ApiRecord> = {
  asset?: string;
  key: Extract<keyof T, string>;
  render?: (record: T) => ReactNode;
  title: string;
  type?: 'amount' | 'json' | 'status' | 'text' | 'timestamp';
  valueMap?: Record<string, string>;
};

type AdminResourceActionHelpers = {
  reload: () => void;
};

type AdminResourcePageProps<T extends ApiRecord> = {
  actions?: ReactNode | ((helpers: AdminResourceActionHelpers) => ReactNode);
  columns: Array<AdminResourceColumn<T>>;
  endpoint: string;
  filters?: FilterField[];
  responseKey: string;
  rowActions?: (
    record: T,
    helpers: {
      reload: () => void;
      openDetail: (detail: DetailDrawerData) => void;
    }
  ) => ReactNode;
  showJsonAction?: boolean;
  title: string;
};

function renderCell<T extends ApiRecord>(column: AdminResourceColumn<T>, value: T[Extract<keyof T, string>]) {
  const mappedValue = value === null || value === undefined ? undefined : column.valueMap?.[String(value)];
  if (mappedValue) {
    return <span>{mappedValue}</span>;
  }

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

  if (value === null || value === undefined || value === '') {
    return <span>-</span>;
  }

  return <span>{formatAdminDisplayValue(column.key, value) ?? String(value)}</span>;
}

export function AdminResourcePage<T extends ApiRecord>({
  actions,
  columns,
  endpoint,
  filters,
  responseKey,
  rowActions,
  showJsonAction = true,
  title
}: AdminResourcePageProps<T>) {
  const [detail, setDetail] = useState<DetailDrawerData | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [filterValues, setFilterValues] = useState<FilterValues>({});
  const [loading, setLoading] = useState(true);
  const [reloadVersion, setReloadVersion] = useState(0);
  const [rows, setRows] = useState<T[]>([]);
  const reload = useCallback(() => setReloadVersion((value) => value + 1), []);

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
  }, [endpoint, filterValues, reloadVersion, responseKey]);

  const filterFields = useMemo(
    () =>
      filters?.map((field) => {
        if (!field.optionsFromRows) {
          return field;
        }

        const options = rows
          .map((row) => row[field.key])
          .filter((value): value is string | number => typeof value === 'string' || typeof value === 'number')
          .map((value) => String(value))
          .filter((item, index, values) => item.length > 0 && values.indexOf(item) === index)
          .map((item) => ({ label: item, value: item }));

        return { ...field, options };
      }),
    [filters, rows]
  );

  const renderedActions = typeof actions === 'function' ? actions({ reload }) : actions;

  const tableColumns = useMemo<Array<ColumnProps<T>>>(() => {
    const resourceColumns = columns.map<ColumnProps<T>>((column) => ({
      dataIndex: column.key,
      key: `${column.key}-${column.title}`,
      render: (value: T[Extract<keyof T, string>], record: T) => (column.render ? column.render(record) : renderCell(column, value)),
      title: column.title
    }));

    return [
      ...resourceColumns,
      {
        fixed: 'right',
        render: (_value: unknown, record: T) => (
          <Space spacing={6} wrap>
            {rowActions?.(record, { reload, openDetail: setDetail })}
            {showJsonAction && !rowActions ? (
              <Button onClick={() => setDetail({ title: '详情', data: record })} size="small" theme="borderless">
                查看详情
              </Button>
            ) : null}
          </Space>
        ),
        title: '操作',
        width: 180
      }
    ];
  }, [columns, reload, rowActions, showJsonAction]);

  return (
    <main className="exchange-page">
      <Card bordered={false} shadows="always">
        <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
          <div className="admin-resource-header">
            <Title heading={3} style={{ marginBottom: 8 }}>
              {title}
            </Title>
            {renderedActions ? <div className="admin-resource-actions">{renderedActions}</div> : null}
          </div>
          <FilterBar fields={filterFields} loading={loading} onChange={setFilterValues} value={filterValues} />
          <DataTable columns={tableColumns} data={rows} error={error} loading={loading} />
        </Space>
      </Card>
      <DetailDrawer detail={detail} onClose={() => setDetail(null)} />
    </main>
  );
}
