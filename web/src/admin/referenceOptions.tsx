import { Typography } from '@douyinfe/semi-ui';
import { useMemo } from 'react';

import { listAdminResource } from '../api/adminResources';
import type { ApiRecord } from '../api/types';
import { AdminSelect } from '../shared/SemiFormControls';
import { useSharedAdminOptionQuery } from './sharedOptionQuery';

const { Text } = Typography;

export type AdminReferenceKind = 'agent' | 'asset' | 'marketPair' | 'newCoinProject' | 'user';

export type AdminReferenceOption = {
  code?: string;
  constraint: string;
  disabled: boolean;
  disabledReason?: string;
  label: string;
  lifecycleStatus?: string;
  status: string;
  statusLabel: string;
  value: string;
};

type ReferenceSource = {
  endpoint: string;
  responseKey: string;
};

const referenceSources: Record<AdminReferenceKind, ReferenceSource> = {
  agent: { endpoint: '/admin/api/v1/agents', responseKey: 'agents' },
  asset: { endpoint: '/admin/api/v1/assets', responseKey: 'assets' },
  marketPair: { endpoint: '/admin/api/v1/market-pairs', responseKey: 'pairs' },
  newCoinProject: { endpoint: '/admin/api/v1/new-coins', responseKey: 'projects' },
  user: { endpoint: '/admin/api/v1/users', responseKey: 'users' }
};

const statusLabels: Record<string, string> = {
  active: '启用',
  disabled: '禁用',
  draft: '草稿',
  inactive: '未启用',
  suspended: '暂停'
};

const lifecycleLabels: Record<string, string> = {
  distribution: '派发中',
  listed: '已上市',
  preheat: '预热',
  subscription: '申购中'
};

function field(record: ApiRecord, key: string): string {
  const value = record[key];
  return typeof value === 'number' || typeof value === 'string' ? String(value).trim() : '';
}

function labelForStatus(status: string): string {
  return statusLabels[status] ?? (status ? status : '未知状态');
}

function availability(status: string): Pick<AdminReferenceOption, 'disabled' | 'disabledReason' | 'status' | 'statusLabel'> {
  const normalized = status.trim().toLowerCase() || 'unknown';
  const disabled = normalized !== 'active';
  return {
    disabled,
    disabledReason: disabled ? `当前状态为${labelForStatus(normalized)}，不可用于新操作` : undefined,
    status: normalized,
    statusLabel: labelForStatus(normalized)
  };
}

function mapReference(kind: AdminReferenceKind, record: ApiRecord): AdminReferenceOption | null {
  const id = field(record, 'id');
  if (!id) {
    return null;
  }

  const state = availability(field(record, 'status'));
  if (kind === 'agent') {
    const code = field(record, 'agent_code') || `代理${id}`;
    const level = field(record, 'level') || '1';
    const email = field(record, 'email');
    return {
      ...state,
      code,
      constraint: `层级 L${level}${email ? ` · ${email}` : ''}`,
      label: `${code} · L${level} · ${state.statusLabel}（ID: ${id}）`,
      value: id
    };
  }

  if (kind === 'asset') {
    const symbol = field(record, 'symbol') || `资产${id}`;
    const name = field(record, 'name');
    return {
      ...state,
      code: symbol,
      constraint: `${name || '未配置名称'} · 资产 ID ${id}`,
      label: `${symbol}${name ? ` - ${name}` : ''} · ${state.statusLabel}（ID: ${id}）`,
      value: id
    };
  }

  if (kind === 'marketPair') {
    const symbol = field(record, 'symbol') || `交易对${id}`;
    const marketType = field(record, 'market_type') || '未分类';
    return {
      ...state,
      code: symbol,
      constraint: `市场类型 ${marketType} · 交易对 ID ${id}`,
      label: `${symbol} · ${marketType} · ${state.statusLabel}（ID: ${id}）`,
      value: id
    };
  }

  if (kind === 'newCoinProject') {
    const symbol = field(record, 'symbol') || `新币项目${id}`;
    const lifecycleStatus = field(record, 'lifecycle_status');
    const lifecycleLabel = lifecycleLabels[lifecycleStatus] ?? (lifecycleStatus || '未配置');
    return {
      ...state,
      code: symbol,
      constraint: `生命周期 ${lifecycleLabel} · 项目资产 ID ${field(record, 'asset_id') || '-'}`,
      label: `${symbol} · ${lifecycleLabel} · ${state.statusLabel}（ID: ${id}）`,
      lifecycleStatus,
      value: id
    };
  }

  const email = field(record, 'email');
  const phone = field(record, 'phone');
  const identity = email || phone || `用户${id}`;
  return {
    ...state,
    constraint: `KYC ${field(record, 'kyc_level') || '0'}${email && phone ? ` · ${phone}` : ''}`,
    label: `${identity} · ${state.statusLabel}（ID: ${id}）`,
    value: id
  };
}

export function useAdminReferenceOptions(kind: AdminReferenceKind, enabled = true) {
  const source = referenceSources[kind];
  const query = useSharedAdminOptionQuery<AdminReferenceOption[]>({
    cacheKey: `reference:${kind}:100`,
    empty: [],
    enabled,
    load: async (signal) => {
      const { rows } = await listAdminResource(source.endpoint, source.responseKey, { limit: 100 }, { signal });
      return rows.map((record) => mapReference(kind, record)).filter((option): option is AdminReferenceOption => option !== null);
    }
  });
  return { error: query.error ? '引用数据加载失败，请刷新后重试' : null, loading: query.loading, options: query.data };
}

export function isReferenceSelectable(options: AdminReferenceOption[], value: string): boolean {
  return options.some((option) => option.value === value && !option.disabled);
}

export function AdminReferenceSelect({
  error,
  label,
  loading,
  onChange,
  options,
  placeholder,
  value
}: {
  error?: string | null;
  label: string;
  loading: boolean;
  onChange: (value: string) => void;
  options: AdminReferenceOption[];
  placeholder: string;
  value: string;
}) {
  const selected = useMemo(() => options.find((option) => option.value === value), [options, value]);

  return (
    <label className="admin-reference-field">
      {label}
      <AdminSelect
        ariaLabel={label}
        disabled={loading}
        filter
        loading={loading}
        onChange={onChange}
        optionList={options.map((option) => ({ disabled: option.disabled, label: option.label, value: option.value }))}
        placeholder={loading ? '加载引用中...' : placeholder}
        showClear
        value={value}
      />
      {selected ? (
        <span className={`admin-reference-field__meta${selected.disabled ? ' is-disabled' : ''}`}>
          <span>{selected.constraint}</span>
          <span>{selected.disabledReason ?? `状态：${selected.statusLabel}`}</span>
        </span>
      ) : null}
      {error ? <Text className="admin-reference-field__error" type="danger">{error}</Text> : null}
    </label>
  );
}
