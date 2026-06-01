import { Card, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

const { Title } = Typography;

type ProductKind = 'earn' | 'margin' | 'seconds-contract';

type ProductValues = {
  kind: ProductKind;
  productId: string;
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

const initialValues: ProductValues = { kind: 'seconds-contract', productId: '', status: 'active' };
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

const endpoints: Record<ProductKind, string> = {
  earn: '/admin/api/v1/earn/products',
  margin: '/admin/api/v1/margin/products',
  'seconds-contract': '/admin/api/v1/seconds-contracts/products'
};

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

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
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

export function ProductStatusActions() {
  const [values, setValues] = useState(initialValues);
  const [spotPair, setSpotPair] = useState(initialSpotPair);
  const [marginProduct, setMarginProduct] = useState(initialMarginProduct);
  const [secondsProduct, setSecondsProduct] = useState(initialSecondsProduct);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="产品配置动作" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>创建现货交易对</Title>
            <div className="admin-action-form">
              <label>基础资产ID<input value={spotPair.baseAssetId} onChange={(event) => setSpotPair({ ...spotPair, baseAssetId: event.currentTarget.value })} /></label>
              <label>计价资产ID<input value={spotPair.quoteAssetId} onChange={(event) => setSpotPair({ ...spotPair, quoteAssetId: event.currentTarget.value })} /></label>
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
                  <option value="external">external</option>
                  <option value="internal">internal</option>
                  <option value="strategy">strategy</option>
                </select>
              </label>
            </div>
            <ConfirmAction
              actionText="创建现货交易对"
              disabled={!isSpotPairCreatable(spotPair)}
              title="确认创建现货交易对"
              onConfirm={(reason) =>
                submitAction('创建现货交易对', () =>
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

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>创建杠杆产品</Title>
            <div className="admin-action-form">
              <label>杠杆交易对ID<input value={marginProduct.pairId} onChange={(event) => setMarginProduct({ ...marginProduct, pairId: event.currentTarget.value })} /></label>
              <label>保证金资产ID<input value={marginProduct.marginAsset} onChange={(event) => setMarginProduct({ ...marginProduct, marginAsset: event.currentTarget.value })} /></label>
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
              actionText="创建杠杆产品"
              title="确认创建杠杆产品"
              onConfirm={(reason) =>
                submitAction('创建杠杆产品', () =>
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

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>创建秒合约产品</Title>
            <div className="admin-action-form">
              <label>秒合约交易对ID<input value={secondsProduct.pairId} onChange={(event) => setSecondsProduct({ ...secondsProduct, pairId: event.currentTarget.value })} /></label>
              <label>押注资产ID<input value={secondsProduct.stakeAsset} onChange={(event) => setSecondsProduct({ ...secondsProduct, stakeAsset: event.currentTarget.value })} /></label>
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
              actionText="创建秒合约产品"
              title="确认创建秒合约产品"
              onConfirm={(reason) =>
                submitAction('创建秒合约产品', () =>
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

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>更新产品状态</Title>
            <div className="admin-action-form admin-action-form-narrow">
              <label>
                产品模块
                <select value={values.kind} onChange={(event) => setValues({ ...values, kind: event.currentTarget.value as ProductKind })}>
                  <option value="seconds-contract">秒合约产品</option>
                  <option value="margin">杠杆产品</option>
                  <option value="earn">理财产品</option>
                </select>
              </label>
              <label>产品ID<input value={values.productId} onChange={(event) => setValues({ ...values, productId: event.currentTarget.value })} /></label>
              <label>
                目标状态
                <select value={values.status} onChange={(event) => setValues({ ...values, status: event.currentTarget.value })}>
                  <option value="active">active</option>
                  <option value="disabled">disabled</option>
                </select>
              </label>
            </div>
            <ConfirmAction
              actionText="更新产品状态"
              title="确认更新产品状态"
              onConfirm={(reason) =>
                submitAction('更新产品状态', () =>
                  apiRequest(`${endpoints[values.kind]}/${requiredPositiveInteger(values.productId, '产品ID')}/status`, {
                    method: 'PATCH',
                    body: JSON.stringify({ status: values.status, reason })
                  })
                )
              }
            />
          </Space>
        </Card>
      </div>
    </main>
  );
}
