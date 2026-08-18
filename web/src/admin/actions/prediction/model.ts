import type { SemiSelectOption } from '../../../shared/SemiFormControls';
import type {
  PredictionAssetConfig,
  PredictionAssetDraft,
  PredictionSettings,
  PredictionSettingsValues
} from './types';

export const settlementModeOptions: SemiSelectOption[] = [
  { value: 'manual_confirm', label: '外部结果 + 人工确认' },
  { value: 'auto', label: '外部结果 + 自动结算' }
];

export const invalidRefundPolicyOptions: SemiSelectOption[] = [
  { value: 'refund_stake_and_fee', label: '退本金 + 退手续费' },
  { value: 'refund_stake_only', label: '只退本金' },
  { value: 'manual', label: '无效结算时人工选择' }
];

export const triggerTypeLabels: Record<string, string> = {
  manual: '手动触发',
  scheduled: '定时同步',
  system: '系统触发'
};

export const syncStatusMeta: Record<
  string,
  { color: 'green' | 'grey' | 'light-blue' | 'orange' | 'red'; label: string }
> = {
  failed: { color: 'red', label: '失败' },
  running: { color: 'light-blue', label: '同步中' },
  skipped: { color: 'grey', label: '已跳过' },
  success: { color: 'green', label: '成功' },
  pending: { color: 'orange', label: '待执行' }
};

export function settingsToValues(settings: PredictionSettings): PredictionSettingsValues {
  return {
    syncEnabled: settings.sync_enabled,
    syncIntervalSeconds: String(settings.sync_interval_seconds),
    syncTags: settings.sync_tags.join('\n'),
    allowedAssetIds: settings.allowed_asset_ids.map(String),
    defaultFeeRate: String(settings.default_fee_rate ?? '0'),
    defaultSettlementMode: settings.default_settlement_mode,
    defaultInvalidRefundPolicy: settings.default_invalid_refund_policy,
    quoteTtlSeconds: String(settings.quote_ttl_seconds)
  };
}

export function assetDraftsFromConfigs(
  configs: PredictionAssetConfig[]
): Record<string, PredictionAssetDraft> {
  return Object.fromEntries(
    configs.map((asset) => [
      String(asset.asset_id),
      { enabled: asset.enabled, maxPayoutAmount: String(asset.max_payout_amount ?? '0') }
    ])
  );
}

function parseTags(value: string): string[] {
  return value
    .split(/[\n,，]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function positiveInteger(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(`${label}必须为正整数`);
  return parsed;
}

function nonNegativeAmount(value: string, label: string): string {
  const trimmed = value.trim();
  if (!trimmed || Number(trimmed) < 0 || Number.isNaN(Number(trimmed))) {
    throw new Error(`${label}必须为非负数字`);
  }
  return trimmed;
}

export function settingsPayload(
  values: PredictionSettingsValues,
  revision: number,
  reason: string
) {
  return {
    sync_enabled: values.syncEnabled,
    sync_interval_seconds: positiveInteger(values.syncIntervalSeconds, '同步间隔'),
    sync_tags: parseTags(values.syncTags),
    allowed_asset_ids: values.allowedAssetIds.map(Number),
    default_fee_rate: nonNegativeAmount(values.defaultFeeRate, '默认手续费率'),
    default_settlement_mode: values.defaultSettlementMode,
    default_invalid_refund_policy: values.defaultInvalidRefundPolicy,
    quote_ttl_seconds: positiveInteger(values.quoteTtlSeconds, '报价有效秒数'),
    revision,
    reason
  };
}

export function assetConfigPayload(
  asset: PredictionAssetConfig,
  draft: PredictionAssetDraft,
  reason: string
) {
  return {
    asset_id: asset.asset_id,
    enabled: draft.enabled,
    max_payout_amount: nonNegativeAmount(draft.maxPayoutAmount, '最大赔付'),
    revision: asset.revision,
    reason
  };
}

export function optionLabel(options: SemiSelectOption[], value?: string | null): string {
  if (!value) return '-';
  return options.find((option) => option.value === value)?.label ?? value;
}

export function triggerTypeLabel(value?: string | null): string {
  if (!value) return '-';
  return triggerTypeLabels[value] ?? value;
}

export function joinText(items: string[]): string {
  return items.length ? items.join('、') : '-';
}
