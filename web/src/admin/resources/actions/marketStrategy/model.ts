import type { ApiRecord } from '../../../../api/types';
import type { SemiSelectOption } from '../../../../shared/SemiFormControls';
import type { MarketStrategyNodeDraft } from '../../../components/MarketStrategyNodeEditor';
import type { MarketPairOption } from '../shared';
import { applyPercentDecimalText, compareDecimalText, isNonNegativeDecimalText, isPositiveDecimalText } from '../../../../shared/decimal';
import { optionalString, recordString, requiredString } from '../shared';
import type {
  MarketStrategyGeneratorRecord,
  MarketStrategyNodeRecord,
  MarketStrategyPreset,
  MarketStrategyValues
} from './types';

export const scenarioOptions: SemiSelectOption[] = [
  { value: 'custom_path', label: '自定义路径' },
  { value: 'trend_up', label: '稳步上涨' },
  { value: 'trend_down', label: '缓慢下行' },
  { value: 'range', label: '区间震荡' },
  { value: 'high_volatility', label: '高波动' },
  { value: 'crash_recovery', label: '急跌修复' },
  { value: 'pump_then_dump', label: '拉升回落' }
];

export const seedModeOptions: SemiSelectOption[] = [
  { value: 'auto', label: '自动 Seed' },
  { value: 'fixed', label: '固定 Seed' }
];

export const volumeShapeOptions: SemiSelectOption[] = [
  { value: 'uniform', label: '均匀分布' },
  { value: 'trend', label: '随时间递增' },
  { value: 'bell', label: '中段放量' },
  { value: 'end_spike', label: '尾段放量' }
];

const strategyTypeOptions: SemiSelectOption[] = [
  { value: 'price_path', label: '价格路径（OHLCV）' }
];

const marketStrategyPairTypes = new Set(['internal', 'strategy']);

export const initialMarketStrategy: MarketStrategyValues = {
  pairId: '',
  strategyType: 'price_path',
  startPrice: '',
  targetPrice: '',
  startTime: '',
  endTime: '',
  volatility: '0.01',
  volumeMin: '0',
  volumeMax: '0',
  nodes: [],
  status: 'draft',
  scenario: 'custom_path',
  seedMode: 'auto',
  seed: '',
  regenerateSeed: false,
  meanReversionStrength: '0.55',
  noiseScale: '1',
  wickScale: '0.75',
  volumeShape: 'uniform'
};

export function strategyTypeOptionsWithCurrent(value: string): SemiSelectOption[] {
  const current = value.trim();
  if (!current || strategyTypeOptions.some((option) => option.value === current)) {
    return strategyTypeOptions;
  }
  return [{ value: current, label: `历史策略类型（${current}）` }, ...strategyTypeOptions];
}

export function eligibleMarketStrategyPairs(options: MarketPairOption[]): MarketPairOption[] {
  return options.filter((option) => marketStrategyPairTypes.has(option.marketType));
}

export function inputDateTimeFromUnixMillis(value: unknown): string {
  const timestamp = Number(value);
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '';
  const date = new Date(timestamp);
  const offsetMillis = date.getTimezoneOffset() * 60_000;
  return new Date(timestamp - offsetMillis).toISOString().slice(0, 16);
}

function inputDateTimeFromUnknown(value: unknown): string {
  if (typeof value === 'string' && value.includes('T') && Number.isFinite(Date.parse(value))) {
    return value.slice(0, 16);
  }
  return inputDateTimeFromUnixMillis(value);
}

function unixMillisFromInputDateTime(value: string, label: string): number {
  const timestamp = new Date(value).getTime();
  if (!value.trim() || !Number.isFinite(timestamp) || timestamp <= 0) {
    throw new Error(`${label}必须为有效日期时间`);
  }
  return timestamp;
}

function marketStrategyNodesFromRecord(record: ApiRecord): MarketStrategyNodeDraft[] {
  if (!Array.isArray(record.nodes)) return [];
  return (record.nodes as MarketStrategyNodeRecord[])
    .slice()
    .sort((left, right) => Number(left.sequence_no ?? 0) - Number(right.sequence_no ?? 0))
    .map((node, index) => ({
      clientId: `strategy-record-node-${Number(node.sequence_no ?? index)}-${index}`,
      targetTime: inputDateTimeFromUnknown(node.target_time),
      targetType: String(node.target_type ?? 'absolute_price') as MarketStrategyNodeDraft['targetType'],
      targetValue: String(node.target_value ?? ''),
      executionMode: String(node.execution_mode ?? 'hard') as MarketStrategyNodeDraft['executionMode'],
      tolerance: String(node.tolerance ?? '0'),
      volatility: String(node.volatility ?? '0'),
      volumeMin: node.volume_min == null ? '' : String(node.volume_min),
      volumeMax: node.volume_max == null ? '' : String(node.volume_max)
    }));
}

function nestedRecord(value: unknown): ApiRecord {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as ApiRecord) : {};
}

