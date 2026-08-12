import { Button, Card, SideSheet, Space, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { AdminModalTriggerButton, AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  MarketStrategyNodeEditor,
  createMarketStrategyNodeDraft,
  type MarketStrategyNodeDraft
} from '../../components/MarketStrategyNodeEditor';
import { MarketStrategyRecoverySheet } from '../../components/MarketStrategyRecoverySheet';
import {
  AssetSelect,
  type CreateActionProps,
  FormModal,
  type RowActionHelpers,
  activeStatusOptions,
  completeCreate,
  createModalProps,
  errorMessage,
  isNonNegativeIntegerInput,
  openRecordDetail,
  optionalString,
  recordString,
  requiredNonNegativeInteger,
  requiredPositiveInteger,
  requiredString,
  statusOptions,
  submitAction,
  toggleActionText,
  useAssetOptions
} from './shared';

type SpotPairValues = {
  baseAssetId: string;
  logoUrl: string;
  quoteAssetId: string;
  symbol: string;
  pricePrecision: string;
  qtyPrecision: string;
  minOrderValue: string;
  status: string;
  marketType: string;
};

type MarketPairConfigValues = {
  logoUrl: string;
  pricePrecision: string;
  qtyPrecision: string;
  minOrderValue: string;
  marketType: string;
  status: string;
};

type MarketStrategyValues = {
  endTime: string;
  pairId: string;
  startPrice: string;
  startTime: string;
  status: string;
  strategyType: string;
  targetPrice: string;
  volatility: string;
  volumeMax: string;
  volumeMin: string;
  nodes: MarketStrategyNodeDraft[];
};

const initialSpotPair: SpotPairValues = {
  baseAssetId: '',
  logoUrl: '',
  quoteAssetId: '',
  symbol: '',
  pricePrecision: '',
  qtyPrecision: '',
  minOrderValue: '',
  status: 'active',
  marketType: 'external'
};

const initialMarketStrategy: MarketStrategyValues = {
  pairId: '',
  strategyType: 'price_path',
  startPrice: '',
  targetPrice: '',
  startTime: '',
  endTime: '',
  volatility: '0',
  volumeMin: '0',
  volumeMax: '0',
  nodes: [],
  status: 'draft'
};

type MarketStrategyNodeRecord = {
  id?: unknown;
  sequence_no?: unknown;
  target_time?: unknown;
  target_type?: unknown;
  target_value?: unknown;
  execution_mode?: unknown;
  tolerance?: unknown;
  volatility?: unknown;
  volume_min?: unknown;
  volume_max?: unknown;
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

function isNonNegativeDecimalInput(value: string): boolean {
  const parsed = Number(value);
  return value.trim().length > 0 && Number.isFinite(parsed) && parsed >= 0;
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
      isNonNegativeDecimalInput(node.tolerance) &&
      isNonNegativeDecimalInput(node.volatility) &&
      ((!volumeMin && !volumeMax) || (isNonNegativeDecimalInput(volumeMin) && isNonNegativeDecimalInput(volumeMax) && Number(volumeMax) >= Number(volumeMin)))
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

function isSpotPairCreatable(values: SpotPairValues): boolean {
  return Boolean(
    values.baseAssetId.trim() &&
      values.quoteAssetId.trim() &&
      values.symbol.trim() &&
      isNonNegativeIntegerInput(values.pricePrecision) &&
      isNonNegativeIntegerInput(values.qtyPrecision) &&
      values.minOrderValue.trim()
  );
}

function isMarketPairConfigUpdatable(values: MarketPairConfigValues): boolean {
  return Boolean(isNonNegativeIntegerInput(values.pricePrecision) && isNonNegativeIntegerInput(values.qtyPrecision) && values.minOrderValue.trim() && values.marketType.trim() && values.status.trim());
}

function isMarketStrategySubmittable(values: MarketStrategyValues, includePairId: boolean): boolean {
  const startTime = parseInputDateTime(values.startTime);
  const endTime = parseInputDateTime(values.endTime);
  if (startTime === null || endTime === null || endTime <= startTime) {
    return false;
  }

  let previousNodeTime = startTime;
  for (const node of values.nodes) {
    const targetTime = parseInputDateTime(node.targetTime);
    if (
      targetTime === null ||
      targetTime <= startTime ||
      targetTime >= endTime ||
      targetTime <= previousNodeTime ||
      !isMarketStrategyNodeSubmittable(node, targetTime)
    ) {
      return false;
    }
    previousNodeTime = targetTime;
  }

  return Boolean(
    (!includePairId || values.pairId.trim()) &&
      values.strategyType.trim() &&
      values.startPrice.trim() &&
      values.targetPrice.trim() &&
      values.volatility.trim() &&
      values.volumeMin.trim() &&
      values.volumeMax.trim()
  );
}

function canCancelSpotOrder(status: string): boolean {
  return status === 'pending' || status === 'open' || status === 'partially_filled';
}

function nextMarketStrategyStatus(status: string): 'active' | 'disabled' {
  return status === 'active' ? 'disabled' : 'active';
}

const marketTypeOptions: SemiSelectOption[] = [
  { value: 'external', label: '外部行情' },
  { value: 'internal', label: '内部撮合' },
  { value: 'strategy', label: '策略行情' }
];

function MarketPairEditAction({ helpers, pairId, record }: { helpers: RowActionHelpers; pairId: string; record: ApiRecord }) {
  const [config, setConfig] = useState<MarketPairConfigValues>({
    logoUrl: recordString(record, 'logo_url'),
    pricePrecision: recordString(record, 'price_precision'),
    qtyPrecision: recordString(record, 'qty_precision'),
    minOrderValue: recordString(record, 'min_order_value'),
    marketType: recordString(record, 'market_type') || 'external',
    status: recordString(record, 'status') || 'active'
  });
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!pairId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改交易对配置" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
              <label>交易对<AdminTextInput ariaLabel="交易对" disabled value={recordString(record, 'symbol')} onChange={() => undefined} /></label>
              <label>基础资产<AdminTextInput ariaLabel="基础资产" disabled value={recordString(record, 'base_asset')} onChange={() => undefined} /></label>
              <label>计价资产<AdminTextInput ariaLabel="计价资产" disabled value={recordString(record, 'quote_asset')} onChange={() => undefined} /></label>
              <label>
                当前状态
                <AdminSelect ariaLabel="当前状态" onChange={(status) => setConfig({ ...config, status })} optionList={statusOptions} value={config.status} />
              </label>
              <AdminImageUpload label="交易对 Logo" value={config.logoUrl} variant="avatar" onChange={(logoUrl) => setConfig({ ...config, logoUrl })} />
              <label>价格精度<AdminTextInput ariaLabel="价格精度" value={config.pricePrecision} onChange={(pricePrecision) => setConfig({ ...config, pricePrecision })} /></label>
              <label>数量精度<AdminTextInput ariaLabel="数量精度" value={config.qtyPrecision} onChange={(qtyPrecision) => setConfig({ ...config, qtyPrecision })} /></label>
              <label>最小下单额<AdminTextInput ariaLabel="最小下单额" value={config.minOrderValue} onChange={(minOrderValue) => setConfig({ ...config, minOrderValue })} /></label>
              <label>
                市场类型
                <AdminSelect ariaLabel="市场类型" onChange={(marketType) => setConfig({ ...config, marketType })} optionList={marketTypeOptions} value={config.marketType} />
              </label>
            </div>
            <ConfirmAction
              actionText="提交修改"
              disabled={!isMarketPairConfigUpdatable(config)}
              title="确认修改交易对配置"
              onConfirm={async (reason) => {
                await submitAction('修改交易对配置', () =>
                  apiRequest(`/admin/api/v1/market-pairs/${pairId}`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      logo_url: optionalString(config.logoUrl),
                      price_precision: requiredNonNegativeInteger(config.pricePrecision, '价格精度'),
                      qty_precision: requiredNonNegativeInteger(config.qtyPrecision, '数量精度'),
                      min_order_value: requiredString(config.minOrderValue, '最小下单额'),
                      status: requiredString(config.status, '状态'),
                      market_type: requiredString(config.marketType, '市场类型'),
                      reason
                    })
                  })
                );
                setVisible(false);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

export function MarketPairRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const pairId = recordString(record, 'id');
  const nextStatus = recordString(record, 'status') === 'active' ? 'disabled' : 'active';
  const actionText = nextStatus === 'disabled' ? '禁用' : '启用';

  return (
    <>
      <Button disabled={!pairId} onClick={() => openRecordDetail('/admin/api/v1/market-pairs', pairId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <MarketPairEditAction helpers={helpers} pairId={pairId} record={record} />
      <ConfirmAction
        actionText={actionText}
        disabled={!pairId}
        title={`${actionText}交易对`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}交易对`, () =>
            apiRequest(`/admin/api/v1/market-pairs/${pairId}/status`, {
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

export function SpotOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'id');
  const status = recordString(record, 'status');

  return (
    <>
      <Button disabled={!orderId} onClick={() => openRecordDetail('/admin/api/v1/spot/orders', orderId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText="管理员撤单"
        disabled={!orderId || !canCancelSpotOrder(status)}
        title="管理员撤单"
        onConfirm={async (reason) => {
          await submitAction('管理员撤单', () =>
            apiRequest(`/admin/api/v1/spot/orders/${orderId}/cancel`, {
              method: 'POST',
              body: JSON.stringify({ reason })
            })
          );
          helpers.reload();
        }}
      />
    </>
  );
}

function marketStrategyFromRecord(record: ApiRecord): MarketStrategyValues {
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
    status: recordString(record, 'status') || 'draft'
  };
}

function MarketStrategyForm({ includePairId, onChange, values }: { includePairId: boolean; onChange: (values: MarketStrategyValues) => void; values: MarketStrategyValues }) {
  return (
    <div className="admin-market-strategy-form">
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
      <MarketStrategyNodeEditor value={values.nodes} onChange={(nodes) => onChange({ ...values, nodes })} />
    </div>
  );
}

export function MarketStrategyRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const strategyId = recordString(record, 'id');
  const nextStatus = nextMarketStrategyStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);
  const [config, setConfig] = useState(() => marketStrategyFromRecord(record));
  const [loading, setLoading] = useState(false);
  const [visible, setVisible] = useState(false);

  const openEditor = async () => {
    setLoading(true);
    try {
      // 列表接口刻意不携带节点集合；编辑前读取详情，避免用空数组覆盖既有路径节点。
      const detail = await apiRequest<ApiRecord>(`/admin/api/v1/market-strategies/${strategyId}`);
      setConfig(marketStrategyFromRecord(detail));
      setVisible(true);
    } catch (error) {
      Toast.error(errorMessage(error));
    } finally {
      setLoading(false);
    }
  };

  return (
    <>
      <Button disabled={!strategyId} onClick={() => openRecordDetail('/admin/api/v1/market-strategies', strategyId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <MarketStrategyRecoverySheet strategyId={strategyId} />
      <Button disabled={!strategyId} loading={loading} onClick={() => void openEditor()} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改行情策略" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <MarketStrategyForm includePairId={false} values={config} onChange={setConfig} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isMarketStrategySubmittable(config, false)}
              title="确认修改行情策略"
              onConfirm={async (reason) => {
                await submitAction('修改行情策略', () =>
                  apiRequest(`/admin/api/v1/market-strategies/${strategyId}`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      strategy_type: requiredString(config.strategyType, '策略类型'),
                      start_price: requiredString(config.startPrice, '起始价'),
                      target_price: requiredString(config.targetPrice, '目标价'),
                      start_time: unixMillisFromInputDateTime(config.startTime, '开始时间'),
                      end_time: unixMillisFromInputDateTime(config.endTime, '结束时间'),
                      volatility: requiredString(config.volatility, '波动率'),
                      volume_min: requiredString(config.volumeMin, '最小成交量'),
                      volume_max: requiredString(config.volumeMax, '最大成交量'),
                      nodes: config.nodes.map(marketStrategyNodePayload),
                      reason
                    })
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
            <MarketStrategyForm includePairId values={strategy} onChange={setStrategy} />
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
                      strategy_type: requiredString(strategy.strategyType, '策略类型'),
                      start_price: requiredString(strategy.startPrice, '起始价'),
                      target_price: requiredString(strategy.targetPrice, '目标价'),
                      start_time: unixMillisFromInputDateTime(strategy.startTime, '开始时间'),
                      end_time: unixMillisFromInputDateTime(strategy.endTime, '结束时间'),
                      volatility: requiredString(strategy.volatility, '波动率'),
                      volume_min: requiredString(strategy.volumeMin, '最小成交量'),
                      volume_max: requiredString(strategy.volumeMax, '最大成交量'),
                      nodes: strategy.nodes.map(marketStrategyNodePayload),
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

export function CreateSpotPairAction({ onCreated }: CreateActionProps = {}) {
  const [spotPair, setSpotPair] = useState(initialSpotPair);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="添加交易对" size="wide" title="添加现货交易对">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <AssetSelect
              label="基础资产"
              loading={assetLoading}
              options={assetOptions}
              value={spotPair.baseAssetId}
              onChange={(baseAssetId) => setSpotPair({ ...spotPair, baseAssetId })}
            />
            <AssetSelect
              label="计价资产"
              loading={assetLoading}
              options={assetOptions}
              value={spotPair.quoteAssetId}
              onChange={(quoteAssetId) => setSpotPair({ ...spotPair, quoteAssetId })}
            />
            <label>交易对符号<AdminTextInput ariaLabel="交易对符号" value={spotPair.symbol} onChange={(symbol) => setSpotPair({ ...spotPair, symbol })} placeholder="BTC-USDT" /></label>
            <AdminImageUpload label="交易对 Logo" value={spotPair.logoUrl} variant="avatar" onChange={(logoUrl) => setSpotPair({ ...spotPair, logoUrl })} />
            <label>价格精度<AdminTextInput ariaLabel="价格精度" value={spotPair.pricePrecision} onChange={(pricePrecision) => setSpotPair({ ...spotPair, pricePrecision })} /></label>
            <label>数量精度<AdminTextInput ariaLabel="数量精度" value={spotPair.qtyPrecision} onChange={(qtyPrecision) => setSpotPair({ ...spotPair, qtyPrecision })} /></label>
            <label>最小下单额<AdminTextInput ariaLabel="最小下单额" value={spotPair.minOrderValue} onChange={(minOrderValue) => setSpotPair({ ...spotPair, minOrderValue })} /></label>
            <label>
              初始状态
              <AdminSelect ariaLabel="初始状态" onChange={(status) => setSpotPair({ ...spotPair, status })} optionList={activeStatusOptions} value={spotPair.status} />
            </label>
            <label>
              市场类型
              <AdminSelect ariaLabel="市场类型" onChange={(marketType) => setSpotPair({ ...spotPair, marketType })} optionList={marketTypeOptions} value={spotPair.marketType} />
            </label>
          </div>
          <ConfirmAction
            actionText="提交添加交易对"
            disabled={!isSpotPairCreatable(spotPair)}
            title="确认添加现货交易对"
            onConfirm={async (reason) => {
              await submitAction('添加现货交易对', () =>
                apiRequest('/admin/api/v1/market-pairs', {
                  method: 'POST',
                  body: JSON.stringify({
                    base_asset_id: requiredPositiveInteger(spotPair.baseAssetId, '基础资产ID'),
                    quote_asset_id: requiredPositiveInteger(spotPair.quoteAssetId, '计价资产ID'),
                    symbol: requiredString(spotPair.symbol, '交易对符号'),
                    logo_url: optionalString(spotPair.logoUrl),
                    price_precision: requiredNonNegativeInteger(spotPair.pricePrecision, '价格精度'),
                    qty_precision: requiredNonNegativeInteger(spotPair.qtyPrecision, '数量精度'),
                    min_order_value: requiredString(spotPair.minOrderValue, '最小下单额'),
                    status: spotPair.status,
                    market_type: spotPair.marketType,
                    reason
                  })
                })
              );
              completeCreate(close, onCreated, () => setSpotPair(initialSpotPair));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
