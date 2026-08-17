import { Button, Card, SideSheet, Space, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useMemo, useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import {
  AdminCheckbox,
  AdminModalTriggerButton,
  AdminSelect,
  AdminTextInput,
  type SemiSelectOption
} from '../../../shared/SemiFormControls';
import {
  MarketStrategyNodeEditor,
  createMarketStrategyNodeDraft,
  type MarketStrategyNodeDraft
} from '../../components/MarketStrategyNodeEditor';
import { MarketStrategyRecoverySheet } from '../../components/MarketStrategyRecoverySheet';
import { MarketStrategyVersionSheet } from '../../components/MarketStrategyVersionSheet';
import {
  type RowActionHelpers,
  createModalProps,
  errorMessage,
  openRecordDetail,
  optionalString,
  recordString,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  toggleActionText
} from './shared';

type MarketStrategyValues = {
  endTime: string;
  meanReversionStrength: string;
  nodes: MarketStrategyNodeDraft[];
  noiseScale: string;
  pairId: string;
  regenerateSeed: boolean;
  scenario: string;
  seed: string;
  seedMode: string;
  startPrice: string;
  startTime: string;
  status: string;
  strategyType: string;
  targetPrice: string;
  volatility: string;
  volumeMax: string;
  volumeMin: string;
  volumeShape: string;
  wickScale: string;
};

type MarketStrategyNodeRecord = {
  execution_mode?: unknown;
  sequence_no?: unknown;
  target_time?: unknown;
  target_type?: unknown;
  target_value?: unknown;
  tolerance?: unknown;
  volatility?: unknown;
  volume_max?: unknown;
  volume_min?: unknown;
};

type MarketStrategyGeneratorRecord = {
  mean_reversion_strength?: unknown;
  noise_scale?: unknown;
  scenario?: unknown;
  seed?: unknown;
  seed_mode?: unknown;
  volume_shape?: unknown;
  wick_scale?: unknown;
};

type MarketStrategyPresetNode = {
  execution_mode: string;
  progress_percent: number;
  target_type: string;
  target_value: string;
  tolerance: string;
  volatility: string;
  volume_max: string | null;
  volume_min: string | null;
};

type MarketStrategyPreset = {
  code: string;
  description: string;
  generator: Omit<MarketStrategyGeneratorRecord, 'seed'>;
  name: string;
  nodes: MarketStrategyPresetNode[];
  target_price_change_percent: string;
};

type MarketStrategyPresetsResponse = {
  presets: MarketStrategyPreset[];
};

type MarketStrategyPreviewSample = {
  close: string;
  high: string;
  low: string;
  open: string;
  open_time: number;
  volume: string;
};

type MarketStrategyPreviewResponse = {
  one_minute_count: number;
  preview_seed: string;
  preview_version: number;
  sample_count: number;
  samples: MarketStrategyPreviewSample[];
};

const scenarioOptions: SemiSelectOption[] = [
  { value: 'custom_path', label: '自定义路径' },
  { value: 'trend_up', label: '稳步上涨' },
  { value: 'trend_down', label: '缓慢下行' },
  { value: 'range', label: '区间震荡' },
  { value: 'high_volatility', label: '高波动' },
  { value: 'crash_recovery', label: '急跌修复' },
  { value: 'pump_then_dump', label: '拉升回落' }
];

const seedModeOptions: SemiSelectOption[] = [
  { value: 'auto', label: '自动 Seed' },
  { value: 'fixed', label: '固定 Seed' }
];

const volumeShapeOptions: SemiSelectOption[] = [
  { value: 'uniform', label: '均匀分布' },
  { value: 'trend', label: '随时间递增' },
  { value: 'bell', label: '中段放量' },
  { value: 'end_spike', label: '尾段放量' }
];

const initialMarketStrategy: MarketStrategyValues = {
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

function inputDateTimeFromUnixMillis(value: unknown): string {
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
    .map((node) => ({
      ...createMarketStrategyNodeDraft(),
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

function marketStrategyFromRecord(record: ApiRecord): MarketStrategyValues {
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

function isDecimalInRange(value: string, minimum: number, maximum: number): boolean {
  const parsed = Number(value);
  return value.trim().length > 0 && Number.isFinite(parsed) && parsed >= minimum && parsed <= maximum;
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
      isDecimalInRange(node.tolerance, 0, Number.MAX_VALUE) &&
      isDecimalInRange(node.volatility, 0, Number.MAX_VALUE) &&
      ((!volumeMin && !volumeMax) ||
        (isDecimalInRange(volumeMin, 0, Number.MAX_VALUE) &&
          isDecimalInRange(volumeMax, 0, Number.MAX_VALUE) &&
          Number(volumeMax) >= Number(volumeMin)))
  );
}

function isMarketStrategySubmittable(values: MarketStrategyValues, includePairId: boolean): boolean {
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

  const fixedSeedValid = values.seedMode !== 'fixed' || (values.seed.trim().length > 0 && [...values.seed.trim()].length <= 128);
  return Boolean(
    (!includePairId || values.pairId.trim()) &&
      values.strategyType.trim() &&
      Number(values.startPrice) > 0 &&
      Number(values.targetPrice) > 0 &&
      isDecimalInRange(values.volatility, 0, Number.MAX_VALUE) &&
      isDecimalInRange(values.volumeMin, 0, Number.MAX_VALUE) &&
      isDecimalInRange(values.volumeMax, Number(values.volumeMin), Number.MAX_VALUE) &&
      scenarioOptions.some((option) => option.value === values.scenario) &&
      seedModeOptions.some((option) => option.value === values.seedMode) &&
      fixedSeedValid &&
      isDecimalInRange(values.meanReversionStrength, 0, 2) &&
      isDecimalInRange(values.noiseScale, 0, 5) &&
      isDecimalInRange(values.wickScale, 0, 5) &&
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

function marketStrategyBasePayload(values: MarketStrategyValues) {
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
  const start = Number(startPrice);
  const percent = Number(changePercent);
  if (!Number.isFinite(start) || start <= 0 || !Number.isFinite(percent)) return null;
  const decimalPlaces = Math.min(18, Math.max(8, startPrice.split('.')[1]?.length ?? 0));
  return (start * (1 + percent / 100)).toFixed(decimalPlaces).replace(/(?:\.0+|(?<=[0-9])0+)$/, '').replace(/\.$/, '');
}

function presetNodes(values: MarketStrategyValues, preset: MarketStrategyPreset): MarketStrategyNodeDraft[] | null {
  const start = parseInputDateTime(values.startTime);
  const end = parseInputDateTime(values.endTime);
  if (start === null || end === null || end <= start) return null;
  const totalMinutes = Math.floor((end - start) / 60_000);
  if (totalMinutes <= 1) return preset.nodes.length === 0 ? [] : null;
  const occupied = new Set<number>();
  const nodes: MarketStrategyNodeDraft[] = [];
  for (const node of preset.nodes) {
    const minuteOffset = Math.max(1, Math.min(totalMinutes - 1, Math.round((totalMinutes * Number(node.progress_percent)) / 100)));
    if (occupied.has(minuteOffset)) continue;
    occupied.add(minuteOffset);
    nodes.push({
      ...createMarketStrategyNodeDraft(),
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
  return nodes.sort((left, right) => String(left.targetTime).localeCompare(String(right.targetTime)));
}

function applyPreset(values: MarketStrategyValues, preset: MarketStrategyPreset): MarketStrategyValues | null {
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

function formatPreviewTime(value: number): string {
  const date = new Date(Number(value));
  return Number.isFinite(date.getTime()) ? date.toLocaleString('zh-CN', { hour12: false }) : '--';
}

function PreviewSparkline({ samples }: { samples: MarketStrategyPreviewSample[] }) {
  const points = useMemo(() => {
    const closes = samples.map((sample) => Number(sample.close)).filter(Number.isFinite);
    if (closes.length === 0) return '';
    const minimum = Math.min(...closes);
    const maximum = Math.max(...closes);
    const range = maximum - minimum || 1;
    return closes
      .map((close, index) => `${(index / Math.max(1, closes.length - 1)) * 100},${46 - ((close - minimum) / range) * 40}`)
      .join(' ');
  }, [samples]);

  return (
    <svg aria-label="预览收盘价走势" className="admin-market-preview-chart" preserveAspectRatio="none" role="img" viewBox="0 0 100 52">
      <defs>
        <linearGradient id="market-preview-fill" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" stopColor="currentColor" stopOpacity="0.22" />
          <stop offset="1" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
      </defs>
      {points ? <polyline fill="none" points={points} stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" vectorEffect="non-scaling-stroke" /> : null}
    </svg>
  );
}

function MarketStrategyPreviewAction({ disabled, strategyId, values }: { disabled: boolean; strategyId?: string; values: MarketStrategyValues }) {
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [preview, setPreview] = useState<MarketStrategyPreviewResponse | null>(null);

  async function runPreview() {
    setVisible(true);
    setLoading(true);
    setError('');
    setPreview(null);
    try {
      const result = await apiRequest<MarketStrategyPreviewResponse>('/admin/api/v1/market-strategies/preview', {
        method: 'POST',
        body: JSON.stringify({
          pair_id: requiredPositiveInteger(values.pairId, '交易对ID'),
          ...(strategyId ? { strategy_id: requiredPositiveInteger(strategyId, '策略ID') } : {}),
          ...marketStrategyBasePayload(values),
          status: values.status,
          reason: null,
          sample_count: 120
        })
      });
      setPreview(result);
    } catch (previewError) {
      setError(errorMessage(previewError));
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <Button disabled={disabled} onClick={() => void runPreview()} theme="light" type="primary">
        生成 OHLCV 预览
      </Button>
      <SideSheet
        maskClosable={!loading}
        onCancel={() => setVisible(false)}
        title="模拟行情预览"
        visible={visible}
        width={860}
      >
        <div aria-busy={loading} aria-live="polite" className="admin-market-preview-sheet">
          <div className="admin-market-preview-heading">
            <div>
              <Typography.Title heading={5}>无副作用预览</Typography.Title>
              <Typography.Text type="tertiary">只读取交易对并在内存生成采样，不写入行情、缓存或运行检查点。</Typography.Text>
            </div>
            <Button disabled={loading} loading={loading} onClick={() => void runPreview()} size="small">重新生成</Button>
          </div>
          {error ? <div className="admin-inline-error" role="alert">{error}</div> : null}
          {loading ? <div className="admin-market-preview-state">正在生成确定性行情样本…</div> : null}
          {!loading && preview ? (
            <>
              <div className="admin-market-preview-metrics">
                <Card bordered><span>完整分钟数</span><strong>{preview.one_minute_count}</strong></Card>
                <Card bordered><span>返回采样数</span><strong>{preview.sample_count}</strong></Card>
                <Card bordered><span>预览版本</span><strong>V{preview.preview_version}</strong></Card>
                <Card bordered className="admin-market-preview-seed"><span>本次预览 Seed</span><strong>{preview.preview_seed}</strong></Card>
              </div>
              {values.seedMode === 'auto' && values.regenerateSeed ? (
                <Typography.Text type="tertiary">
                  当前选择了重新生成 Seed；本次 Seed 只用于预览，正式提交新版本时会再次生成。
                </Typography.Text>
              ) : null}
              <PreviewSparkline samples={preview.samples ?? []} />
              <div aria-label="OHLCV 预览样本" className="admin-market-preview-grid" role="table">
                <div className="admin-market-preview-grid__row admin-market-preview-grid__head" role="row">
                  <span role="columnheader">时间</span><span role="columnheader">开</span><span role="columnheader">高</span><span role="columnheader">低</span><span role="columnheader">收</span><span role="columnheader">成交量</span>
                </div>
                {(preview.samples ?? []).map((sample) => (
                  <div className="admin-market-preview-grid__row" key={sample.open_time} role="row">
                    <time role="cell">{formatPreviewTime(sample.open_time)}</time>
                    <span role="cell">{sample.open}</span><span role="cell">{sample.high}</span><span role="cell">{sample.low}</span><strong role="cell">{sample.close}</strong><span role="cell">{sample.volume}</span>
                  </div>
                ))}
              </div>
            </>
          ) : null}
        </div>
      </SideSheet>
    </>
  );
}

function MarketStrategyForm({
  active,
  includePairId,
  isEditing,
  onChange,
  strategyId,
  values
}: {
  active: boolean;
  includePairId: boolean;
  isEditing: boolean;
  onChange: (values: MarketStrategyValues) => void;
  strategyId?: string;
  values: MarketStrategyValues;
}) {
  const [presets, setPresets] = useState<MarketStrategyPreset[]>([]);
  const [presetsLoading, setPresetsLoading] = useState(false);
  const [presetsError, setPresetsError] = useState('');
  const [presetsRequested, setPresetsRequested] = useState(false);

  useEffect(() => {
    if (!active || presetsRequested) return;
    setPresetsRequested(true);
    setPresetsLoading(true);
    setPresetsError('');
    apiRequest<MarketStrategyPresetsResponse>('/admin/api/v1/market-strategies/presets')
      .then((result) => {
        setPresets(Array.isArray(result.presets) ? result.presets : []);
      })
      .catch((loadError: unknown) => {
        setPresetsError(errorMessage(loadError));
      })
      .finally(() => {
        setPresetsLoading(false);
      });
  }, [active, presetsRequested]);

  const selectedPreset = presets.find((preset) => preset.code === values.scenario);
  const canPreview = isMarketStrategySubmittable(values, true);

  return (
    <div className="admin-market-strategy-form">
      <section className="admin-market-strategy-section">
        <div className="admin-market-strategy-section__heading">
          <div><h3>策略基础配置</h3><p>定义权威 1m 行情的交易对、时间范围、起止价格和全局量价边界。</p></div>
        </div>
        <div className="admin-action-form">
          {includePairId ? <label>交易对ID<AdminTextInput ariaLabel="交易对ID" value={values.pairId} onChange={(pairId) => onChange({ ...values, pairId })} /></label> : null}
          {!includePairId ? <label>交易对ID<AdminTextInput ariaLabel="交易对ID" readOnly value={values.pairId} onChange={() => undefined} /></label> : null}
          <label>策略类型<AdminTextInput ariaLabel="策略类型" value={values.strategyType} onChange={(strategyType) => onChange({ ...values, strategyType })} /></label>
          <label>起始价<AdminTextInput ariaLabel="起始价" value={values.startPrice} onChange={(startPrice) => onChange({ ...values, startPrice })} /></label>
          <label>目标价<AdminTextInput ariaLabel="目标价" value={values.targetPrice} onChange={(targetPrice) => onChange({ ...values, targetPrice })} /></label>
          <label>开始时间<AdminTextInput ariaLabel="开始时间" type="datetime-local" value={values.startTime} onChange={(startTime) => onChange({ ...values, startTime })} /></label>
          <label>结束时间<AdminTextInput ariaLabel="结束时间" type="datetime-local" value={values.endTime} onChange={(endTime) => onChange({ ...values, endTime })} /></label>
          <label>波动率<AdminTextInput ariaLabel="波动率" value={values.volatility} onChange={(volatility) => onChange({ ...values, volatility })} /></label>
          <label>最小成交量<AdminTextInput ariaLabel="最小成交量" value={values.volumeMin} onChange={(volumeMin) => onChange({ ...values, volumeMin })} /></label>
          <label>最大成交量<AdminTextInput ariaLabel="最大成交量" value={values.volumeMax} onChange={(volumeMax) => onChange({ ...values, volumeMax })} /></label>
          {includePairId ? (
            <label>
              初始状态
              <AdminSelect
                ariaLabel="初始状态"
                onChange={(status) => onChange({ ...values, status })}
                optionList={[
                  { value: 'draft', label: '草稿' },
                  { value: 'active', label: '启用' },
                  { value: 'paused', label: '暂停' },
                  { value: 'disabled', label: '禁用' }
                ]}
                value={values.status}
              />
            </label>
          ) : (
            <label>当前状态<AdminTextInput ariaLabel="当前状态" readOnly value={values.status} onChange={() => undefined} /></label>
          )}
        </div>
      </section>

      <section className="admin-market-strategy-section admin-market-generator-section">
        <div className="admin-market-strategy-section__heading">
          <div><h3>生成模型与场景</h3><p>场景只填充显式节点和参数；最终版本不会依赖隐藏规则，可审计、可重放。</p></div>
          <MarketStrategyPreviewAction disabled={!canPreview} strategyId={strategyId} values={values} />
        </div>
        <div className="admin-market-preset-bar">
          <label>
            行情场景
            <AdminSelect
              ariaLabel="行情场景"
              loading={presetsLoading}
              onChange={(scenario) => onChange({ ...values, scenario })}
              optionList={scenarioOptions}
              value={values.scenario}
            />
          </label>
          <Button
            disabled={!selectedPreset || presetsLoading}
            onClick={() => {
              if (!selectedPreset) return;
              const next = applyPreset(values, selectedPreset);
              if (!next) {
                Toast.warning('请先填写有效的起始价、开始时间与结束时间，再应用场景预设');
                return;
              }
              onChange(next);
              Toast.success(`已应用“${selectedPreset.name}”预设，所有参数仍可继续修改`);
            }}
            theme="solid"
            type="primary"
          >
            应用场景预设
          </Button>
          <div className="admin-market-preset-description">
            {presetsError ? (
              <span role="alert">
                预设加载失败：{presetsError}
                <Button onClick={() => setPresetsRequested(false)} size="small" theme="borderless">重新加载</Button>
              </span>
            ) : selectedPreset?.description ?? '选择场景后可一键生成显式参数与时间节点。'}
          </div>
        </div>
        <div className="admin-action-form admin-market-generator-fields">
          <label>
            Seed 模式
            <AdminSelect
              ariaLabel="Seed 模式"
              onChange={(seedMode) => onChange({ ...values, seedMode, regenerateSeed: false })}
              optionList={seedModeOptions}
              value={values.seedMode}
            />
          </label>
          {values.seedMode === 'fixed' ? (
            <label>固定 Seed<AdminTextInput ariaLabel="固定 Seed" placeholder="1～128 个字符" value={values.seed} onChange={(seed) => onChange({ ...values, seed })} /></label>
          ) : (
            <label>
              当前实际 Seed
              <AdminTextInput ariaLabel="当前实际 Seed" placeholder={isEditing ? '读取当前激活版本' : '创建时由后端生成'} readOnly value={values.seed} onChange={() => undefined} />
            </label>
          )}
          <label>均值回归强度（0～2）<AdminTextInput ariaLabel="均值回归强度" value={values.meanReversionStrength} onChange={(meanReversionStrength) => onChange({ ...values, meanReversionStrength })} /></label>
          <label>噪声强度（0～5）<AdminTextInput ariaLabel="噪声强度" value={values.noiseScale} onChange={(noiseScale) => onChange({ ...values, noiseScale })} /></label>
          <label>影线强度（0～5）<AdminTextInput ariaLabel="影线强度" value={values.wickScale} onChange={(wickScale) => onChange({ ...values, wickScale })} /></label>
          <label>
            成交量形态
            <AdminSelect ariaLabel="成交量形态" onChange={(volumeShape) => onChange({ ...values, volumeShape })} optionList={volumeShapeOptions} value={values.volumeShape} />
          </label>
        </div>
        {isEditing && values.seedMode === 'auto' ? (
          <div className="admin-market-seed-command">
            <AdminCheckbox checked={values.regenerateSeed} onChange={(regenerateSeed) => onChange({ ...values, regenerateSeed })}>
              为本次新版本重新生成 Seed；未选中时继承当前激活版本，保持随机纹理连续
            </AdminCheckbox>
          </div>
        ) : null}
      </section>

      <MarketStrategyNodeEditor value={values.nodes} onChange={(nodes) => onChange({ ...values, nodes })} />
    </div>
  );
}

function nextMarketStrategyStatus(status: string): 'active' | 'disabled' {
  return status === 'active' ? 'disabled' : 'active';
}

export function MarketStrategyRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const strategyId = recordString(record, 'id');
  const nextStatus = nextMarketStrategyStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);
  const [config, setConfig] = useState(() => marketStrategyFromRecord(record));
  const [loading, setLoading] = useState(false);
  const [visible, setVisible] = useState(false);

  async function openEditor() {
    setLoading(true);
    try {
      // 列表接口不携带节点与版本生成参数；编辑前必须读取详情，避免空数组覆盖既有节点。
      const detail = await apiRequest<ApiRecord>(`/admin/api/v1/market-strategies/${strategyId}`);
      setConfig(marketStrategyFromRecord(detail));
      setVisible(true);
    } catch (error) {
      Toast.error(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <Button disabled={!strategyId} onClick={() => openRecordDetail('/admin/api/v1/market-strategies', strategyId, helpers)} size="small" theme="borderless">查看详情</Button>
      <MarketStrategyRecoverySheet strategyId={strategyId} />
      <MarketStrategyVersionSheet onRestored={helpers.reload} strategyId={strategyId} />
      <Button disabled={!strategyId} loading={loading} onClick={() => void openEditor()} size="small" theme="borderless">修改</Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改行情策略" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <MarketStrategyForm active={visible} includePairId={false} isEditing strategyId={strategyId} values={config} onChange={setConfig} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isMarketStrategySubmittable(config, false)}
              title="确认修改行情策略"
              onConfirm={async (reason) => {
                await submitAction('修改行情策略', () =>
                  apiRequest(`/admin/api/v1/market-strategies/${strategyId}`, {
                    method: 'PATCH',
                    body: JSON.stringify({ ...marketStrategyBasePayload(config), reason })
                  })
                );
                setVisible(false);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
      <ConfirmAction
        actionText={actionText}
        disabled={!strategyId}
        title={`${actionText}行情策略`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}行情策略`, () =>
            apiRequest(`/admin/api/v1/market-strategies/${strategyId}/status`, {
              method: 'PATCH',
              body: JSON.stringify({ status: nextStatus, reason })
            })
          );
          helpers.reload();
        }}
      />
    </>
  );
}

export function CreateMarketStrategyAction({ onCreated }: { onCreated?: () => void }) {
  const [strategy, setStrategy] = useState(initialMarketStrategy);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>创建策略</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="创建策略" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <MarketStrategyForm active={visible} includePairId isEditing={false} values={strategy} onChange={setStrategy} />
            <ConfirmAction
              actionText="提交创建策略"
              disabled={!isMarketStrategySubmittable(strategy, true)}
              title="确认创建行情策略"
              onConfirm={async (reason) => {
                await submitAction('创建行情策略', () =>
                  apiRequest('/admin/api/v1/market-strategies', {
                    method: 'POST',
                    body: JSON.stringify({
                      pair_id: requiredPositiveInteger(strategy.pairId, '交易对ID'),
                      ...marketStrategyBasePayload(strategy),
                      status: strategy.status,
                      reason
                    })
                  })
                );
                setVisible(false);
                setStrategy(initialMarketStrategy);
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}
