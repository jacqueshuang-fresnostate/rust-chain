import { Button, Card, Modal, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { type ReactNode, useEffect, useState } from 'react';

import { listAdminResource } from '../../api/adminResources';
import { ApiError, apiRequest } from '../../api/client';
import type { ApiRecord } from '../../api/types';
import { ConfirmAction } from '../../shared/ConfirmAction';

const { Title } = Typography;

type AssetValues = {
  symbol: string;
  name: string;
  precisionScale: string;
  assetType: string;
  status: string;
};

type SpotPairValues = {
  baseAssetId: string;
  quoteAssetId: string;
  symbol: string;
  pricePrecision: string;
  qtyPrecision: string;
  minOrderValue: string;
  status: string;
  marketType: string;
};

type MarketPairConfigValues = {
  pricePrecision: string;
  qtyPrecision: string;
  minOrderValue: string;
  marketType: string;
};

type MarginProductValues = {
  pairId: string;
  marginAsset: string;
  maxLeverage: string;
  minMargin: string;
  maxMargin: string;
  maintenanceMarginRate: string;
  hourlyInterestRate: string;
  status: string;
};

type SecondsProductValues = {
  pairId: string;
  stakeAsset: string;
  durationSeconds: string;
  payoutRate: string;
  minStake: string;
  maxStake: string;
  status: string;
};

type AssetOption = {
  id: string;
  label: string;
};

type RowActionHelpers = {
  reload: () => void;
  openJson: (data: ApiRecord) => void;
};

const initialAsset: AssetValues = {
  symbol: '',
  name: '',
  precisionScale: '8',
  assetType: 'coin',
  status: 'active'
};

const initialSpotPair: SpotPairValues = {
  baseAssetId: '',
  quoteAssetId: '',
  symbol: '',
  pricePrecision: '',
  qtyPrecision: '',
  minOrderValue: '',
  status: 'active',
  marketType: 'external'
};

const initialMarginProduct: MarginProductValues = {
  pairId: '',
  marginAsset: '',
  maxLeverage: '',
  minMargin: '',
  maxMargin: '',
  maintenanceMarginRate: '',
  hourlyInterestRate: '',
  status: 'active'
};

const initialSecondsProduct: SecondsProductValues = {
  pairId: '',
  stakeAsset: '',
  durationSeconds: '',
  payoutRate: '',
  minStake: '',
  maxStake: '',
  status: 'active'
};

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function requiredPositiveInteger(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
  }
  return parsed;
}

function requiredNonNegativeInteger(value: string, label: string): number {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${label}不能为空`);
  }
  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`${label}必须为非负整数`);
  }
  return parsed;
}

function isNonNegativeIntegerInput(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }
  const parsed = Number(trimmed);
  return Number.isInteger(parsed) && parsed >= 0;
}

function requiredString(value: string, label: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${label}不能为空`);
  }
  return trimmed;
}

function optionalString(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}

function assetFieldToString(asset: ApiRecord, key: string): string {
  const value = asset[key];
  return typeof value === 'number' || typeof value === 'string' ? String(value) : '';
}

function assetOptionLabel(asset: ApiRecord): string {
  const id = assetFieldToString(asset, 'id');
  const symbol = assetFieldToString(asset, 'symbol') || `资产${id}`;
  const name = assetFieldToString(asset, 'name');
  return `${symbol}${name ? ` - ${name}` : ''}（ID: ${id}）`;
}

function toAssetOption(asset: ApiRecord): AssetOption | null {
  const id = assetFieldToString(asset, 'id');
  return id ? { id, label: assetOptionLabel(asset) } : null;
}

function isAssetCreatable(values: AssetValues): boolean {
  return Boolean(values.symbol.trim() && values.name.trim() && isNonNegativeIntegerInput(values.precisionScale));
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
  return Boolean(isNonNegativeIntegerInput(values.pricePrecision) && isNonNegativeIntegerInput(values.qtyPrecision) && values.minOrderValue.trim() && values.marketType.trim());
}