export function marketStrategyFromRecord(record: ApiRecord): MarketStrategyValues {
  const generator = nestedRecord(record.generator) as MarketStrategyGeneratorRecord;
  return {
    pairId: recordString(record, 'pair_id'),
    strategyType: recordString(record, 'strategy_type') || 'price_path',
    startPrice: recordString(record, 'start_price'),
    targetPrice: recordString(record, 'target_price'),
    startTime: inputDateTimeFromUnknown(record.start_time),
    endTime: inputDateTimeFromUnknown(record.end_time),
    volatility: recordString(record, 'volatility') || '0',
    volumeMin: recordString(record, 'volume_min') || '0',
    volumeMax: recordString(record, 'volume_max') || '0',
    nodes: marketStrategyNodesFromRecord(record),
    status: recordString(record, 'status') || 'draft',
    scenario: String(generator.scenario ?? 'custom_path'),
    seedMode: String(generator.seed_mode ?? 'auto'),
    seed: String(generator.seed ?? ''),
    regenerateSeed: false,
    meanReversionStrength: String(generator.mean_reversion_strength ?? '0.55'),
    noiseScale: String(generator.noise_scale ?? '1'),
    wickScale: String(generator.wick_scale ?? '0.75'),
    volumeShape: String(generator.volume_shape ?? 'uniform')
  };
}

function isDecimalInRange(value: string, minimum: string, maximum?: string): boolean {
  const minimumComparison = compareDecimalText(value, minimum);
  const maximumComparison = maximum === undefined ? null : compareDecimalText(value, maximum);
  return minimumComparison !== null && minimumComparison >= 0 && (maximum === undefined || (maximumComparison !== null && maximumComparison <= 0));
}

function parseInputDateTime(value: string): number | null {
  if (!value.trim()) return null;
  const timestamp = new Date(value).getTime();
  return Number.isFinite(timestamp) && timestamp > 0 ? timestamp : null;
}

function isMarketStrategyNodeSubmittable(node: MarketStrategyNodeDraft, targetTime: number): boolean {
  const volumeMin = node.volumeMin.trim();
  const volumeMax = node.volumeMax.trim();
  return Boolean(
    Number.isFinite(targetTime) &&
      node.targetType &&
      node.targetValue.trim() &&
      node.executionMode &&
      isDecimalInRange(node.tolerance, '0') &&
      isDecimalInRange(node.volatility, '0') &&
      ((!volumeMin && !volumeMax) ||
        (isDecimalInRange(volumeMin, '0') &&
          isDecimalInRange(volumeMax, '0') &&
          (compareDecimalText(volumeMax, volumeMin) ?? -1) >= 0))
  );
}

export function isMarketStrategySubmittable(values: MarketStrategyValues, includePairId: boolean): boolean {
  const startTime = parseInputDateTime(values.startTime);
  const endTime = parseInputDateTime(values.endTime);
  if (
    startTime === null ||
    endTime === null ||
    endTime <= startTime ||
    startTime % 60_000 !== 0 ||
    endTime % 60_000 !== 0
  ) {
    return false;
  }

  let previousNodeTime = startTime;
  for (const node of values.nodes) {
    const targetTime = parseInputDateTime(node.targetTime);
    if (
      targetTime === null ||
      targetTime % 60_000 !== 0 ||
      targetTime <= startTime ||
      targetTime >= endTime ||
      targetTime <= previousNodeTime ||
      !isMarketStrategyNodeSubmittable(node, targetTime)
    ) {
      return false;
    }
    previousNodeTime = targetTime;
  }

  const fixedSeedValid =
    values.seedMode !== 'fixed' ||
    (values.seed.trim().length > 0 && [...values.seed.trim()].length <= 128);
  return Boolean(
    (!includePairId || values.pairId.trim()) &&
      values.strategyType.trim() &&
      isPositiveDecimalText(values.startPrice) &&
      isPositiveDecimalText(values.targetPrice) &&
      isNonNegativeDecimalText(values.volatility) &&
      isNonNegativeDecimalText(values.volumeMin) &&
      (compareDecimalText(values.volumeMax, values.volumeMin) ?? -1) >= 0 &&
      scenarioOptions.some((option) => option.value === values.scenario) &&
      seedModeOptions.some((option) => option.value === values.seedMode) &&
      fixedSeedValid &&
      isDecimalInRange(values.meanReversionStrength, '0', '2') &&
      isDecimalInRange(values.noiseScale, '0', '5') &&
      isDecimalInRange(values.wickScale, '0', '5') &&
      volumeShapeOptions.some((option) => option.value === values.volumeShape)
  );
}

