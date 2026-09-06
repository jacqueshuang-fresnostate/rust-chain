import { Tag } from '@douyinfe/semi-ui';
import { ADMIN_STATUS_META } from './adminStatus';

type StatusTagProps = {
  label?: string;
  value?: boolean | number | string | null;
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

export function StatusTag({ label, value }: StatusTagProps) {
  const normalized = normalizeStatus(value);

  if (!normalized) {
    return <span>-</span>;
  }

  const meta = (Object.hasOwn(ADMIN_STATUS_META, normalized) ? ADMIN_STATUS_META[normalized] : null) ?? { label: String(value), color: 'light-blue' as const };

  return <Tag color={meta.color}>{label?.trim() || meta.label}</Tag>;
}
