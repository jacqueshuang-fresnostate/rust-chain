import { Card, Modal, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { type ReactNode, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { ConfirmAction } from '../../shared/ConfirmAction';

const { Text, Title } = Typography;

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

export function CreateAssetAction() {
  const [asset, setAsset] = useState(initialAsset);

  return (
    <FormModal actionText="添加资产" title="添加资产">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div>
            <Title heading={4}>添加资产</Title>
            <Text type="secondary">资产创建后可作为交易对、钱包账户、闪兑和产品配置的基础资产。</Text>
          </div>
          <div className="admin-action-form">
            <label>资产符号<input value={asset.symbol} onChange={(event) => setAsset({ ...asset, symbol: event.currentTarget.value })} placeholder="BTC" /></label>
            <label>资产名称<input value={asset.name} onChange={(event) => setAsset({ ...asset, name: event.currentTarget.value })} placeholder="Bitcoin" /></label>
            <label>资产精度<input value={asset.precisionScale} onChange={(event) => setAsset({ ...asset, precisionScale: event.currentTarget.value })} /></label>
            <label>
              资产类型
              <select value={asset.assetType} onChange={(event) => setAsset({ ...asset, assetType: event.currentTarget.value })}>
                <option value="coin">coin</option>
                <option value="stablecoin">stablecoin</option>
                <option value="fiat">fiat</option>
                <option value="platform">platform</option>
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

  return (
    <FormModal actionText="添加交易对" title="添加现货交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div>
            <Title heading={4}>添加现货交易对</Title>
            <Text type="secondary">现货交易对创建后可被杠杆、秒合约产品复用。</Text>
          </div>
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

  return (
    <FormModal actionText="添加杠杆交易对" title="添加杠杆交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div>
            <Title heading={4}>添加杠杆交易对</Title>
            <Text type="secondary">需先在交易对配置中创建现货交易对，再用 pair_id 开通杠杆交易。</Text>
          </div>
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

  return (
    <FormModal actionText="添加秒合约交易对" title="添加秒合约交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div>
            <Title heading={4}>添加秒合约交易对</Title>
            <Text type="secondary">需先在交易对配置中创建现货交易对，再配置周期、赔率和押注资产。</Text>
          </div>
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
