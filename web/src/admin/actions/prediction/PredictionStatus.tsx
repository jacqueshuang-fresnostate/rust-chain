import { Tag } from '@douyinfe/semi-ui';

import { syncStatusMeta } from './model';

export function PredictionSyncStatusTag({ value }: { value?: string | null }) {
  if (!value) return <span>-</span>;
  const meta = syncStatusMeta[value] ?? { color: 'light-blue' as const, label: value };
  return <Tag color={meta.color}>{meta.label}</Tag>;
}
