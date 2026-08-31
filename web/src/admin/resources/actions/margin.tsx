import { Button, Card, SideSheet, Space, Tabs, Typography } from '@douyinfe/semi-ui';
import { useState, type ReactNode } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { AdminRequestActionBoundary } from '../../access';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { canonicalDecimalText, compareDecimalText, isNonNegativeDecimalText, isPositiveDecimalText } from '../../../shared/decimal';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { AdminCheckbox, AdminMultiSelect, AdminSelect, AdminTextInput } from '../../../shared/SemiFormControls';
import {
  type AssetOption,
  type CreateActionProps,
  FormModal,
  type MarketPairOption,
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

type MarginMode = 'isolated' | 'cross';

type MarginProductValues = {
  pairId: string;
  marginAsset: string;
  logoUrl: string;
  marginModes: MarginMode[];
  defaultMarginMode: MarginMode | '';
  leverageLevels: string[];
  customLeverageLevels: string;
  minMargin: string;
  maxMargin: string;
  maintenanceMarginRate: string;
  hourlyInterestRate: string;
  status: string;
};

type MarginProductTab = 'basic' | 'leverage' | 'risk' | 'review';

const defaultLeverageLevels = ['2', '5', '10', '20', '30', '40', '50', '100', '200', '1000'];

const marginProductTabs = [
  { itemKey: 'basic', tab: '基础配置' },
  { itemKey: 'leverage', tab: '杠杆档位' },
  { itemKey: 'risk', tab: '风控与计费' },
  { itemKey: 'review', tab: '发布确认' }
];

const marginModeOptions = [
  { label: '逐仓', value: 'isolated' },
  { label: '全仓', value: 'cross' }
] satisfies Array<{ label: string; value: MarginMode }>;

const marginModeLabels: Record<MarginMode, string> = {
  isolated: '逐仓',
  cross: '全仓'
};

const initialMarginProduct: MarginProductValues = {
  pairId: '',
  marginAsset: '',
  logoUrl: '',
  marginModes: ['isolated'],
  defaultMarginMode: 'isolated',
  leverageLevels: [],
  customLeverageLevels: '',
  minMargin: '',
  maxMargin: '',
  maintenanceMarginRate: '',
  hourlyInterestRate: '',
  status: 'active'
};

function isMarginMode(value: unknown): value is MarginMode {
  return value === 'isolated' || value === 'cross';
}

function uniqueMarginModes(modes: unknown[]): MarginMode[] {
  return [...new Set(modes.filter(isMarginMode))];
}

function updateSupportedMarginModes(values: MarginProductValues, nextModes: string[]): MarginProductValues {
  const marginModes = uniqueMarginModes(nextModes);
  return {
    ...values,
    marginModes,
    defaultMarginMode: marginModes.includes(values.defaultMarginMode as MarginMode) ? values.defaultMarginMode : marginModes[0] ?? ''
  };
}

const plainUnsignedDecimalPattern = /^(?:\d+(?:\.\d*)?|\.\d+)$/;

function isPlainUnsignedDecimal(value: string): boolean {
  return plainUnsignedDecimalPattern.test(value.trim());
}

function customLeverageLevelError(value: string): string | null {
  if (!value.trim()) {
    return null;
  }

  const invalidLevel = value
    .split(',')
    .map((level) => level.trim())
    .find((level) => !isPlainUnsignedDecimal(level) || compareDecimalText(level, '1') !== 1);

  return invalidLevel === undefined ? null : `自定义杠杆档位“${invalidLevel || '空项'}”必须为大于 1 的十进制数`;
}

function marginLeverageLevels(values: MarginProductValues): string[] {
  const levels = [...values.leverageLevels, ...values.customLeverageLevels.split(',')]
    .map((level) => level.trim())
    .filter(Boolean)
    .filter((level) => isPlainUnsignedDecimal(level) && compareDecimalText(level, '1') === 1)
    .map((level) => canonicalDecimalText(level) as string);

  return [...new Set(levels)].sort((left, right) => compareDecimalText(left, right) ?? 0);
}

function marginProductStepError(values: MarginProductValues, tab: Exclude<MarginProductTab, 'review'>): string | null {
  if (tab === 'basic') {
    if (!values.pairId.trim()) return '请选择杠杆交易对';
    if (!values.marginAsset.trim()) return '请选择保证金资产';
    if (values.marginModes.length === 0) return '请至少选择一种支持的保证金模式';
    if (!values.defaultMarginMode || !values.marginModes.includes(values.defaultMarginMode)) return '请选择已支持的默认保证金模式';
    return null;
  }

  if (tab === 'leverage') {
    const customError = customLeverageLevelError(values.customLeverageLevels);
    if (customError) return customError;
    if (marginLeverageLevels(values).length === 0) return '请至少选择一个杠杆档位';
    return null;
  }

  if (!isPlainUnsignedDecimal(values.minMargin) || !isPositiveDecimalText(values.minMargin)) return '最小保证金必须为大于 0 的十进制数';
  if (values.maxMargin.trim() && (!isPlainUnsignedDecimal(values.maxMargin) || !isPositiveDecimalText(values.maxMargin))) return '最大保证金必须为大于 0 的十进制数，或留空表示不设上限';
  if (values.maxMargin.trim() && compareDecimalText(values.maxMargin, values.minMargin) === -1) return '最大保证金不能小于最小保证金';
  if (!isPlainUnsignedDecimal(values.maintenanceMarginRate) || !isNonNegativeDecimalText(values.maintenanceMarginRate)) return '维持保证金率必须为非负十进制数';
  if (values.hourlyInterestRate.trim() && (!isPlainUnsignedDecimal(values.hourlyInterestRate) || !isNonNegativeDecimalText(values.hourlyInterestRate))) return '小时利率必须为非负十进制数，或留空';
  return null;
}

function marginProductWorkflowError(values: MarginProductValues, activeTab: MarginProductTab): string | null {
  const validationTabs: Array<Exclude<MarginProductTab, 'review'>> = ['basic', 'leverage', 'risk'];
  const lastIndex = activeTab === 'review' ? validationTabs.length - 1 : validationTabs.indexOf(activeTab);
  for (let index = 0; index <= lastIndex; index += 1) {
    const tab = validationTabs[index];
    const error = marginProductStepError(values, tab);
    if (error) return error;
  }
  return null;
}

function isMarginProductCreatable(values: MarginProductValues): boolean {
  return !marginProductWorkflowError(values, 'review');
}

function normalizedMarginLeverageLevel(value: string): string {
  return canonicalDecimalText(value) ?? value.trim();
}

function marginProductFromRecord(record: ApiRecord): MarginProductValues {
  const storedDefaultMode = isMarginMode(record.margin_mode) ? record.margin_mode : null;
  const storedMarginModes = Array.isArray(record.margin_modes) ? uniqueMarginModes(record.margin_modes) : [];
  const marginModes = uniqueMarginModes(storedDefaultMode ? [storedDefaultMode, ...storedMarginModes] : storedMarginModes);
  const supportedMarginModes: MarginMode[] = marginModes.length > 0 ? marginModes : ['isolated'];
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
    marginModes: supportedMarginModes,
    defaultMarginMode: storedDefaultMode && supportedMarginModes.includes(storedDefaultMode) ? storedDefaultMode : supportedMarginModes[0] ?? 'isolated',
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
  const validationError = marginProductWorkflowError(values, 'review');
  if (validationError) {
    throw new Error(validationError);
  }
  const leverageLevels = marginLeverageLevels(values);
  const maxLeverage = leverageLevels.at(-1);
  if (!maxLeverage) {
    throw new Error('杠杆档位不能为空');
  }
  const defaultMarginMode = values.defaultMarginMode as MarginMode;
  const marginModes = [defaultMarginMode, ...values.marginModes.filter((mode) => mode !== defaultMarginMode)];

  return {
    pair_id: requiredPositiveInteger(values.pairId, '杠杆交易对ID'),
    margin_asset: requiredPositiveInteger(values.marginAsset, '保证金资产ID'),
    logo_url: optionalString(values.logoUrl),
    margin_mode: defaultMarginMode,
    margin_modes: marginModes,
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
  submitAction,
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
  submitAction: ReactNode;
  values: MarginProductValues;
}) {
  const selectedLeverageLevels = marginLeverageLevels(values);
  const activeTabIndex = marginProductTabs.findIndex((tab) => tab.itemKey === activeTab);
  const currentError = marginProductWorkflowError(values, activeTab);
  const selectedPairLabel = pairOptions.find((option) => option.id === values.pairId)?.label ?? values.pairId;
  const selectedAssetLabel = assetOptions.find((option) => option.id === values.marginAsset)?.label ?? values.marginAsset;

  return (
    <div className="admin-margin-workflow">
      <Tabs className="admin-margin-workflow__tabs" activeKey={activeTab} onChange={(nextTab) => onActiveTabChange(nextTab as MarginProductTab)} tabList={marginProductTabs} type="button" style={{ width: '100%' }} />
      {activeTab === 'basic' ? (
        <div aria-labelledby="semiTabbasic" className="admin-action-form admin-action-form-wide admin-margin-workflow__panel" id="semiTabPanelbasic" role="tabpanel" tabIndex={0}>
          <label>
            杠杆交易对
            <AdminSelect
              ariaLabel="杠杆交易对"
              disabled={pairLoading}
              filter
              loading={pairLoading}
              onChange={(pairId) => onChange({ ...values, pairId })}
              optionList={pairOptions.map((pair) => ({ value: pair.id, label: pair.label }))}
              placeholder={pairLoading ? '加载交易对中...' : '请选择交易对'}
              value={values.pairId}
            />
          </label>
          <label>
            保证金资产
            <AdminSelect
              ariaLabel="保证金资产"
              disabled={assetLoading}
              filter
              loading={assetLoading}
              onChange={(marginAsset) => onChange({ ...values, marginAsset })}
              optionList={assetOptions.map((asset) => ({ value: asset.id, label: asset.label }))}
              placeholder={assetLoading ? '加载资产中...' : '请选择资产'}
              value={values.marginAsset}
            />
          </label>
          <AdminImageUpload label="杠杆交易对 Logo" value={values.logoUrl} variant="avatar" onChange={(logoUrl) => onChange({ ...values, logoUrl })} />
          <label>
            支持保证金模式
            <AdminMultiSelect ariaLabel="支持保证金模式" onChange={(marginModes) => onChange(updateSupportedMarginModes(values, marginModes))} optionList={marginModeOptions} value={values.marginModes} />
          </label>
          <label>
            默认保证金模式
            <AdminSelect
              ariaLabel="默认保证金模式"
              disabled={values.marginModes.length === 0}
              onChange={(defaultMarginMode) => {
                if (isMarginMode(defaultMarginMode) && values.marginModes.includes(defaultMarginMode)) onChange({ ...values, defaultMarginMode });
              }}
              optionList={marginModeOptions.filter((option) => values.marginModes.includes(option.value))}
              placeholder="请先选择支持模式"
              value={values.defaultMarginMode}
            />
          </label>
          <label>
            {statusLabel}
            <AdminSelect ariaLabel={statusLabel} onChange={(status) => onChange({ ...values, status })} optionList={activeStatusOptions} value={values.status} />
          </label>
        </div>
      ) : activeTab === 'leverage' ? (
        <div aria-labelledby="semiTableverage" className="admin-margin-workflow__panel" id="semiTabPanelleverage" role="tabpanel" tabIndex={0}>
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
            <Text strong>最大杠杆：{selectedLeverageLevels.length ? `${selectedLeverageLevels.at(-1)}x` : '待选择'}</Text>
          </Space>
        </div>
      ) : activeTab === 'risk' ? (
        <div aria-labelledby="semiTabrisk" className="admin-action-form admin-action-form-wide admin-margin-workflow__panel" id="semiTabPanelrisk" role="tabpanel" tabIndex={0}>
          <label>最小保证金<AdminTextInput ariaLabel="最小保证金" value={values.minMargin} onChange={(minMargin) => onChange({ ...values, minMargin })} /></label>
          <label>最大保证金<AdminTextInput ariaLabel="最大保证金" value={values.maxMargin} onChange={(maxMargin) => onChange({ ...values, maxMargin })} /></label>
          <label>维持保证金率<AdminTextInput ariaLabel="维持保证金率" value={values.maintenanceMarginRate} onChange={(maintenanceMarginRate) => onChange({ ...values, maintenanceMarginRate })} /></label>
          <label>小时利率<AdminTextInput ariaLabel="小时利率" value={values.hourlyInterestRate} onChange={(hourlyInterestRate) => onChange({ ...values, hourlyInterestRate })} /></label>
          <Text className="admin-margin-workflow__rate-hint" type="secondary">费率采用小数口径，例如 0.05 = 5%。</Text>
        </div>
      ) : (
        <div aria-labelledby="semiTabreview" className="admin-margin-workflow__panel admin-margin-workflow__review" id="semiTabPanelreview" role="tabpanel" tabIndex={0}>
          <div className="admin-margin-workflow__review-grid">
            <section><strong>基础配置</strong><dl><dt>交易对</dt><dd>{selectedPairLabel || '-'}</dd><dt>保证金资产</dt><dd>{selectedAssetLabel || '-'}</dd><dt>支持模式</dt><dd>{values.marginModes.map((mode) => marginModeLabels[mode]).join(' / ') || '-'}</dd><dt>默认模式</dt><dd>{values.defaultMarginMode ? marginModeLabels[values.defaultMarginMode] : '-'}</dd></dl></section>
            <section><strong>杠杆档位</strong><dl><dt>可选档位</dt><dd>{selectedLeverageLevels.length ? `${selectedLeverageLevels.join('x / ')}x` : '-'}</dd><dt>最大杠杆</dt><dd>{selectedLeverageLevels.length ? `${selectedLeverageLevels.at(-1)}x` : '-'}</dd></dl></section>
            <section><strong>风控与计费</strong><dl><dt>保证金范围</dt><dd>{values.minMargin || '-'} ～ {values.maxMargin || '不设上限'}</dd><dt>维持保证金率</dt><dd>{values.maintenanceMarginRate || '-'}</dd><dt>小时利率</dt><dd>{values.hourlyInterestRate || '未配置'}</dd><dt>状态</dt><dd>{values.status === 'active' ? '启用' : '禁用'}</dd></dl></section>
          </div>
          <div className="admin-margin-workflow__impact" role="note">
            {values.status === 'active' ? '启用后将立即开放新开仓。' : '禁用后将停止新开仓。'}配置变更仅影响后续开仓，不改写既有仓位。
          </div>
        </div>
      )}
      <div className="admin-margin-workflow__footer">
        <div aria-live="polite" className="admin-margin-workflow__validation" role={currentError ? 'alert' : undefined}>
          {currentError ? `当前流程未完成：${currentError}` : '当前步骤已完成'}
        </div>
        <Space>
          <Button disabled={activeTabIndex === 0} onClick={() => onActiveTabChange(marginProductTabs[activeTabIndex - 1].itemKey as MarginProductTab)}>上一步</Button>
          {activeTab === 'review' ? submitAction : (
            <Button disabled={Boolean(currentError)} onClick={() => onActiveTabChange(marginProductTabs[activeTabIndex + 1].itemKey as MarginProductTab)} theme="solid" type="primary">下一步</Button>
          )}
        </Space>
      </div>
    </div>
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
              submitAction={(
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
              )}
              values={config}
            />
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
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/margin/products/${productId}`} method="PATCH">
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
      </AdminRequestActionBoundary>
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
            submitAction={(
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
            )}
            values={marginProduct}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
