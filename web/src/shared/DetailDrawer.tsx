import { adminErrorFieldValue } from './adminErrorMessage';
import { SideSheet, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';

import { adminFieldLabel, adminEnumLabel } from './adminPresentation';
import type { ApiRecord } from '../api/types';
import { formatAdminBetContent, isAdminBetContentField } from './betContentFormat';
import { formatAdminDisplayValue, formatAdminNumber } from './numberFormat';
import { formatBusinessOrderNo } from './orderNo';
import { ResizableTable } from './ResizableTable';
import { containedTableStyle } from './tableLayout';
import { formatAdminTimestamp } from './TimestampText';

const { Text } = Typography;

type DetailFieldType = 'amount' | 'json' | 'status' | 'text' | 'timestamp';

type DetailDrawerFieldMeta = {
  assets?: Record<string, string | undefined>;
  labels?: Record<string, string>;
  types?: Record<string, DetailFieldType | undefined>;
  valueMaps?: Record<string, Record<string, string> | undefined>;
};

type DetailDrawerData = {
  data: ApiRecord | ApiRecord[];
  fieldMeta?: DetailDrawerFieldMeta;
  title?: string;
};

type DetailDrawerProps = {
  detail: DetailDrawerData | null;
  onClose: () => void;
};

type FieldRow = {
  field: string;
  value: unknown;
};

function fieldLabel(key: string, meta?: DetailDrawerFieldMeta) {
  const label = meta?.labels?.[key];
  return typeof label === 'string' ? label : adminFieldLabel(key);
}

function mappedValue(value: unknown, key: string, meta?: DetailDrawerFieldMeta): string | null {
  if (value === null || value === undefined || value === '') {
    return null;
  }
  const label = meta?.valueMaps?.[key]?.[String(value)];
  return typeof label === 'string' ? label : adminEnumLabel(key, value, meta?.types?.[key] === 'status');
}

function businessOrderFieldValue(value: unknown, key: string): string | null {
  const prefixes: Record<string, string> = {
    buy_order_id: 'SP',
    sell_order_id: 'SP',
    subscription_id: 'NC'
  };
  const prefix = prefixes[key];
  if (typeof prefix !== 'string' || value === null || value === undefined || value === '') {
    return null;
  }
  return formatBusinessOrderNo(prefix, { id: value });
}

function typedDisplayValue(value: unknown, key: string, meta?: DetailDrawerFieldMeta): string | null {
  const orderNoValue = businessOrderFieldValue(value, key);
  if (orderNoValue) {
    return orderNoValue;
  }

  const type = meta?.types?.[key];
  if ((type === 'timestamp' || (type === undefined && adminFieldLabel(key) !== key && /(?:_at|_time)$/.test(key))) && typeof value === 'number') {
    return formatAdminTimestamp(value);
  }
  if (type === 'amount') {
    const formatted = typeof value === 'string' || typeof value === 'number' ? formatAdminNumber(value) : null;
    return formatted ? `${formatted}${meta?.assets?.[key] ? ` ${meta.assets[key]}` : ''}` : null;
  }
  if (type === 'status') {
    return mappedValue(value, key, meta);
  }
  return null;
}

export function displayDetailValue(value: unknown, key = '', meta?: DetailDrawerFieldMeta): string {
  if (value === null || value === undefined || value === '') {
    return '-';
  }

  if (isAdminBetContentField(key, fieldLabel(key, meta))) {
    const formattedBetContent = formatAdminBetContent(value);
    if (formattedBetContent) {
      return formattedBetContent;
    }
  }

  if (Array.isArray(value)) {
    return value.map((item) => displayDetailValue(item, key, meta)).join(' / ');
  }

  if (typeof value === 'object') {
    const nestedMeta = key === 'nodes' ? { ...meta, labels: { ...meta?.labels, target_type: '目标类型' } } : meta;
    return Object.entries(value as ApiRecord)
      .map(([itemKey, item]) => `${fieldLabel(itemKey, nestedMeta)}: ${displayDetailValue(item, itemKey, nestedMeta)}`)
      .join('；');
  }

  const typed = typedDisplayValue(value, key, meta);
  if (typed) {
    return typed;
  }

  const mapped = mappedValue(value, key, meta);
  if (mapped) {
    return mapped;
  }

  if (typeof value === 'boolean') return value ? '是' : '否';
  return adminErrorFieldValue(key, value) ?? formatAdminDisplayValue(key, value) ?? String(value);
}

function toRows(record: ApiRecord): FieldRow[] {
  return Object.entries(record).map(([field, value]) => ({ field, value }));
}

function fieldColumns(meta?: DetailDrawerFieldMeta): Array<ColumnProps<FieldRow>> {
  return [
    {
      dataIndex: 'field',
      title: '字段',
      width: 220,
      render: (value: string) => <Text strong>{fieldLabel(value, meta)}</Text>
    },
    {
      dataIndex: 'value',
      title: '内容',
      width: 640,
      render: (value: unknown, row: FieldRow) => <span>{displayDetailValue(value, row.field, meta)}</span>
    }
  ];
}

function recordColumns(records: ApiRecord[], meta?: DetailDrawerFieldMeta): Array<ColumnProps<ApiRecord>> {
  const keys = [...new Set(records.flatMap((record) => Object.keys(record)))];
  return keys.map((key) => ({
    dataIndex: key,
    title: fieldLabel(key, meta),
    width: 180,
    render: (value: unknown) => <span>{displayDetailValue(value, key, meta)}</span>
  }));
}

type DetailFieldTableProps = {
  fieldMeta?: DetailDrawerFieldMeta;
  record: ApiRecord | null;
};

export function DetailFieldTable({ fieldMeta, record }: DetailFieldTableProps) {
  return (
    <ResizableTable
      bordered
      columns={fieldColumns(fieldMeta)}
      dataSource={record ? toRows(record) : []}
      pagination={false}
      rowKey="field"
      style={containedTableStyle}
    />
  );
}

export function DetailDrawer({ detail, onClose }: DetailDrawerProps) {
  const data = detail?.data;
  const meta = detail?.fieldMeta;
  const records = Array.isArray(data) ? data : [];

  return (
    <SideSheet
      bodyStyle={{ overflowY: 'auto' }}
      className="admin-detail-drawer"
      closeOnEsc
      maskClosable={false}
      onCancel={onClose}
      title={detail?.title ?? '详情'}
      visible={detail !== null}
      width={920}
    >
      {Array.isArray(data) ? (
        <ResizableTable
          bordered
          columns={recordColumns(records, meta)}
          dataSource={records}
          pagination={false}
          rowKey={(record) => String(record?.id ?? displayDetailValue(record))}
          style={containedTableStyle}
        />
      ) : (
        <DetailFieldTable fieldMeta={meta} record={data ?? null} />
      )}
    </SideSheet>
  );
}

export type { DetailDrawerData, DetailDrawerFieldMeta, DetailFieldType };