function marketStrategyNodePayload(node: MarketStrategyNodeDraft, index: number) {
  return {
    target_time: unixMillisFromInputDateTime(node.targetTime, `节点${index + 1}目标时间`),
    target_type: requiredString(node.targetType, `节点${index + 1}目标类型`),
    target_value: requiredString(node.targetValue, `节点${index + 1}目标值`),
    execution_mode: requiredString(node.executionMode, `节点${index + 1}执行模式`),
    tolerance: requiredString(node.tolerance, `节点${index + 1}容差`),
    volatility: requiredString(node.volatility, `节点${index + 1}局部波动率`),
    volume_min: optionalString(node.volumeMin) ?? null,
    volume_max: optionalString(node.volumeMax) ?? null
  };
}

function marketStrategyGeneratorPayload(values: MarketStrategyValues) {
  return {
    scenario: requiredString(values.scenario, '行情场景'),
    seed_mode: requiredString(values.seedMode, 'Seed 模式'),
    seed: values.seedMode === 'fixed' ? requiredString(values.seed, '固定 Seed') : null,
    regenerate_seed: values.seedMode === 'auto' && values.regenerateSeed,
    mean_reversion_strength: requiredString(values.meanReversionStrength, '均值回归强度'),
    noise_scale: requiredString(values.noiseScale, '噪声强度'),
    wick_scale: requiredString(values.wickScale, '影线强度'),
    volume_shape: requiredString(values.volumeShape, '成交量形态')
  };
}

export function marketStrategyBasePayload(values: MarketStrategyValues) {
  return {
    strategy_type: requiredString(values.strategyType, '策略类型'),
    start_price: requiredString(values.startPrice, '起始价'),
    target_price: requiredString(values.targetPrice, '目标价'),
    start_time: unixMillisFromInputDateTime(values.startTime, '开始时间'),
    end_time: unixMillisFromInputDateTime(values.endTime, '结束时间'),
    volatility: requiredString(values.volatility, '波动率'),
    volume_min: requiredString(values.volumeMin, '最小成交量'),
    volume_max: requiredString(values.volumeMax, '最大成交量'),
    nodes: values.nodes.map(marketStrategyNodePayload),
    generator: marketStrategyGeneratorPayload(values)
  };
}

function targetPriceFromPreset(startPrice: string, changePercent: string): string | null {
  if (!isPositiveDecimalText(startPrice)) return null;
  return applyPercentDecimalText(startPrice, changePercent);
}

function presetNodes(
  values: MarketStrategyValues,
  preset: MarketStrategyPreset
): MarketStrategyNodeDraft[] | null {
  const start = parseInputDateTime(values.startTime);
  const end = parseInputDateTime(values.endTime);
  if (start === null || end === null || end <= start) return null;
  const totalMinutes = Math.floor((end - start) / 60_000);
  if (totalMinutes <= 1) return preset.nodes.length === 0 ? [] : null;

  const occupied = new Set<number>();
  const nodes: MarketStrategyNodeDraft[] = [];
  preset.nodes.forEach((node, index) => {
    const minuteOffset = Math.max(
      1,
      Math.min(totalMinutes - 1, Math.round((totalMinutes * Number(node.progress_percent)) / 100))
    );
    if (occupied.has(minuteOffset)) return;
    occupied.add(minuteOffset);
    nodes.push({
      clientId: `strategy-preset-${preset.code}-${minuteOffset}-${index}`,
      targetTime: inputDateTimeFromUnixMillis(start + minuteOffset * 60_000),
      targetType: node.target_type as MarketStrategyNodeDraft['targetType'],
      targetValue: String(node.target_value),
      executionMode: node.execution_mode as MarketStrategyNodeDraft['executionMode'],
      tolerance: String(node.tolerance),
      volatility: String(node.volatility),
      volumeMin: node.volume_min == null ? '' : String(node.volume_min),
      volumeMax: node.volume_max == null ? '' : String(node.volume_max)
    });
  });
  return nodes.sort((left, right) => left.targetTime.localeCompare(right.targetTime));
}

export function applyPreset(
  values: MarketStrategyValues,
  preset: MarketStrategyPreset
): MarketStrategyValues | null {
  const targetPrice = targetPriceFromPreset(values.startPrice, String(preset.target_price_change_percent));
  const nodes = presetNodes(values, preset);
  if (targetPrice === null || nodes === null) return null;
  return {
    ...values,
    scenario: String(preset.generator.scenario ?? preset.code),
    seedMode: String(preset.generator.seed_mode ?? 'auto'),
    seed: '',
    regenerateSeed: false,
    meanReversionStrength: String(preset.generator.mean_reversion_strength ?? '0.55'),
    noiseScale: String(preset.generator.noise_scale ?? '1'),
    wickScale: String(preset.generator.wick_scale ?? '0.75'),
    volumeShape: String(preset.generator.volume_shape ?? 'uniform'),
    targetPrice,
    nodes
  };
}

export function formatPreviewTime(value: number): string {
  const date = new Date(Number(value));
  return Number.isFinite(date.getTime()) ? date.toLocaleString('zh-CN', { hour12: false }) : '--';
}

export function nextMarketStrategyStatus(status: string): 'active' | 'disabled' {
  return status === 'active' ? 'disabled' : 'active';
}
