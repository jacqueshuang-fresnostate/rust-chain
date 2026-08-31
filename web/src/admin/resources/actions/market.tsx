import { Button, Card, SideSheet, Space } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { AdminRequestActionBoundary } from '../../access';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  AssetSelect,
  type CreateActionProps,
  FormModal,
  type RowActionHelpers,
  activeStatusOptions,
  completeCreate,
  createModalProps,
  isNonNegativeIntegerInput,
  openRecordDetail,
  optionalString,
  recordString,
  requiredNonNegativeInteger,
  requiredPositiveInteger,
  requiredString,
  statusOptions,
  submitAction,
  useAssetOptions
} from './shared';

export { CreateMarketStrategyAction, MarketStrategyRowActions } from './marketStrategy';

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

function canCancelSpotOrder(status: string): boolean {
  return status === 'pending' || status === 'open' || status === 'partially_filled';
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
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/market-pairs/${pairId}`} method="PATCH">
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
      </AdminRequestActionBoundary>
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
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/spot/orders/${orderId}/cancel`} method="POST">
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
      </AdminRequestActionBoundary>
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
