import { Button, Card, SideSheet, Space, Tabs, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { AdminCheckbox, AdminSelect, AdminTextInput } from '../../../shared/SemiFormControls';
import {
  type AssetOption,
  AssetSelect,
  type CreateActionProps,
  FormModal,
  type MarketPairOption,
  MarketPairSelect,
  type RowActionHelpers,
  activeStatusOptions,
  completeCreate,
  createModalProps,
  includeCurrentOption,
  nextToggleStatus,
  openRecordDetail,
  optionalString,
  recordString,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  toggleActionText,
  useAssetOptions,
  useMarketPairOptions
} from './shared';

const { Text } = Typography;

type MarginProductValues = {
  pairId: string;
  marginAsset: string;
  logoUrl: string;
  marginModes: string[];
  leverageLevels: string[];
  customLeverageLevels: string;
  minMargin: string;
  maxMargin: string;
  maintenanceMarginRate: string;
  hourlyInterestRate: string;
  status: string;
};

type MarginProductTab = 'basic' | 'leverage' | 'risk';

const defaultLeverageLevels = ['2', '5', '10', '20', '30', '40', '50', '100', '200', '1000'];

const marginProductTabs = [
  { itemKey: 'basic', tab: '基础配置' },
  { itemKey: 'leverage', tab: '杠杆档位' },
  { itemKey: 'risk', tab: '风控参数' }
];

const initialMarginProduct: MarginProductValues = {
  pairId: '',
  marginAsset: '',
  logoUrl: '',
  marginModes: ['isolated'],
  leverageLevels: [],
  customLeverageLevels: '',
  minMargin: '',
  maxMargin: '',
  maintenanceMarginRate: '',
  hourlyInterestRate: '',
  status: 'active'
};

function marginLeverageLevels(values: MarginProductValues): string[] {
  const levels = [...values.leverageLevels, ...values.customLeverageLevels.split(',')]
    .map((level) => level.trim())
    .filter(Boolean)
    .filter((level) => Number.isFinite(Number(level)) && Number(level) > 1);

  return [...new Set(levels)].sort((left, right) => Number(left) - Number(right));
}

function isMarginProductCreatable(values: MarginProductValues): boolean {
  return Boolean(
      values.pairId.trim() &&
      values.marginAsset.trim() &&
      values.marginModes.length > 0 &&
      marginLeverageLevels(values).length > 0 &&
      values.minMargin.trim() &&
      values.maintenanceMarginRate.trim()
  );
}

function normalizedMarginLeverageLevel(value: string): string {
  const trimmed = value.trim();
  const numeric = Number(trimmed);
  return trimmed && Number.isFinite(numeric) ? String(numeric) : trimmed;
}

function marginProductFromRecord(record: ApiRecord): MarginProductValues {
  const marginModes = Array.isArray(record.margin_modes)
    ? record.margin_modes.filter((mode): mode is string => mode === 'isolated')
    : [];
  const leverageLevels = Array.isArray(record.leverage_levels)
    ? record.leverage_levels
        .filter((level) => typeof level === 'string' || typeof level === 'number')
        .map((level) => normalizedMarginLeverageLevel(String(level)))
        .filter(Boolean)
    : [];
  const defaultLevelSet = new Set(defaultLeverageLevels);

  return {
    pairId: recordString(record, 'pair_id'),
    marginAsset: recordString(record, 'margin_asset'),
    logoUrl: recordString(record, 'logo_url'),
    marginModes: marginModes.length > 0 ? marginModes : ['isolated'],
    leverageLevels: leverageLevels.filter((level) => defaultLevelSet.has(level)),
    customLeverageLevels: leverageLevels.filter((level) => !defaultLevelSet.has(level)).join(','),
    minMargin: recordString(record, 'min_margin'),
    maxMargin: recordString(record, 'max_margin'),
    maintenanceMarginRate: recordString(record, 'maintenance_margin_rate'),
    hourlyInterestRate: recordString(record, 'hourly_interest_rate'),
    status: recordString(record, 'status') || 'active'
  };
}

function marginProductRequestBody(values: MarginProductValues, reason: string) {
  const leverageLevels = marginLeverageLevels(values);
  const maxLeverage = leverageLevels.at(-1);
  if (!maxLeverage) {
    throw new Error('杠杆档位不能为空');
  }

  return {
    pair_id: requiredPositiveInteger(values.pairId, '杠杆交易对ID'),
    margin_asset: requiredPositiveInteger(values.marginAsset, '保证金资产ID'),
    logo_url: optionalString(values.logoUrl),
    margin_modes: ['isolated'],
    leverage_levels: leverageLevels,
    max_leverage: maxLeverage,
    min_margin: requiredString(values.minMargin, '最小保证金'),
    max_margin: optionalString(values.maxMargin),
    maintenance_margin_rate: requiredString(values.maintenanceMarginRate, '维持保证金率'),
    hourly_interest_rate: optionalString(values.hourlyInterestRate),
    status: values.status,
    reason
  };
}

function MarginProductFields({
  activeTab,
  assetLoading,
  assetOptions,
  onActiveTabChange,
  onChange,
  pairLoading,
  pairOptions,
  statusLabel,
  values
}: {
  activeTab: MarginProductTab;
  assetLoading: boolean;
  assetOptions: AssetOption[];
  onActiveTabChange: (tab: MarginProductTab) => void;
  onChange: (values: MarginProductValues) => void;
  pairLoading: boolean;
  pairOptions: MarketPairOption[];
  statusLabel: string;
  values: MarginProductValues;
}) {
  const selectedLeverageLevels = marginLeverageLevels(values);

  return (
    <>
      <Tabs activeKey={activeTab} onChange={(nextTab) => onActiveTabChange(nextTab as MarginProductTab)} tabList={marginProductTabs} type="button" style={{ width: '100%' }} />
      {activeTab === 'basic' ? (
        <div className="admin-action-form admin-action-form-wide">
          <MarketPairSelect
            label="杠杆交易对"
            loading={pairLoading}
            options={pairOptions}
            value={values.pairId}
            onChange={(pairId) => onChange({ ...values, pairId })}
          />
          <AssetSelect
            label="保证金资产"
            loading={assetLoading}
            options={assetOptions}
            value={values.marginAsset}
            onChange={(marginAsset) => onChange({ ...values, marginAsset })}
          />
          <AdminImageUpload label="杠杆交易对 Logo" value={values.logoUrl} variant="avatar" onChange={(logoUrl) => onChange({ ...values, logoUrl })} />
          <label>
            支持保证金模式
            <AdminTextInput
              ariaLabel="支持保证金模式"
              readOnly
              onChange={() => undefined}
              value="逐仓"
            />
          </label>
          <label>
            {statusLabel}
            <AdminSelect ariaLabel={statusLabel} onChange={(status) => onChange({ ...values, status })} optionList={activeStatusOptions} value={values.status} />
          </label>
        </div>
      ) : activeTab === 'leverage' ? (
        <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
          <fieldset className="admin-action-choice-group">
            <legend>杠杆档位</legend>
            <div className="admin-action-choice-list">
              {defaultLeverageLevels.map((level) => (
                <div className="admin-action-checkbox" key={level}>
                  <AdminCheckbox checked={values.leverageLevels.includes(level)} onChange={() => onChange(toggleLeverageLevel(values, level))}>{level}x</AdminCheckbox>
                </div>
              ))}
            </div>
          </fieldset>
          <div className="admin-action-form admin-action-form-wide">
            <label>
              自定义杠杆档位
              <AdminTextInput ariaLabel="自定义杠杆档位" value={values.customLeverageLevels} onChange={(customLeverageLevels) => onChange({ ...values, customLeverageLevels })} placeholder="25,125" />
            </label>
          </div>
          <Text type={selectedLeverageLevels.length ? 'secondary' : 'danger'}>已选杠杆：{selectedLeverageLevels.length ? `${selectedLeverageLevels.join('x / ')}x` : '未选择'}</Text>
        </Space>
      ) : (
        <div className="admin-action-form admin-action-form-wide">
          <label>最小保证金<AdminTextInput ariaLabel="最小保证金" value={values.minMargin} onChange={(minMargin) => onChange({ ...values, minMargin })} /></label>
          <label>最大保证金<AdminTextInput ariaLabel="最大保证金" value={values.maxMargin} onChange={(maxMargin) => onChange({ ...values, maxMargin })} /></label>
          <label>维持保证金率<AdminTextInput ariaLabel="维持保证金率" value={values.maintenanceMarginRate} onChange={(maintenanceMarginRate) => onChange({ ...values, maintenanceMarginRate })} /></label>
          <label>小时利率<AdminTextInput ariaLabel="小时利率" value={values.hourlyInterestRate} onChange={(hourlyInterestRate) => onChange({ ...values, hourlyInterestRate })} /></label>
        </div>
      )}
    </>
  );
}

function toggleLeverageLevel(values: MarginProductValues, level: string): MarginProductValues {
  const selected = values.leverageLevels.includes(level);
  return {
    ...values,
    leverageLevels: selected ? values.leverageLevels.filter((item) => item !== level) : [...values.leverageLevels, level]
  };
}

function MarginProductEditAction({ helpers, productId, record }: { helpers: RowActionHelpers; productId: string; record: ApiRecord }) {
  const [config, setConfig] = useState(() => marginProductFromRecord(record));
  const [activeTab, setActiveTab] = useState<MarginProductTab>('basic');
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { pairLoading, pairOptions } = useMarketPairOptions(visible);
  const pairOptionsWithCurrent = includeCurrentOption(pairOptions, config.pairId, `${recordString(record, 'symbol') || `交易对${config.pairId}`}（ID: ${config.pairId}）`);
  const assetOptionsWithCurrent = includeCurrentOption(
    assetOptions,
    config.marginAsset,
    `${recordString(record, 'margin_asset_symbol') || `资产${config.marginAsset}`}（ID: ${config.marginAsset}）`
  );

  return (
    <>
      <Button disabled={!productId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改杠杆产品" visible={visible} {...createModalProps('extra-wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <MarginProductFields
              activeTab={activeTab}
              assetLoading={assetLoading}
              assetOptions={assetOptionsWithCurrent}
              onActiveTabChange={setActiveTab}
              onChange={setConfig}
              pairLoading={pairLoading}
              pairOptions={pairOptionsWithCurrent}
              statusLabel="状态"
              values={config}
            />
            <div className="admin-action-footer">
              <ConfirmAction
                actionText="提交修改"
                disabled={!isMarginProductCreatable(config)}
                title="确认修改杠杆产品"
                onConfirm={async (reason) => {
                  await submitAction('修改杠杆产品', () =>
                    apiRequest(`/admin/api/v1/margin/products/${productId}`, {
                      method: 'PATCH',
                      body: JSON.stringify(marginProductRequestBody(config, reason))
                    })
                  );
                  setVisible(false);
                  helpers.reload();
                }}
              />
            </div>
          </Space>
        </Card>
      </SideSheet>
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
      <MarginProductEditAction helpers={helpers} productId={productId} record={record} />
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

export function CreateMarginPairAction({ onCreated }: CreateActionProps = {}) {
  const [marginProduct, setMarginProduct] = useState(initialMarginProduct);
  const [activeTab, setActiveTab] = useState<MarginProductTab>('basic');
  const { assetLoading, assetOptions } = useAssetOptions();
  const { pairLoading, pairOptions } = useMarketPairOptions();

  return (
    <FormModal actionText="添加杠杆交易对" size="extra-wide" title="添加杠杆交易对">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <MarginProductFields
            activeTab={activeTab}
            assetLoading={assetLoading}
            assetOptions={assetOptions}
            onActiveTabChange={setActiveTab}
            onChange={setMarginProduct}
            pairLoading={pairLoading}
            pairOptions={pairOptions}
            statusLabel="初始状态"
            values={marginProduct}
          />
          <div className="admin-action-footer">
            <ConfirmAction
              actionText="提交添加杠杆交易对"
              disabled={!isMarginProductCreatable(marginProduct)}
              title="确认添加杠杆交易对"
              onConfirm={async (reason) => {
                await submitAction('添加杠杆交易对', () =>
                  apiRequest('/admin/api/v1/margin/products', {
                    method: 'POST',
                    body: JSON.stringify(marginProductRequestBody(marginProduct, reason))
                  })
                );
                completeCreate(close, onCreated, () => {
                  setMarginProduct(initialMarginProduct);
                  setActiveTab('basic');
                });
              }}
            />
          </div>
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
