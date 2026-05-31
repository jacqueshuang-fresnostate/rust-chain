import { Tag } from '@douyinfe/semi-ui';
import type { ComponentProps } from 'react';

type TagColor = NonNullable<ComponentProps<typeof Tag>['color']>;

type StatusTagProps = {
  value?: boolean | number | string | null;
};

type StatusMeta = {
  label: string;
  color: TagColor;
};

const STATUS_MAP: Record<string, StatusMeta> = {
  active: { label: '启用', color: 'green' },
  allow: { label: '放行', color: 'green' },
  approved: { label: '已通过', color: 'green' },
  cancelled: { label: '已取消', color: 'grey' },
  completed: { label: '已完成', color: 'green' },
  deny: { label: '拒绝', color: 'red' },
  disabled: { label: '禁用', color: 'grey' },
  distribution: { label: '派发中', color: 'light-blue' },
  down: { label: '看跌', color: 'red' },
  enabled: { label: '启用', color: 'green' },
  failed: { label: '失败', color: 'red' },
  inactive: { label: '禁用', color: 'grey' },
  liquidated: { label: '已强平', color: 'red' },
  listed: { label: '已上市', color: 'green' },
  locked: { label: '锁定', color: 'orange' },
  long: { label: '做多', color: 'green' },
  loss: { label: '亏损', color: 'red' },
  not_required: { label: '无需支付', color: 'grey' },
  opened: { label: '持仓中', color: 'light-blue' },
  paid: { label: '已支付', color: 'green' },
  partially_filled: { label: '部分成交', color: 'light-blue' },
  pending: { label: '待处理', color: 'orange' },
  preheat: { label: '预热中', color: 'orange' },
  needs_reload: { label: '待重载', color: 'orange' },
  redeemed: { label: '已赎回', color: 'green' },
  rejected: { label: '已拒绝', color: 'red' },
  review: { label: '人工复核', color: 'orange' },
  settled: { label: '已结算', color: 'green' },
  short: { label: '做空', color: 'red' },
  skipped: { label: '已跳过', color: 'grey' },
  subscribed: { label: '已申购', color: 'light-blue' },
  subscription: { label: '发行申购', color: 'light-blue' },
  success: { label: '成功', color: 'green' },
  suspended: { label: '暂停', color: 'orange' },
  unpaid: { label: '未支付', color: 'orange' },
  up: { label: '看涨', color: 'green' },
  win: { label: '盈利', color: 'green' },
  true: { label: '启用', color: 'green' },
  false: { label: '禁用', color: 'grey' }
};

function normalizeStatus(value: StatusTagProps['value']) {
  if (value === null || value === undefined || value === '') {
    return null;
  }

  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }

  return String(value).trim().toLowerCase();
}

export function StatusTag({ value }: StatusTagProps) {
  const normalized = normalizeStatus(value);

  if (!normalized) {
    return <span>-</span>;
  }

  const meta = STATUS_MAP[normalized] ?? { label: String(value), color: 'light-blue' as TagColor };

  return <Tag color={meta.color}>{meta.label}</Tag>;
}