async function submitAction(label: string, request: () => Promise<unknown>) {
  try {
    await request();
    Toast.success(`${label}已提交`);
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

function FormModal({ actionText, children, title }: { actionText: string; children: ReactNode; title: string }) {
  const [visible, setVisible] = useState(false);

  return (
    <>
      <button className="admin-inline-action-button" onClick={() => setVisible(true)} type="button">
        {actionText}
      </button>
      <Modal footer={null} onCancel={() => setVisible(false)} title={title} visible={visible}>
        {children}
      </Modal>
    </>
  );
}

function useAssetOptions() {
  const [assetOptions, setAssetOptions] = useState<AssetOption[]>([]);
  const [assetLoading, setAssetLoading] = useState(false);

  useEffect(() => {
    let active = true;
    setAssetLoading(true);

    listAdminResource('/admin/api/v1/assets', 'assets', { status: 'active', limit: 100 })
      .then((result) => {
        if (!active) {
          return;
        }

        setAssetOptions(result.rows.map(toAssetOption).filter((asset): asset is AssetOption => asset !== null));
      })
      .catch(() => {
        if (active) {
          setAssetOptions([]);
        }
      })
      .finally(() => {
        if (active) {
          setAssetLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  return { assetLoading, assetOptions };
}

function AssetSelect({
  label,
  loading,
  onChange,
  options,
  value
}: {
  label: string;
  loading: boolean;
  onChange: (value: string) => void;
  options: AssetOption[];
  value: string;
}) {
  return (
    <label>
      {label}
      <select disabled={loading} value={value} onChange={(event) => onChange(event.currentTarget.value)}>
        <option value="">{loading ? '加载资产中...' : '请选择资产'}</option>
        {options.map((asset) => (
          <option key={asset.id} value={asset.id}>
            {asset.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function recordString(record: ApiRecord, key: string): string {
  const value = record[key];
  return typeof value === 'number' || typeof value === 'string' ? String(value) : '';
}

function canCancelSpotOrder(status: string): boolean {
  return status === 'pending' || status === 'open' || status === 'partially_filled';
}

async function openRecordDetail(endpoint: string, recordId: string, helpers: RowActionHelpers) {
  try {
    helpers.openJson(await apiRequest<ApiRecord>(`${endpoint}/${recordId}`));
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

function nextToggleStatus(status: string): 'active' | 'disabled' {
  return status === 'active' ? 'disabled' : 'active';
}

function toggleActionText(nextStatus: string): string {
  return nextStatus === 'disabled' ? '禁用' : '启用';
}

function MarketPairEditAction({ helpers, pairId, record }: { helpers: RowActionHelpers; pairId: string; record: ApiRecord }) {
  const [config, setConfig] = useState<MarketPairConfigValues>({
    pricePrecision: recordString(record, 'price_precision'),
    qtyPrecision: recordString(record, 'qty_precision'),
    minOrderValue: recordString(record, 'min_order_value'),
    marketType: recordString(record, 'market_type') || 'external'
  });
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!pairId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <Modal footer={null} onCancel={() => setVisible(false)} title="修改交易对配置" visible={visible}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>修改交易对配置</Title>
            <div className="admin-action-form">
              <label>交易对<input readOnly value={recordString(record, 'symbol')} /></label>
              <label>基础资产<input readOnly value={recordString(record, 'base_asset')} /></label>
              <label>计价资产<input readOnly value={recordString(record, 'quote_asset')} /></label>
              <label>当前状态<input readOnly value={recordString(record, 'status')} /></label>
              <label>价格精度<input aria-label="价格精度" value={config.pricePrecision} onChange={(event) => setConfig({ ...config, pricePrecision: event.currentTarget.value })} /></label>
              <label>数量精度<input aria-label="数量精度" value={config.qtyPrecision} onChange={(event) => setConfig({ ...config, qtyPrecision: event.currentTarget.value })} /></label>
              <label>最小下单额<input aria-label="最小下单额" value={config.minOrderValue} onChange={(event) => setConfig({ ...config, minOrderValue: event.currentTarget.value })} /></label>
              <label>
                市场类型
                <select aria-label="市场类型" value={config.marketType} onChange={(event) => setConfig({ ...config, marketType: event.currentTarget.value })}>
                  <option value="external">外部行情</option>
                  <option value="internal">内部撮合</option>
                  <option value="strategy">策略行情</option>
                </select>
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
                      price_precision: requiredNonNegativeInteger(config.pricePrecision, '价格精度'),
                      qty_precision: requiredNonNegativeInteger(config.qtyPrecision, '数量精度'),
                      min_order_value: requiredString(config.minOrderValue, '最小下单额'),
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
      </Modal>
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

export function MarginProductRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const productId = recordString(record, 'id');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/margin/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText={actionText}
        disabled={!productId}
        title={`${actionText}杠杆产品`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}杠杆产品`, () =>
            apiRequest(`/admin/api/v1/margin/products/${productId}/status`, {
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

export function MarginPositionRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const positionId = recordString(record, 'id');

  return (
    <Button disabled={!positionId} onClick={() => openRecordDetail('/admin/api/v1/margin/positions', positionId, helpers)} size="small" theme="borderless">
      查看详情
    </Button>
  );
}

export function MarginLiquidationRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const liquidationId = recordString(record, 'id');

  return (
    <Button disabled={!liquidationId} onClick={() => openRecordDetail('/admin/api/v1/margin/liquidations', liquidationId, helpers)} size="small" theme="borderless">
      查看详情
    </Button>
  );
}

export function SecondsProductRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const productId = recordString(record, 'id');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/seconds-contracts/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText={actionText}
        disabled={!productId}
        title={`${actionText}秒合约产品`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}秒合约产品`, () =>
            apiRequest(`/admin/api/v1/seconds-contracts/products/${productId}/status`, {
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

export function SecondsOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'id');
  const canSettle = recordString(record, 'status') === 'opened';

  async function settle(result: 'win' | 'loss', reason: string) {
    await submitAction(result === 'win' ? '结算赢' : '结算输', () =>
      apiRequest(`/admin/api/v1/seconds-contracts/orders/${orderId}/settle`, {
        method: 'POST',
        body: JSON.stringify({ result, reason })
      })
    );
    helpers.reload();
  }

  return (
    <>
      <Button disabled={!orderId} onClick={() => openRecordDetail('/admin/api/v1/seconds-contracts/orders', orderId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction actionText="结算赢" disabled={!orderId || !canSettle} title="结算赢" onConfirm={(reason) => settle('win', reason)} />
      <ConfirmAction actionText="结算输" disabled={!orderId || !canSettle} title="结算输" onConfirm={(reason) => settle('loss', reason)} />
    </>
  );
}

export function EarnProductRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const productId = recordString(record, 'id');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/earn/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText={actionText}
        disabled={!productId}
        title={`${actionText}理财产品`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}理财产品`, () =>
            apiRequest(`/admin/api/v1/earn/products/${productId}/status`, {
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

export function EarnSubscriptionRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const subscriptionId = recordString(record, 'id');

  return (
    <Button disabled={!subscriptionId} onClick={() => openRecordDetail('/admin/api/v1/earn/subscriptions', subscriptionId, helpers)} size="small" theme="borderless">
      查看详情
    </Button>
  );
}

export function ConvertPairRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const pairId = recordString(record, 'id');
  const enabled = record.enabled === true;
  const nextEnabled = !enabled;
  const actionText = enabled ? '禁用' : '启用';

  return (
    <>
      <Button disabled={!pairId} onClick={() => openRecordDetail('/admin/api/v1/convert/pairs', pairId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText={actionText}
        disabled={!pairId}
        title={`${actionText}闪兑交易对`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}闪兑交易对`, () =>
            apiRequest(`/admin/api/v1/convert/pairs/${pairId}`, {
              method: 'PATCH',
              body: JSON.stringify({ enabled: nextEnabled, reason })
            })
          );
          helpers.reload();
        }}
      />
    </>
  );
}

export function ConvertOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'id');

  return (
    <Button disabled={!orderId} onClick={() => openRecordDetail('/admin/api/v1/convert/orders', orderId, helpers)} size="small" theme="borderless">
      查看详情
    </Button>
  );
}

export function CreateAssetAction() {
  const [asset, setAsset] = useState(initialAsset);

  return (
    <FormModal actionText="添加资产" title="添加资产">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加资产</Title>
          <div className="admin-action-form">
            <label>资产符号<input value={asset.symbol} onChange={(event) => setAsset({ ...asset, symbol: event.currentTarget.value })} placeholder="BTC" /></label>
            <label>资产名称<input value={asset.name} onChange={(event) => setAsset({ ...asset, name: event.currentTarget.value })} placeholder="Bitcoin" /></label>
            <label>资产精度<input value={asset.precisionScale} onChange={(event) => setAsset({ ...asset, precisionScale: event.currentTarget.value })} /></label>
            <label>
              资产类型
              <select value={asset.assetType} onChange={(event) => setAsset({ ...asset, assetType: event.currentTarget.value })}>
                <option value="coin">数字货币</option>
                <option value="stablecoin">稳定币</option>
                <option value="fiat">法币</option>
                <option value="platform">平台币</option>
              </select>
            </label>
            <label>
              初始状态
              <select value={asset.status} onChange={(event) => setAsset({ ...asset, status: event.currentTarget.value })}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
          </div>
          <ConfirmAction
            actionText="提交添加资产"
            disabled={!isAssetCreatable(asset)}
            title="确认添加资产"
            onConfirm={(reason) =>
              submitAction('添加资产', () =>
                apiRequest('/admin/api/v1/assets', {
                  method: 'POST',
                  body: JSON.stringify({
                    symbol: requiredString(asset.symbol, '资产符号'),
                    name: requiredString(asset.name, '资产名称'),
                    precision_scale: requiredNonNegativeInteger(asset.precisionScale, '资产精度'),
                    asset_type: asset.assetType,
                    status: asset.status,
                    reason
                  })
                })
              )
            }
          />
        </Space>
      </Card>
    </FormModal>
  );
}

export function CreateSpotPairAction() {
  const [spotPair, setSpotPair] = useState(initialSpotPair);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="添加交易对" title="添加现货交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加现货交易对</Title>
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
            <label>交易对符号<input value={spotPair.symbol} onChange={(event) => setSpotPair({ ...spotPair, symbol: event.currentTarget.value })} placeholder="BTC-USDT" /></label>
            <label>价格精度<input value={spotPair.pricePrecision} onChange={(event) => setSpotPair({ ...spotPair, pricePrecision: event.currentTarget.value })} /></label>
            <label>数量精度<input value={spotPair.qtyPrecision} onChange={(event) => setSpotPair({ ...spotPair, qtyPrecision: event.currentTarget.value })} /></label>
            <label>最小下单额<input value={spotPair.minOrderValue} onChange={(event) => setSpotPair({ ...spotPair, minOrderValue: event.currentTarget.value })} /></label>
            <label>
              初始状态
              <select value={spotPair.status} onChange={(event) => setSpotPair({ ...spotPair, status: event.currentTarget.value })}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
            <label>
              市场类型
              <select value={spotPair.marketType} onChange={(event) => setSpotPair({ ...spotPair, marketType: event.currentTarget.value })}>
                <option value="external">外部行情</option>
                <option value="internal">内部撮合</option>
                <option value="strategy">策略行情</option>
              </select>
            </label>
          </div>
          <ConfirmAction
            actionText="提交添加交易对"
            disabled={!isSpotPairCreatable(spotPair)}
            title="确认添加现货交易对"
            onConfirm={(reason) =>
              submitAction('添加现货交易对', () =>
                apiRequest('/admin/api/v1/market-pairs', {
                  method: 'POST',
                  body: JSON.stringify({
                    base_asset_id: requiredPositiveInteger(spotPair.baseAssetId, '基础资产ID'),
                    quote_asset_id: requiredPositiveInteger(spotPair.quoteAssetId, '计价资产ID'),
                    symbol: requiredString(spotPair.symbol, '交易对符号'),
                    price_precision: requiredNonNegativeInteger(spotPair.pricePrecision, '价格精度'),
                    qty_precision: requiredNonNegativeInteger(spotPair.qtyPrecision, '数量精度'),
                    min_order_value: requiredString(spotPair.minOrderValue, '最小下单额'),
                    status: spotPair.status,
                    market_type: spotPair.marketType,
                    reason
                  })
                })
              )
            }
          />
        </Space>
      </Card>
    </FormModal>
  );
}

export function CreateMarginPairAction() {
  const [marginProduct, setMarginProduct] = useState(initialMarginProduct);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="添加杠杆交易对" title="添加杠杆交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加杠杆交易对</Title>
          <div className="admin-action-form">
            <label>杠杆交易对ID<input value={marginProduct.pairId} onChange={(event) => setMarginProduct({ ...marginProduct, pairId: event.currentTarget.value })} /></label>
            <AssetSelect
              label="保证金资产"
              loading={assetLoading}
              options={assetOptions}
              value={marginProduct.marginAsset}
              onChange={(marginAsset) => setMarginProduct({ ...marginProduct, marginAsset })}
            />
            <label>最大杠杆<input value={marginProduct.maxLeverage} onChange={(event) => setMarginProduct({ ...marginProduct, maxLeverage: event.currentTarget.value })} /></label>
            <label>最小保证金<input value={marginProduct.minMargin} onChange={(event) => setMarginProduct({ ...marginProduct, minMargin: event.currentTarget.value })} /></label>
            <label>最大保证金<input value={marginProduct.maxMargin} onChange={(event) => setMarginProduct({ ...marginProduct, maxMargin: event.currentTarget.value })} /></label>
            <label>维持保证金率<input value={marginProduct.maintenanceMarginRate} onChange={(event) => setMarginProduct({ ...marginProduct, maintenanceMarginRate: event.currentTarget.value })} /></label>
            <label>小时利率<input value={marginProduct.hourlyInterestRate} onChange={(event) => setMarginProduct({ ...marginProduct, hourlyInterestRate: event.currentTarget.value })} /></label>
            <label>
              初始状态
              <select value={marginProduct.status} onChange={(event) => setMarginProduct({ ...marginProduct, status: event.currentTarget.value })}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
          </div>
          <ConfirmAction
            actionText="提交添加杠杆交易对"
            title="确认添加杠杆交易对"
            onConfirm={(reason) =>
              submitAction('添加杠杆交易对', () =>
                apiRequest('/admin/api/v1/margin/products', {
                  method: 'POST',
                  body: JSON.stringify({
                    pair_id: requiredPositiveInteger(marginProduct.pairId, '杠杆交易对ID'),
                    margin_asset: requiredPositiveInteger(marginProduct.marginAsset, '保证金资产ID'),
                    max_leverage: requiredString(marginProduct.maxLeverage, '最大杠杆'),
                    min_margin: requiredString(marginProduct.minMargin, '最小保证金'),
                    max_margin: optionalString(marginProduct.maxMargin),
                    maintenance_margin_rate: requiredString(marginProduct.maintenanceMarginRate, '维持保证金率'),
                    hourly_interest_rate: optionalString(marginProduct.hourlyInterestRate),
                    status: marginProduct.status,
                    reason
                  })
                })
              )
            }
          />
        </Space>
      </Card>
    </FormModal>
  );
}

export function CreateSecondsPairAction() {
  const [secondsProduct, setSecondsProduct] = useState(initialSecondsProduct);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="添加秒合约交易对" title="添加秒合约交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加秒合约交易对</Title>
          <div className="admin-action-form">
            <label>秒合约交易对ID<input value={secondsProduct.pairId} onChange={(event) => setSecondsProduct({ ...secondsProduct, pairId: event.currentTarget.value })} /></label>
            <AssetSelect
              label="押注资产"
              loading={assetLoading}
              options={assetOptions}
              value={secondsProduct.stakeAsset}
              onChange={(stakeAsset) => setSecondsProduct({ ...secondsProduct, stakeAsset })}
            />
            <label>周期秒数<input value={secondsProduct.durationSeconds} onChange={(event) => setSecondsProduct({ ...secondsProduct, durationSeconds: event.currentTarget.value })} /></label>
            <label>赔率<input value={secondsProduct.payoutRate} onChange={(event) => setSecondsProduct({ ...secondsProduct, payoutRate: event.currentTarget.value })} /></label>
            <label>最小押注<input value={secondsProduct.minStake} onChange={(event) => setSecondsProduct({ ...secondsProduct, minStake: event.currentTarget.value })} /></label>
            <label>最大押注<input value={secondsProduct.maxStake} onChange={(event) => setSecondsProduct({ ...secondsProduct, maxStake: event.currentTarget.value })} /></label>
            <label>
              初始状态
              <select value={secondsProduct.status} onChange={(event) => setSecondsProduct({ ...secondsProduct, status: event.currentTarget.value })}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
          </div>
          <ConfirmAction
            actionText="提交添加秒合约交易对"
            title="确认添加秒合约交易对"
            onConfirm={(reason) =>
              submitAction('添加秒合约交易对', () =>
                apiRequest('/admin/api/v1/seconds-contracts/products', {
                  method: 'POST',
                  body: JSON.stringify({
                    pair_id: requiredPositiveInteger(secondsProduct.pairId, '秒合约交易对ID'),
                    stake_asset: requiredPositiveInteger(secondsProduct.stakeAsset, '押注资产ID'),
                    duration_seconds: requiredPositiveInteger(secondsProduct.durationSeconds, '周期秒数'),
                    payout_rate: requiredString(secondsProduct.payoutRate, '赔率'),
                    min_stake: requiredString(secondsProduct.minStake, '最小押注'),
                    max_stake: optionalString(secondsProduct.maxStake),
                    status: secondsProduct.status,
                    reason
                  })
                })
              )
            }
          />
        </Space>
      </Card>
    </FormModal>
  );
}
