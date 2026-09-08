import { canonicalDecimalText, compareDecimalText, decimalFitsPrecision, isNonNegativeDecimalText, isPositiveDecimalText } from '../../../../shared/decimal';
import type { DefaultMarketDraft, DefaultMarketResponse } from './types';

export function defaultMarketDraft(value: DefaultMarketResponse): DefaultMarketDraft {
  const { mode = 'independent', follow = null, ...parameters } = value.config;
  return { ...parameters, mode, reference_pair_id: follow ? String(follow.reference_pair_id) : '',
    follow_multiplier: follow?.multiplier ?? '1', follow_max_move_ratio: follow?.max_move_ratio ?? '0.05',
    follow_stale_after_seconds: String(follow?.stale_after_seconds ?? 60), enabled: value.enabled, initial_price: value.initial_price ?? '',
    price_min: value.config.price_min ?? '', price_max: value.config.price_max ?? '', depth_levels: String(value.config.depth_levels) };
}

/** 金额只去除首尾空白；不转 Number、不自动舍入，也不隐式启用或解除全部暂停。 */
export function defaultMarketPayload(draft: DefaultMarketDraft, version: number) {
  const optional = (value: string) => value.trim() || null;
  return {
    expected_version: version, enabled: draft.enabled, initial_price: optional(draft.initial_price),
    config: {
      mode: draft.mode,
      follow: draft.mode === 'follow' ? {
        reference_pair_id: Number(draft.reference_pair_id), multiplier: draft.follow_multiplier.trim(),
        max_move_ratio: draft.follow_max_move_ratio.trim(), stale_after_seconds: Number(draft.follow_stale_after_seconds)
      } : null,
      volatility: draft.volatility.trim(), mean_reversion: draft.mean_reversion.trim(),
      price_min: optional(draft.price_min), price_max: optional(draft.price_max),
      volume_min: draft.volume_min.trim(), volume_max: draft.volume_max.trim(),
      wick_strength: draft.wick_strength.trim(), depth_levels: Number(draft.depth_levels)
    }
  };
}

function exact(value: string, precision: number, positive: boolean): boolean {
  const normalized = canonicalDecimalText(value);
  return normalized !== null && (positive ? isPositiveDecimalText(value) : isNonNegativeDecimalText(value))
    && normalized.split('.')[0].replace('-', '').length <= 20 && decimalFitsPrecision(value, precision);
}

export function validateDefaultMarketDraft(draft: DefaultMarketDraft, pricePrecision: number, qtyPrecision: number): string {
  if (draft.mode !== 'independent' && draft.mode !== 'follow') return '请选择有效的生成模式。';
  if (draft.mode === 'follow') {
    if (!/^\d+$/.test(draft.reference_pair_id) || !Number.isSafeInteger(Number(draft.reference_pair_id)) || Number(draft.reference_pair_id) <= 0) return '请明确选择有效的参考交易对，不会自动选择 BTC。';
    if (!exact(draft.follow_multiplier, 18, true) || (compareDecimalText(draft.follow_multiplier, '3') ?? 1) > 0) return '跟随倍率须大于 0 且不超过 3，最多 18 位小数。';
    if (!exact(draft.follow_max_move_ratio, 18, true) || (compareDecimalText(draft.follow_max_move_ratio, '1') ?? 1) > 0) return '每分钟最大变动须大于 0 且不超过 1，最多 18 位小数。';
    if (!/^\d{2,3}$/.test(draft.follow_stale_after_seconds.trim()) || Number(draft.follow_stale_after_seconds) < 15 || Number(draft.follow_stale_after_seconds) > 300) return '参考过期时间须为 15 至 300 秒的整数。';
  }
  if (![pricePrecision, qtyPrecision].every((value) => Number.isInteger(value) && value >= 0 && value <= 18)) return '交易对精度缺失或无效，请刷新交易对列表。';
  for (const [key, label] of [['volatility', '波动率'], ['mean_reversion', '均值回归强度'], ['wick_strength', '影线强度']] as const) {
    if (!exact(draft[key], 18, false) || (compareDecimalText(draft[key], '1') ?? 1) > 0) return `${label}须为 0 至 1 的小数比例，最多 18 位小数。`;
  }
  for (const [key, label] of [['initial_price', '初始价格'], ['price_min', '价格下限'], ['price_max', '价格上限']] as const) {
    if (draft[key].trim() && !exact(draft[key], pricePrecision, true)) return `${label}须为正数，最多 ${pricePrecision} 位小数、20 位整数，不会自动舍入。`;
  }
  for (const [key, label] of [['volume_min', '分钟成交量下限'], ['volume_max', '分钟成交量上限']] as const) {
    if (!exact(draft[key], qtyPrecision, false)) return `${label}须为非负数，最多 ${qtyPrecision} 位小数、20 位整数，不会自动舍入。`;
  }
  if (draft.price_min.trim() && draft.price_max.trim() && compareDecimalText(draft.price_min, draft.price_max) === 1) return '价格下限须小于等于价格上限。';
  if (compareDecimalText(draft.volume_min, draft.volume_max) === 1) return '分钟成交量下限须小于等于上限。';
  if (!/^\d{1,2}$/.test(draft.depth_levels.trim()) || Number(draft.depth_levels) < 1 || Number(draft.depth_levels) > 20) return '盘口档数须为 1 至 20 的整数。';
  return '';
}
