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
  if (!Number.isFinite(date.getTime())) return '';
  const offsetMillis = date.getTimezoneOffset() * 60_000;
  const local = new Date(timestamp - offsetMillis).toISOString();
  // 保留异常历史秒数，交由分钟对齐校验提示，不在回填时静默改写配置。
  return date.getSeconds() || date.getMilliseconds() ? local.slice(0, -1) : local.slice(0, 16);
}

function inputDateTimeFromUnknown(value: unknown): string {
  if (typeof value === 'string' && value.includes('T') && Number.isFinite(Date.parse(value))) {
    return inputDateTimeFromUnixMillis(Date.parse(value));
  }
  return inputDateTimeFromUnixMillis(value);
}

function unixMillisFromInputDateTime(value: string, label: string): number {
  const timestamp = parseInputDateTime(value);
  if (timestamp === null) {
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
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?$/.exec(value.trim());
  if (!match) return null;
  const [, year, month, day, hour, minute, second = '0', millis = '0'] = match;
  const parts = [Number(year), Number(month) - 1, Number(day), Number(hour), Number(minute), Number(second), Number(millis.padEnd(3, '0'))];
  const date = new Date(0);
  date.setFullYear(parts[0], parts[1], parts[2]);
  date.setHours(parts[3], parts[4], parts[5], parts[6]);
  const actual = [date.getFullYear(), date.getMonth(), date.getDate(), date.getHours(), date.getMinutes(), date.getSeconds(), date.getMilliseconds()];
  // 拒绝日期滚动和本地 DST 空洞；回填与提交使用同一个本地日历语义。
  return date.getTime() > 0 && parts.every((part, index) => part === actual[index]) ? date.getTime() : null;
}

function marketStrategyNodeValidationError(node: MarketStrategyNodeDraft, index: number): string | null {
  const label = `节点${index + 1}`;
  if (!['absolute_price', 'percent_from_start', 'percent_from_previous'].includes(node.targetType)) {
    return `${label}目标类型无效`;
  }
  if (!['hard', 'soft', 'range'].includes(node.executionMode)) return `${label}执行模式无效`;
  // 正的起始价与前序目标乘以正比例仍为正，无需把链式金额转成浮点数。
  if (node.targetType === 'absolute_price') {
    if (!isPositiveDecimalText(node.targetValue)) return `${label}目标值必须为大于 0 的价格`;
  } else if (compareDecimalText(node.targetValue, '-100') !== 1) {
    return `${label}目标值必须为大于 -100 的百分比`;
  }
  if (!isNonNegativeDecimalText(node.tolerance)) return `${label}容差必须为非负数`;
  if (!isNonNegativeDecimalText(node.volatility)) return `${label}局部波动率必须为非负数`;
  const minimum = node.volumeMin.trim();
  const maximum = node.volumeMax.trim();
  if (!minimum && !maximum) return null;
  if (!minimum || !maximum) return `${label}最小和最大成交量须同时填写或同时留空`;
  if (!isNonNegativeDecimalText(minimum) || !isNonNegativeDecimalText(maximum)) return `${label}成交量必须为非负数`;
  if ((compareDecimalText(maximum, minimum) ?? -1) < 0) return `${label}最大成交量不得小于最小成交量`;
  return null;
}

/** 预览、保存按钮和请求序列化共用同一校验，返回首个可操作的中文错误。 */
export function marketStrategyValidationError(values: MarketStrategyValues, includePairId: boolean): string | null {
  if (includePairId && (!values.pairId.trim() || !Number.isSafeInteger(Number(values.pairId)) || Number(values.pairId) <= 0)) {
    return '请选择有效的交易对';
  }
  if (!values.strategyType.trim()) return '请选择策略类型';
  if (!isPositiveDecimalText(values.startPrice)) return '起始价必须为大于 0 的价格';
  if (!isPositiveDecimalText(values.targetPrice)) return '目标价必须为大于 0 的价格';
  const startTime = parseInputDateTime(values.startTime);
  const endTime = parseInputDateTime(values.endTime);
  if (startTime === null) return '开始时间必须为有效日期时间';
  if (endTime === null) return '结束时间必须为有效日期时间';
  if (endTime <= startTime) return '结束时间必须晚于开始时间';
  if (startTime % 60_000 !== 0 || endTime % 60_000 !== 0) return '开始和结束时间必须对齐到整分钟';
  if (!isNonNegativeDecimalText(values.volatility)) return '波动率必须为非负数';
  if (!isNonNegativeDecimalText(values.volumeMin) || !isNonNegativeDecimalText(values.volumeMax)) return '成交量必须为非负数';
  if ((compareDecimalText(values.volumeMax, values.volumeMin) ?? -1) < 0) return '最大成交量不得小于最小成交量';

  let previousNodeTime = startTime;
  for (const [index, node] of values.nodes.entries()) {
    const targetTime = parseInputDateTime(node.targetTime);
    if (targetTime === null) return `节点${index + 1}目标时间必须为有效日期时间`;
    if (targetTime % 60_000 !== 0) return `节点${index + 1}目标时间必须对齐到整分钟`;
    if (targetTime <= startTime || targetTime >= endTime) return `节点${index + 1}目标时间须在开始与结束时间之间，不含边界`;
    if (targetTime <= previousNodeTime) return `节点${index + 1}目标时间必须晚于上一节点`;
    const error = marketStrategyNodeValidationError(node, index);
    if (error) return error;
    previousNodeTime = targetTime;
  }

  if (!scenarioOptions.some((option) => option.value === values.scenario)) return '请选择有效的行情场景';
  if (!seedModeOptions.some((option) => option.value === values.seedMode)) return '请选择有效的 Seed 模式';
  if (values.seedMode === 'fixed' && (!values.seed.trim() || [...values.seed.trim()].length > 128)) return '固定 Seed 须为 1～128 个字符';
  if (!isDecimalInRange(values.meanReversionStrength, '0', '2')) return '均值回归强度须在 0～2 之间';
  if (!isDecimalInRange(values.noiseScale, '0', '5')) return '噪声强度须在 0～5 之间';
  if (!isDecimalInRange(values.wickScale, '0', '5')) return '影线强度须在 0～5 之间';
  if (!volumeShapeOptions.some((option) => option.value === values.volumeShape)) return '请选择有效的成交量形态';
  if (includePairId && !['draft', 'active', 'paused', 'disabled'].includes(values.status)) return '请选择有效的策略状态';
  return null;
}

export function isMarketStrategySubmittable(values: MarketStrategyValues, includePairId: boolean): boolean {
  return marketStrategyValidationError(values, includePairId) === null;
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
  const error = marketStrategyValidationError(values, false);
  if (error) throw new Error(error);
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
  if (start === null || end === null || end <= start || start % 60_000 !== 0 || end % 60_000 !== 0) return null;
  const totalMinutes = Math.floor((end - start) / 60_000);
  if (totalMinutes <= 1) return preset.nodes.length === 0 ? [] : null;

  let previousMinute = 0;
  const nodes: MarketStrategyNodeDraft[] = [];
  for (const [index, node] of preset.nodes.entries()) {
    if (!Number.isFinite(node.progress_percent) || node.progress_percent <= 0 || node.progress_percent >= 100) return null;
    const minuteOffset = Math.max(
      1,
      Math.min(totalMinutes - 1, Math.round((totalMinutes * Number(node.progress_percent)) / 100))
    );
    // 整分钟舍入发生碰撞时拒绝整个预设，不丢节点、不改变相对前一节点的语义。
    if (minuteOffset <= previousMinute) return null;
    previousMinute = minuteOffset;
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
  }
  return nodes;
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
