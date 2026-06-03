import { SideSheet, Table, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';

import type { ApiRecord } from '../api/types';
import { formatAdminDisplayValue } from './numberFormat';

const { Text } = Typography;

type DetailDrawerData = {
  data: ApiRecord | ApiRecord[];
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

function fieldLabel(key: string) {
  const labels: Record<string, string> = {
    id: 'ID',
    user_id: '用户ID',
    email: '邮箱',
    phone: '手机号',
    status: '状态',
    kyc_level: 'KYC等级',
    asset_id: '资产ID',
    asset_symbol: '资产',
    available: '可用',
    frozen: '冻结',
    locked: '锁定',
    created_at: '创建时间',
    updated_at: '更新时间'
  };

  return labels[key] ?? key;
}

function displayValue(value: unknown, key = ''): string {
  if (value === null || value === undefined || value === '') {
    return '-';
  }

  if (Array.isArray(value)) {
    return value.map((item) => displayValue(item, key)).join(' / ');
  }

  if (typeof value === 'object') {
    return Object.entries(value as ApiRecord)
      .map(([itemKey, item]) => `${fieldLabel(itemKey)}: ${displayValue(item, itemKey)}`)
      .join('；');
  }

  return formatAdminDisplayValue(key, value) ?? String(value);
}

function toRows(record: ApiRecord): FieldRow[] {
  return Object.entries(record).map(([field, value]) => ({ field, value }));
}

const fieldColumns: Array<ColumnProps<FieldRow>> = [
  {
    dataIndex: 'field',
    title: '字段',
    render: (value: string) => <Text strong>{fieldLabel(value)}</Text>
  },
  {
    dataIndex: 'value',
    title: '内容',
    render: (value: unknown, row: FieldRow) => <span>{displayValue(value, row.field)}</span>
  }
];

function recordColumns(records: ApiRecord[]): Array<ColumnProps<ApiRecord>> {
  const keys = [...new Set(records.flatMap((record) => Object.keys(record)))];
  return keys.map((key) => ({
    dataIndex: key,
    title: fieldLabel(key),
    render: (value: unknown) => <span>{displayValue(value, key)}</span>
  }));
}

export function DetailDrawer({ detail, onClose }: DetailDrawerProps) {
  const data = detail?.data;
  const records = Array.isArray(data) ? data : [];

  return (
    <SideSheet onCancel={onClose} title={detail?.title ?? '详情'} visible={detail !== null} width="80%">
      {Array.isArray(data) ? (
        <Table columns={recordColumns(records)} dataSource={records} pagination={false} rowKey={(record) => String(record?.id ?? displayValue(record))} />
      ) : (
        <Table columns={fieldColumns} dataSource={data ? toRows(data) : []} pagination={false} rowKey="field" />
      )}
    </SideSheet>
  );
}

export type { DetailDrawerData };
