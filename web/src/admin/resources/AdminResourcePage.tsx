import { IconEyeOpened, IconList, IconRefresh } from '@douyinfe/semi-icons';
import { Button, Card, Space, Switch, Tooltip, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';

import { listAdminResource } from '../../api/adminResources';
import type { ApiRecord } from '../../api/types';
import { AmountText } from '../../shared/AmountText';
import { formatAdminBetContent, isAdminBetContentField } from '../../shared/betContentFormat';
import { DataTable, type DataTableDisplayMode } from '../../shared/DataTable';
import { FilterBar, type FilterField, type FilterValues } from '../../shared/FilterBar';
import { DetailDrawer, type DetailDrawerData, type DetailDrawerFieldMeta } from '../../shared/DetailDrawer';
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
  toolbarFilters?: FilterField[];
};

function renderCell<T extends ApiRecord>(column: AdminResourceColumn<T>, value: T[Extract<keyof T, string>]) {
  const mappedValue = value === null || value === undefined ? undefined : column.valueMap?.[String(value)];
  if (mappedValue) {
    return <span>{mappedValue}</span>;
  }

  if (isAdminBetContentField(column.key, column.title)) {
    const formattedBetContent = formatAdminBetContent(value);
    if (formattedBetContent) {
      return <span>{formattedBetContent}</span>;
    }
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

function mergeDetailFieldMeta(base: DetailDrawerFieldMeta, next?: DetailDrawerFieldMeta): DetailDrawerFieldMeta {
  return {
    assets: { ...base.assets, ...next?.assets },
    labels: { ...base.labels, ...next?.labels },
    types: { ...base.types, ...next?.types },
    valueMaps: { ...base.valueMaps, ...next?.valueMaps }
  };
}

export function AdminResourcePage<T extends ApiRecord>({
  actions,
  columns,
  endpoint,
  filters,
  responseKey,
  rowActions,
  showJsonAction = true,
  title,
  toolbarFilters
}: AdminResourcePageProps<T>) {
  const [detail, setDetail] = useState<DetailDrawerData | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [filterValues, setFilterValues] = useState<FilterValues>({});
  const [toolbarFilterValues, setToolbarFilterValues] = useState<FilterValues>({});
  const [loading, setLoading] = useState(true);
  const [reloadVersion, setReloadVersion] = useState(0);
  const [rows, setRows] = useState<T[]>([]);
  const [tableDisplayMode, setTableDisplayMode] = useState<DataTableDisplayMode>('compact');
  const reload = useCallback(() => setReloadVersion((value) => value + 1), []);
  const handleFilterChange = useCallback((values: FilterValues) => {
    setFilterValues(values);
  }, []);
  const updateToolbarFilter = useCallback((field: FilterField, nextValue: string) => {
    setToolbarFilterValues((current) => {
      const next = { ...current };
      if (nextValue.trim()) {
        next[field.key] = nextValue;
      } else {
        delete next[field.key];
      }
      return next;
    });
  }, []);
  const requestFilterValues = useMemo(
    () => ({
      ...filterValues,
      ...toolbarFilterValues
    }),
    [filterValues, toolbarFilterValues]
  );

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);

    listAdminResource<T>(endpoint, responseKey, requestFilterValues)
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
  }, [endpoint, reloadVersion, requestFilterValues, responseKey]);

  const filterFields = useMemo(
    () =>
      filters?.map((field) => {
        if (!field.optionsFromRows) {
          return field;
        }

        const optionLabels = rows.reduce((options, row) => {
          const rawValue = row[field.key];
          if (typeof rawValue !== 'string' && typeof rawValue !== 'number') {
            return options;
          }

          const value = String(rawValue);
          if (value.length === 0 || options.has(value)) {
            return options;
          }

          const rawLabel = field.optionLabelKey ? row[field.optionLabelKey] : rawValue;
          const label = typeof rawLabel === 'string' || typeof rawLabel === 'number' ? String(rawLabel) : value;
          options.set(value, label.trim().length > 0 ? label : value);
          return options;
        }, new Map<string, string>());
        const options = [...optionLabels].map(([value, label]) => ({ label, value }));

        return { ...field, options };
      }),
    [filters, rows]
  );

  const renderedActions = typeof actions === 'function' ? actions({ reload }) : actions;
  const renderedToolbarFilters = toolbarFilters?.map((field) => {
    if (field.type !== 'switch') {
      return null;
    }
    return (
      <label className="admin-resource-head-switch" key={field.key}>
        <span>{field.label}</span>
        <Switch
          aria-label={field.label}
          checked={toolbarFilterValues[field.key] === 'true'}
          checkedText="开"
          disabled={loading}
          onChange={(checked) => updateToolbarFilter(field, checked ? 'true' : '')}
          uncheckedText="关"
        />
      </label>
    );
  });
  const activeFilterCount = Object.keys(requestFilterValues).length;
  const filterCount = filterFields?.length ?? 0;
  const nextDisplayMode: DataTableDisplayMode = tableDisplayMode === 'adaptive' ? 'compact' : 'adaptive';
  const displayModeButtonText = tableDisplayMode === 'adaptive' ? '自适应列表' : '紧凑列表';
  const detailFieldMeta = useMemo<DetailDrawerFieldMeta>(
    () =>
      columns.reduce<DetailDrawerFieldMeta>(
        (meta, column) => {
          meta.labels = { ...meta.labels, [column.key]: column.title };
          meta.types = { ...meta.types, [column.key]: column.type };
          if (column.asset) {
            meta.assets = { ...meta.assets, [column.key]: column.asset };
          }
          if (column.valueMap) {
            meta.valueMaps = { ...meta.valueMaps, [column.key]: column.valueMap };
          }
          return meta;
        },
        { assets: {}, labels: {}, types: {}, valueMaps: {} }
      ),
    [columns]
  );
  const openDetail = useCallback(
    (nextDetail: DetailDrawerData) =>
      setDetail({
        ...nextDetail,
        fieldMeta: mergeDetailFieldMeta(detailFieldMeta, nextDetail.fieldMeta)
      }),
    [detailFieldMeta]
  );

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
            {rowActions?.(record, { reload, openDetail })}
            {showJsonAction && !rowActions ? (
              <Button icon={<IconEyeOpened aria-hidden="true" />} onClick={() => openDetail({ title: '详情', data: record })} size="small" theme="borderless">
                查看详情
              </Button>
            ) : null}
          </Space>
        ),
        title: '操作',
        width: 300
      }
    ];
  }, [columns, openDetail, reload, rowActions, showJsonAction]);

  return (
    <main className="exchange-page">
      <Card bordered={false} className="admin-resource-shell">
        <div className="admin-resource-head">
          <div>
            <Title heading={4} style={{ marginBottom: 6 }}>
              {title}
            </Title>
            <Text type="tertiary">共 {rows.length} 条记录，{activeFilterCount > 0 ? `已启用 ${activeFilterCount} 个筛选` : '未启用筛选'}</Text>
          </div>
          <Space align="center" className="admin-resource-head-actions" spacing={12} wrap>
            {renderedToolbarFilters}
            <Button
              icon={<IconList aria-hidden="true" />}
              onClick={() => setTableDisplayMode(nextDisplayMode)}
              theme="light"
              type="tertiary"
            >
              {displayModeButtonText}
            </Button>
          </Space>
        </div>
        <div className="admin-resource-toolbar">
          <Space className="admin-resource-toolbar-actions" spacing={10} wrap>
            {renderedActions}
            <Tooltip content="重新加载当前资源">
              <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={reload} theme="borderless">
                刷新
              </Button>
            </Tooltip>
          </Space>
          {filterCount > 0 ? (
            <div className="admin-resource-toolbar-filters">
              <FilterBar fields={filterFields} loading={loading} onChange={handleFilterChange} value={filterValues} />
            </div>
          ) : null}
        </div>
        <DataTable columns={tableColumns} data={rows} displayMode={tableDisplayMode} error={error} loading={loading} />
      </Card>
      <DetailDrawer detail={detail} onClose={() => setDetail(null)} />
    </main>
  );
}
