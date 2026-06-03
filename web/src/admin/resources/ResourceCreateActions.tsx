import { Button, Card, Modal, Space, Typography, Toast } from '@douyinfe/semi-ui';
import { type ReactNode, useEffect, useState } from 'react';

import { listAdminResource } from '../../api/adminResources';
import { ApiError, apiRequest } from '../../api/client';
import type { ApiRecord } from '../../api/types';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { QuillRichTextEditor, type RichTextValue } from '../../shared/QuillRichTextEditor';
import { AdminCheckbox, AdminModalTriggerButton, AdminPasswordInput, AdminSelect, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';

const { Title } = Typography;

type AssetValues = {
  symbol: string;
  name: string;
  precisionScale: string;
  assetType: string;
  status: string;
};

type AssetConfigValues = {
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
  marginMode: string;
  leverageLevels: string[];
  customLeverageLevels: string;
  minMargin: string;
  maxMargin: string;
  maintenanceMarginRate: string;
  hourlyInterestRate: string;
  status: string;
};

type ConvertPairValues = {
  fromAssetId: string;
  toAssetId: string;
  pricingMode: string;
  spreadRate: string;
  minAmount: string;
  maxAmount: string;
  enabled: string;
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
};

type EarnIntroductionItemValues = {
  content: RichTextValue;
  country: string;
  locale: string;
  title: string;
};

type EarnProductValues = {
  aprRate: string;
  assetId: string;
  category: string;
  introductions: EarnIntroductionItemValues[];
  maxSubscribe: string;
  minSubscribe: string;
  name: string;
  status: string;
  termDays: string;
};

type UserValues = {
  email: string;
  phone: string;
  password: string;
  status: string;
  kycLevel: string;
};

type UserRechargeValues = {
  assetId: string;
  amount: string;
};

type RiskRuleValues = {
  ruleType: string;
  targetType: string;
  targetId: string;
  configJson: string;
  enabled: string;
};

type NewCoinProjectValues = {
  assetId: string;
  symbol: string;
  lifecycleStatus: string;
  totalSupply: string;
  issuePrice: string;
  unlockType: string;
  listedAt: string;
  fixedUnlockAt: string;
  relativeUnlockSeconds: string;
  unlockFeeEnabled: string;
  unlockFeeRate: string;
  unlockFeeBasis: string;
  unlockFeeAsset: string;
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

type MarketPairOption = {
  id: string;
  label: string;
};

type RowActionHelpers = {
  reload: () => void;
  openDetail: (detail: { data: ApiRecord | ApiRecord[]; title?: string }) => void;
};

type CreateModalSize = 'medium' | 'wide' | 'extra-wide';

const createModalWidths: Record<CreateModalSize, string> = {
  medium: 'min(720px, calc(100vw - 48px))',
  wide: 'min(920px, calc(100vw - 48px))',
  'extra-wide': 'min(1120px, calc(100vw - 48px))'
};

function createModalProps(size: CreateModalSize) {
  return {
    bodyStyle: { maxHeight: 'calc(100vh - 180px)', overflowY: 'auto' as const },
    className: `admin-create-modal admin-create-modal-${size}`,
    width: createModalWidths[size]
  };
}

const initialUser: UserValues = {
  email: '',
  phone: '',
  password: '',
  status: 'active',
  kycLevel: '0'
};

const initialUserRecharge: UserRechargeValues = {
  assetId: '',
  amount: ''
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

const defaultLeverageLevels = ['2', '5', '10', '20', '30', '40', '50', '100', '200', '1000'];

const initialMarginProduct: MarginProductValues = {
  pairId: '',
  marginAsset: '',
  marginMode: 'isolated',
  leverageLevels: [],
  customLeverageLevels: '',
  minMargin: '',
  maxMargin: '',
  maintenanceMarginRate: '',
  hourlyInterestRate: '',
  status: 'active'
};

const initialConvertPair: ConvertPairValues = {
  fromAssetId: '',
  toAssetId: '',
  pricingMode: 'fixed',
  spreadRate: '',
  minAmount: '',
  maxAmount: '',
  enabled: 'true'
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
  status: 'draft'
};

const emptyRichTextValue: RichTextValue = [{ type: 'p', children: [{ text: '' }] }];

const initialEarnProduct: EarnProductValues = {
  assetId: '',
  name: '',
  category: 'fixed_term',
  termDays: '',
  aprRate: '',
  minSubscribe: '',
  maxSubscribe: '',
  status: 'active',
  introductions: [{ locale: 'zh-CN', country: 'CN', title: '', content: emptyRichTextValue }]
};

const initialRiskRule: RiskRuleValues = {
  ruleType: '',
  targetType: '',
  targetId: '',
  configJson: '{}',
  enabled: 'true'
};

const earnProductCategoryOptions: SemiSelectOption[] = [
  { value: 'fixed_term', label: '定期' },
  { value: 'flexible', label: '活期' },
  { value: 'structured', label: '结构化' },
  { value: 'staking', label: '质押' }
];

const activeStatusOptions: SemiSelectOption[] = [
  { value: 'active', label: 'active' },
  { value: 'disabled', label: 'disabled' }
];

const initialNewCoinProject: NewCoinProjectValues = {
  assetId: '',
  symbol: '',
  lifecycleStatus: 'preheat',
  totalSupply: '',
  issuePrice: '',
  unlockType: 'fixed_time',
  listedAt: '',
  fixedUnlockAt: '',
  relativeUnlockSeconds: '',
  unlockFeeEnabled: 'false',
  unlockFeeRate: '',
  unlockFeeBasis: 'market_value',
  unlockFeeAsset: ''
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

function requiredTimestamp(value: string, label: string): number {
  const parsed = requiredNonNegativeInteger(value, label);
  if (parsed <= 0) {
    throw new Error(`${label}必须为 Unix 毫秒时间戳`);
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

function marketPairOptionLabel(pair: ApiRecord): string {
  const id = assetFieldToString(pair, 'id');
  const symbol = assetFieldToString(pair, 'symbol') || `交易对${id}`;
  return `${symbol}（ID: ${id}）`;
}

function toMarketPairOption(pair: ApiRecord): MarketPairOption | null {
  const id = assetFieldToString(pair, 'id');
  return id ? { id, label: marketPairOptionLabel(pair) } : null;
}

function isUserCreatable(values: UserValues): boolean {
  return Boolean((values.email.trim() || values.phone.trim()) && values.password.trim() && values.status.trim() && isNonNegativeIntegerInput(values.kycLevel));
}

function isUserRechargeSubmittable(values: UserRechargeValues): boolean {
  return Boolean(values.assetId.trim() && values.amount.trim() && Number(values.amount) > 0);
}

function isAssetCreatable(values: AssetValues): boolean {
  return Boolean(values.symbol.trim() && values.name.trim() && isNonNegativeIntegerInput(values.precisionScale));
}

function isAssetConfigUpdatable(values: AssetConfigValues): boolean {
  return Boolean(values.name.trim() && isNonNegativeIntegerInput(values.precisionScale) && values.assetType.trim() && values.status.trim());
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
      values.marginMode.trim() &&
      marginLeverageLevels(values).length > 0 &&
      values.minMargin.trim() &&
      values.maintenanceMarginRate.trim()
  );
}

function isConvertPairCreatable(values: ConvertPairValues): boolean {
  return Boolean(values.fromAssetId.trim() && values.toAssetId.trim() && values.pricingMode.trim() && values.spreadRate.trim() && values.minAmount.trim());
}

function isMarketStrategySubmittable(values: MarketStrategyValues, includePairId: boolean): boolean {
  return Boolean(
    (!includePairId || values.pairId.trim()) &&
      values.strategyType.trim() &&
      values.startPrice.trim() &&
      values.targetPrice.trim() &&
      isNonNegativeIntegerInput(values.startTime) &&
      isNonNegativeIntegerInput(values.endTime) &&
      values.volatility.trim() &&
      values.volumeMin.trim() &&
      values.volumeMax.trim()
  );
}

function isEarnProductCreatable(values: EarnProductValues): boolean {
  return Boolean(
    values.assetId.trim() &&
      values.name.trim() &&
      values.category.trim() &&
      isNonNegativeIntegerInput(values.termDays) &&
      values.aprRate.trim() &&
      values.minSubscribe.trim() &&
      values.status.trim() &&
      values.introductions.length > 0 &&
      values.introductions.every((item) => item.locale.trim() && item.country.trim() && item.title.trim())
  );
}

function isRiskRuleCreatable(values: RiskRuleValues): boolean {
  return Boolean(values.ruleType.trim() && values.targetType.trim() && values.configJson.trim());
}

function isNewCoinProjectCreatable(values: NewCoinProjectValues): boolean {
  return Boolean(values.assetId.trim() && values.symbol.trim() && values.lifecycleStatus.trim() && values.totalSupply.trim() && values.issuePrice.trim() && values.unlockType.trim());
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

function FormModal({ actionText, children, size = 'medium', title }: { actionText: string; children: ReactNode; size?: CreateModalSize; title: string }) {
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>{actionText}</AdminModalTriggerButton>
      <Modal footer={null} onCancel={() => setVisible(false)} title={title} visible={visible} {...createModalProps(size)}>
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

function useMarketPairOptions() {
  const [pairOptions, setPairOptions] = useState<MarketPairOption[]>([]);
  const [pairLoading, setPairLoading] = useState(false);

  useEffect(() => {
    let active = true;
    setPairLoading(true);

    listAdminResource('/admin/api/v1/market-pairs', 'pairs', { status: 'active', limit: 100 })
      .then((result) => {
        if (!active) {
          return;
        }

        setPairOptions(result.rows.map(toMarketPairOption).filter((pair): pair is MarketPairOption => pair !== null));
      })
      .catch(() => {
        if (active) {
          setPairOptions([]);
        }
      })
      .finally(() => {
        if (active) {
          setPairLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  return { pairLoading, pairOptions };
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
      <AdminSelect
        ariaLabel={label}
        disabled={loading}
        loading={loading}
        onChange={onChange}
        optionList={options.map((asset) => ({ value: asset.id, label: asset.label }))}
        placeholder={loading ? '加载资产中...' : '请选择资产'}
        value={value}
      />
    </label>
  );
}

function MarketPairSelect({
  label,
  loading,
  onChange,
  options,
  value
}: {
  label: string;
  loading: boolean;
  onChange: (value: string) => void;
  options: MarketPairOption[];
  value: string;
}) {
  return (
    <label>
      {label}
      <AdminSelect
        ariaLabel={label}
        disabled={loading}
        loading={loading}
        onChange={onChange}
        optionList={options.map((pair) => ({ value: pair.id, label: pair.label }))}
        placeholder={loading ? '加载交易对中...' : '请选择交易对'}
        value={value}
      />
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
    helpers.openDetail({ title: '详情', data: await apiRequest<ApiRecord>(`${endpoint}/${recordId}`) });
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

async function openUserAssets(userId: string, helpers: RowActionHelpers) {
  try {
    const result = await apiRequest<ApiRecord>(`/admin/api/v1/wallet/accounts?user_id=${userId}&include_empty=true&limit=100`);
    const accounts = Array.isArray(result.accounts) ? (result.accounts as ApiRecord[]) : [];
    helpers.openDetail({ title: '用户资产', data: accounts });
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

function nextToggleStatus(status: string): 'active' | 'disabled' {
  return status === 'active' ? 'disabled' : 'active';
}

function nextMarketStrategyStatus(status: string): 'active' | 'disabled' {
  return status === 'active' ? 'disabled' : 'active';
}

const assetTypeOptions: SemiSelectOption[] = [
  { value: 'coin', label: '数字货币' },
  { value: 'stablecoin', label: '稳定币' },
  { value: 'fiat', label: '法币' },
  { value: 'platform', label: '平台币' }
];

const statusOptions: SemiSelectOption[] = [
  { value: 'active', label: '启用' },
  { value: 'disabled', label: '禁用' }
];

const booleanOptions: SemiSelectOption[] = [
  { value: 'true', label: '启用' },
  { value: 'false', label: '禁用' }
];

const marketTypeOptions: SemiSelectOption[] = [
  { value: 'external', label: '外部行情' },
  { value: 'internal', label: '内部撮合' },
  { value: 'strategy', label: '策略行情' }
];

function AssetTypeSelect({ onChange, value }: { onChange: (value: string) => void; value: string }) {
  return <AdminSelect ariaLabel="资产类型" onChange={onChange} optionList={assetTypeOptions} value={value} />;
}

function AssetStatusSelect({ onChange, value }: { onChange: (value: string) => void; value: string }) {
  return <AdminSelect ariaLabel="状态" onChange={onChange} optionList={statusOptions} value={value} />;
}

function BooleanSelect({ label, onChange, value }: { label: string; onChange: (value: string) => void; value: string }) {
  return <AdminSelect ariaLabel={label} onChange={onChange} optionList={booleanOptions} value={value} />;
}

function booleanFromSelect(value: string): boolean {
  return value !== 'false';
}

function toggleLeverageLevel(values: MarginProductValues, level: string): MarginProductValues {
  const selected = values.leverageLevels.includes(level);
  return {
    ...values,
    leverageLevels: selected ? values.leverageLevels.filter((item) => item !== level) : [...values.leverageLevels, level]
  };
}

function optionalPositiveInteger(value: string, label: string): number | undefined {
  const trimmed = value.trim();
  return trimmed ? requiredPositiveInteger(trimmed, label) : undefined;
}

function toggleActionText(nextStatus: string): string {
  return nextStatus === 'disabled' ? '禁用' : '启用';
}

function newEarnIntroduction(): EarnIntroductionItemValues {
  return { locale: 'en-US', country: 'US', title: '', content: emptyRichTextValue };
}

function updateEarnIntroduction(values: EarnProductValues, index: number, patch: Partial<EarnIntroductionItemValues>): EarnProductValues {
  return {
    ...values,
    introductions: values.introductions.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item))
  };
}

function earnProductRequestBody(values: EarnProductValues, reason: string) {
  return {
    asset_id: requiredPositiveInteger(values.assetId, '理财资产'),
    name: requiredString(values.name, '产品名称'),
    category: requiredString(values.category, '产品分类'),
    introduction_json: {
      version: 1,
      default_locale: values.introductions[0]?.locale.trim() || 'zh-CN',
      items: values.introductions.map((item) => ({
        locale: requiredString(item.locale, '语言'),
        country: requiredString(item.country, '国家'),
        title: requiredString(item.title, '介绍标题'),
        content: item.content
      }))
    },
    term_days: requiredPositiveInteger(values.termDays, '期限天数'),
    apr_rate: requiredString(values.aprRate, '年化利率'),
    min_subscribe: requiredString(values.minSubscribe, '最小申购'),
    max_subscribe: optionalString(values.maxSubscribe),
    status: requiredString(values.status, '状态'),
    reason
  };
}

function AssetEditAction({ assetId, helpers, record }: { assetId: string; helpers: RowActionHelpers; record: ApiRecord }) {
  const [config, setConfig] = useState<AssetConfigValues>({
    name: recordString(record, 'name'),
    precisionScale: recordString(record, 'precision_scale'),
    assetType: recordString(record, 'asset_type') || 'coin',
    status: recordString(record, 'status') || 'active'
  });
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!assetId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <Modal footer={null} onCancel={() => setVisible(false)} title="修改资产配置" visible={visible}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>修改资产配置</Title>
            <div className="admin-action-form">
              <label>资产符号<AdminTextInput ariaLabel="资产符号" readOnly value={recordString(record, 'symbol')} onChange={() => undefined} /></label>
              <label>资产名称<AdminTextInput ariaLabel="资产名称" value={config.name} onChange={(name) => setConfig({ ...config, name })} /></label>
              <label>资产精度<AdminTextInput ariaLabel="资产精度" value={config.precisionScale} onChange={(precisionScale) => setConfig({ ...config, precisionScale })} /></label>
              <label>资产类型<AssetTypeSelect value={config.assetType} onChange={(assetType) => setConfig({ ...config, assetType })} /></label>
              <label>状态<AssetStatusSelect value={config.status} onChange={(status) => setConfig({ ...config, status })} /></label>
            </div>
            <ConfirmAction
              actionText="提交修改"
              disabled={!isAssetConfigUpdatable(config)}
              title="确认修改资产配置"
              onConfirm={async (reason) => {
                await submitAction('修改资产配置', () =>
                  apiRequest(`/admin/api/v1/assets/${assetId}`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      name: requiredString(config.name, '资产名称'),
                      precision_scale: requiredNonNegativeInteger(config.precisionScale, '资产精度'),
                      asset_type: requiredString(config.assetType, '资产类型'),
                      status: requiredString(config.status, '状态'),
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

export function AssetRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const assetId = recordString(record, 'id');

  return (
    <>
      <Button disabled={!assetId} onClick={() => openRecordDetail('/admin/api/v1/assets', assetId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AssetEditAction assetId={assetId} helpers={helpers} record={record} />
    </>
  );
}

function UserRechargeAction({ helpers, userId }: { helpers: RowActionHelpers; userId: string }) {
  const [recharge, setRecharge] = useState(initialUserRecharge);
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <>
      <Button disabled={!userId} onClick={() => setVisible(true)} size="small" theme="borderless">
        充值
      </Button>
      <Modal footer={null} onCancel={() => setVisible(false)} title="用户充值" visible={visible}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>用户充值</Title>
            <div className="admin-action-form">
              <label>用户ID<AdminTextInput ariaLabel="用户ID" readOnly value={userId} onChange={() => undefined} /></label>
              <AssetSelect label="充值资产" loading={assetLoading} options={assetOptions} value={recharge.assetId} onChange={(assetId) => setRecharge({ ...recharge, assetId })} />
              <label>充值金额<AdminTextInput ariaLabel="充值金额" value={recharge.amount} onChange={(amount) => setRecharge({ ...recharge, amount })} /></label>
            </div>
            <ConfirmAction
              actionText="提交充值"
              disabled={!isUserRechargeSubmittable(recharge)}
              title="确认用户充值"
              onConfirm={async (reason) => {
                await submitAction('用户充值', () =>
                  apiRequest(`/admin/api/v1/users/${userId}/recharge`, {
                    method: 'POST',
                    body: JSON.stringify({
                      asset_id: requiredPositiveInteger(recharge.assetId, '充值资产'),
                      amount: requiredString(recharge.amount, '充值金额'),
                      reason
                    })
                  })
                );
                setVisible(false);
                setRecharge(initialUserRecharge);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </Modal>
    </>
  );
}

export function UserRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const userId = recordString(record, 'id');

  return (
    <>
      <Button disabled={!userId} onClick={() => openRecordDetail('/admin/api/v1/users', userId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <Button disabled={!userId} onClick={() => openUserAssets(userId, helpers)} size="small" theme="borderless">
        查看资产
      </Button>
      <UserRechargeAction helpers={helpers} userId={userId} />
    </>
  );
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
              <label>交易对<AdminTextInput ariaLabel="交易对" readOnly value={recordString(record, 'symbol')} onChange={() => undefined} /></label>
              <label>基础资产<AdminTextInput ariaLabel="基础资产" readOnly value={recordString(record, 'base_asset')} onChange={() => undefined} /></label>
              <label>计价资产<AdminTextInput ariaLabel="计价资产" readOnly value={recordString(record, 'quote_asset')} onChange={() => undefined} /></label>
              <label>当前状态<AdminTextInput ariaLabel="当前状态" readOnly value={recordString(record, 'status')} onChange={() => undefined} /></label>
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

export function CreateEarnProductAction({ onCreated }: { onCreated?: () => void }) {
  const [product, setProduct] = useState(initialEarnProduct);
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加理财产品</AdminModalTriggerButton>
      <Modal footer={null} onCancel={() => setVisible(false)} title="添加理财产品" visible={visible} {...createModalProps('extra-wide')}>
        <Card bordered={false}>
          <div className="admin-earn-product-layout">
            <section className="admin-earn-product-section" aria-labelledby="earn-product-basic-title">
              <Title heading={4} id="earn-product-basic-title">
                基础信息
              </Title>
              <div className="admin-action-form admin-earn-product-basic-grid">
                <AssetSelect label="理财资产" loading={assetLoading} options={assetOptions} value={product.assetId} onChange={(assetId) => setProduct({ ...product, assetId })} />
                <label>产品名称<AdminTextInput ariaLabel="产品名称" value={product.name} onChange={(name) => setProduct({ ...product, name })} /></label>
                <label>
                  产品分类
                  <AdminSelect ariaLabel="产品分类" onChange={(category) => setProduct({ ...product, category })} optionList={earnProductCategoryOptions} value={product.category} />
                </label>
                <label>期限天数<AdminTextInput ariaLabel="期限天数" value={product.termDays} onChange={(termDays) => setProduct({ ...product, termDays })} /></label>
                <label>年化利率<AdminTextInput ariaLabel="年化利率" value={product.aprRate} onChange={(aprRate) => setProduct({ ...product, aprRate })} /></label>
                <label>最小申购<AdminTextInput ariaLabel="最小申购" value={product.minSubscribe} onChange={(minSubscribe) => setProduct({ ...product, minSubscribe })} /></label>
                <label>最大申购<AdminTextInput ariaLabel="最大申购" value={product.maxSubscribe} onChange={(maxSubscribe) => setProduct({ ...product, maxSubscribe })} /></label>
                <label>
                  初始状态
                  <AdminSelect ariaLabel="初始状态" onChange={(status) => setProduct({ ...product, status })} optionList={activeStatusOptions} value={product.status} />
                </label>
              </div>
            </section>
            <section className="admin-earn-product-section" aria-labelledby="earn-product-introduction-title">
              <div className="admin-earn-section-header">
                <Title heading={4} id="earn-product-introduction-title">
                  多国语言介绍
                </Title>
                <Button onClick={() => setProduct({ ...product, introductions: [...product.introductions, newEarnIntroduction()] })} theme="borderless">
                  新增语言介绍
                </Button>
              </div>
              <div className="admin-earn-introduction-list">
                {product.introductions.map((item, index) => (
                  <Card bordered className="admin-earn-introduction-card" key={index}>
                    <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                      <Title heading={5}>语言版本 {index + 1}</Title>
                      <div className="admin-action-form admin-earn-introduction-meta">
                        <label>语言<AdminTextInput ariaLabel="语言" value={item.locale} onChange={(locale) => setProduct((current) => updateEarnIntroduction(current, index, { locale }))} /></label>
                        <label>国家<AdminTextInput ariaLabel="国家" value={item.country} onChange={(country) => setProduct((current) => updateEarnIntroduction(current, index, { country }))} /></label>
                        <label>介绍标题<AdminTextInput ariaLabel="介绍标题" value={item.title} onChange={(title) => setProduct((current) => updateEarnIntroduction(current, index, { title }))} /></label>
                      </div>
                      <QuillRichTextEditor value={item.content} onChange={(content) => setProduct((current) => updateEarnIntroduction(current, index, { content }))} />
                    </Space>
                  </Card>
                ))}
              </div>
            </section>
            <div className="admin-earn-product-footer">
              <ConfirmAction
                actionText="提交添加理财产品"
                disabled={!isEarnProductCreatable(product)}
                title="确认添加理财产品"
                onConfirm={async (reason) => {
                  await submitAction('添加理财产品', () =>
                    apiRequest('/admin/api/v1/earn/products', {
                      method: 'POST',
                      body: JSON.stringify(earnProductRequestBody(product, reason))
                    })
                  );
                  setVisible(false);
                  setProduct(initialEarnProduct);
                  onCreated?.();
                }}
              />
            </div>
          </div>
        </Card>
      </Modal>
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

export function RiskRuleRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const ruleId = recordString(record, 'id');
  const enabled = record.enabled === true;
  const nextEnabled = !enabled;
  const actionText = enabled ? '禁用' : '启用';

  return (
    <ConfirmAction
      actionText={actionText}
      disabled={!ruleId}
      title={`${actionText}风控规则`}
      onConfirm={async (reason) => {
        await submitAction(`${actionText}风控规则`, () =>
          apiRequest(`/admin/api/v1/risk/rules/${ruleId}/status`, {
            method: 'PATCH',
            body: JSON.stringify({ enabled: nextEnabled, reason })
          })
        );
        helpers.reload();
      }}
    />
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

function marketStrategyFromRecord(record: ApiRecord): MarketStrategyValues {
  return {
    pairId: recordString(record, 'pair_id'),
    strategyType: recordString(record, 'strategy_type') || 'price_path',
    startPrice: recordString(record, 'start_price'),
    targetPrice: recordString(record, 'target_price'),
    startTime: recordString(record, 'start_time'),
    endTime: recordString(record, 'end_time'),
    volatility: recordString(record, 'volatility') || '0',
    volumeMin: recordString(record, 'volume_min') || '0',
    volumeMax: recordString(record, 'volume_max') || '0',
    status: recordString(record, 'status') || 'draft'
  };
}

function MarketStrategyForm({ includePairId, onChange, values }: { includePairId: boolean; onChange: (values: MarketStrategyValues) => void; values: MarketStrategyValues }) {
  return (
    <div className="admin-action-form">
      {includePairId ? <label>交易对ID<AdminTextInput ariaLabel="交易对ID" value={values.pairId} onChange={(pairId) => onChange({ ...values, pairId })} /></label> : null}
      {!includePairId ? <label>交易对ID<AdminTextInput ariaLabel="交易对ID" readOnly value={values.pairId} onChange={() => undefined} /></label> : null}
      <label>策略类型<AdminTextInput ariaLabel="策略类型" value={values.strategyType} onChange={(strategyType) => onChange({ ...values, strategyType })} /></label>
      <label>起始价<AdminTextInput ariaLabel="起始价" value={values.startPrice} onChange={(startPrice) => onChange({ ...values, startPrice })} /></label>
      <label>目标价<AdminTextInput ariaLabel="目标价" value={values.targetPrice} onChange={(targetPrice) => onChange({ ...values, targetPrice })} /></label>
      <label>开始时间戳<AdminTextInput ariaLabel="开始时间戳" value={values.startTime} onChange={(startTime) => onChange({ ...values, startTime })} /></label>
      <label>结束时间戳<AdminTextInput ariaLabel="结束时间戳" value={values.endTime} onChange={(endTime) => onChange({ ...values, endTime })} /></label>
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
              { value: 'draft', label: 'draft' },
              { value: 'active', label: 'active' },
              { value: 'paused', label: 'paused' },
              { value: 'disabled', label: 'disabled' }
            ]}
            value={values.status}
          />
        </label>
      ) : (
        <label>当前状态<AdminTextInput ariaLabel="当前状态" readOnly value={values.status} onChange={() => undefined} /></label>
      )}
    </div>
  );
}

export function MarketStrategyRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const strategyId = recordString(record, 'id');
  const nextStatus = nextMarketStrategyStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);
  const [config, setConfig] = useState(() => marketStrategyFromRecord(record));
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!strategyId} onClick={() => openRecordDetail('/admin/api/v1/market-strategies', strategyId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <Button disabled={!strategyId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <Modal footer={null} onCancel={() => setVisible(false)} title="修改行情策略" visible={visible}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>修改行情策略</Title>
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
                      start_time: requiredTimestamp(config.startTime, '开始时间'),
                      end_time: requiredTimestamp(config.endTime, '结束时间'),
                      volatility: requiredString(config.volatility, '波动率'),
                      volume_min: requiredString(config.volumeMin, '最小成交量'),
                      volume_max: requiredString(config.volumeMax, '最大成交量'),
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
      <Modal footer={null} onCancel={() => setVisible(false)} title="创建策略" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>创建策略</Title>
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
                      start_time: requiredTimestamp(strategy.startTime, '开始时间'),
                      end_time: requiredTimestamp(strategy.endTime, '结束时间'),
                      volatility: requiredString(strategy.volatility, '波动率'),
                      volume_min: requiredString(strategy.volumeMin, '最小成交量'),
                      volume_max: requiredString(strategy.volumeMax, '最大成交量'),
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
      </Modal>
    </>
  );
}

export function CreateUserAction() {
  const [user, setUser] = useState(initialUser);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加用户</AdminModalTriggerButton>
      <Modal footer={null} onCancel={() => setVisible(false)} title="添加用户" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>添加用户</Title>
            <div className="admin-action-form">
              <label>邮箱<AdminTextInput ariaLabel="邮箱" value={user.email} onChange={(email) => setUser({ ...user, email })} /></label>
              <label>手机号<AdminTextInput ariaLabel="手机号" value={user.phone} onChange={(phone) => setUser({ ...user, phone })} /></label>
              <label>登录密码<AdminPasswordInput ariaLabel="登录密码" value={user.password} onChange={(password) => setUser({ ...user, password })} /></label>
              <label>状态<AssetStatusSelect value={user.status} onChange={(status) => setUser({ ...user, status })} /></label>
              <label>KYC等级<AdminTextInput ariaLabel="KYC等级" value={user.kycLevel} onChange={(kycLevel) => setUser({ ...user, kycLevel })} /></label>
            </div>
            <ConfirmAction
              actionText="提交添加用户"
              disabled={!isUserCreatable(user)}
              title="确认添加用户"
              onConfirm={async (reason) => {
                await submitAction('添加用户', () =>
                  apiRequest('/admin/api/v1/users', {
                    method: 'POST',
                    body: JSON.stringify({
                      email: optionalString(user.email),
                      phone: optionalString(user.phone),
                      password: requiredString(user.password, '登录密码'),
                      status: requiredString(user.status, '状态'),
                      kyc_level: requiredNonNegativeInteger(user.kycLevel, 'KYC等级'),
                      reason
                    })
                  })
                );
                setVisible(false);
                setUser(initialUser);
              }}
            />
          </Space>
        </Card>
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
          <Title heading={4}>添加资产</Title>
          <div className="admin-action-form">
            <label>资产符号<AdminTextInput ariaLabel="资产符号" value={asset.symbol} onChange={(symbol) => setAsset({ ...asset, symbol })} placeholder="BTC" /></label>
            <label>资产名称<AdminTextInput ariaLabel="资产名称" value={asset.name} onChange={(name) => setAsset({ ...asset, name })} placeholder="Bitcoin" /></label>
            <label>资产精度<AdminTextInput ariaLabel="资产精度" value={asset.precisionScale} onChange={(precisionScale) => setAsset({ ...asset, precisionScale })} /></label>
            <label>资产类型<AssetTypeSelect value={asset.assetType} onChange={(assetType) => setAsset({ ...asset, assetType })} /></label>
            <label>初始状态<AssetStatusSelect value={asset.status} onChange={(status) => setAsset({ ...asset, status })} /></label>
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
    <FormModal actionText="添加交易对" size="wide" title="添加现货交易对">
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
            <label>交易对符号<AdminTextInput ariaLabel="交易对符号" value={spotPair.symbol} onChange={(symbol) => setSpotPair({ ...spotPair, symbol })} placeholder="BTC-USDT" /></label>
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
  const { pairLoading, pairOptions } = useMarketPairOptions();

  return (
    <FormModal actionText="添加杠杆交易对" size="extra-wide" title="添加杠杆交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加杠杆交易对</Title>
          <div className="admin-action-form">
            <MarketPairSelect
              label="杠杆交易对"
              loading={pairLoading}
              options={pairOptions}
              value={marginProduct.pairId}
              onChange={(pairId) => setMarginProduct({ ...marginProduct, pairId })}
            />
            <AssetSelect
              label="保证金资产"
              loading={assetLoading}
              options={assetOptions}
              value={marginProduct.marginAsset}
              onChange={(marginAsset) => setMarginProduct({ ...marginProduct, marginAsset })}
            />
            <label>
              保证金模式
              <AdminSelect
                ariaLabel="保证金模式"
                onChange={(marginMode) => setMarginProduct({ ...marginProduct, marginMode })}
                optionList={[
                  { value: 'isolated', label: '逐仓' },
                  { value: 'cross', label: '全仓' }
                ]}
                value={marginProduct.marginMode}
              />
            </label>
            <fieldset>
              <legend>杠杆档位</legend>
              {defaultLeverageLevels.map((level) => (
                <label key={level}>
                  <AdminCheckbox checked={marginProduct.leverageLevels.includes(level)} onChange={() => setMarginProduct(toggleLeverageLevel(marginProduct, level))}>{level}x</AdminCheckbox>
                </label>
              ))}
            </fieldset>
            <label>自定义杠杆档位<AdminTextInput ariaLabel="自定义杠杆档位" value={marginProduct.customLeverageLevels} onChange={(customLeverageLevels) => setMarginProduct({ ...marginProduct, customLeverageLevels })} placeholder="25,125" /></label>
            <label>最小保证金<AdminTextInput ariaLabel="最小保证金" value={marginProduct.minMargin} onChange={(minMargin) => setMarginProduct({ ...marginProduct, minMargin })} /></label>
            <label>最大保证金<AdminTextInput ariaLabel="最大保证金" value={marginProduct.maxMargin} onChange={(maxMargin) => setMarginProduct({ ...marginProduct, maxMargin })} /></label>
            <label>维持保证金率<AdminTextInput ariaLabel="维持保证金率" value={marginProduct.maintenanceMarginRate} onChange={(maintenanceMarginRate) => setMarginProduct({ ...marginProduct, maintenanceMarginRate })} /></label>
            <label>小时利率<AdminTextInput ariaLabel="小时利率" value={marginProduct.hourlyInterestRate} onChange={(hourlyInterestRate) => setMarginProduct({ ...marginProduct, hourlyInterestRate })} /></label>
            <label>
              初始状态
              <AdminSelect ariaLabel="初始状态" onChange={(status) => setMarginProduct({ ...marginProduct, status })} optionList={activeStatusOptions} value={marginProduct.status} />
            </label>
          </div>
          <ConfirmAction
            actionText="提交添加杠杆交易对"
            disabled={!isMarginProductCreatable(marginProduct)}
            title="确认添加杠杆交易对"
            onConfirm={(reason) => {
              const leverageLevels = marginLeverageLevels(marginProduct);
              const maxLeverage = leverageLevels.at(-1);
              if (!maxLeverage) {
                Toast.error('杠杆档位不能为空');
                return;
              }

              return submitAction('添加杠杆交易对', () =>
                apiRequest('/admin/api/v1/margin/products', {
                  method: 'POST',
                  body: JSON.stringify({
                    pair_id: requiredPositiveInteger(marginProduct.pairId, '杠杆交易对ID'),
                    margin_asset: requiredPositiveInteger(marginProduct.marginAsset, '保证金资产ID'),
                    margin_mode: requiredString(marginProduct.marginMode, '保证金模式'),
                    leverage_levels: leverageLevels,
                    max_leverage: maxLeverage,
                    min_margin: requiredString(marginProduct.minMargin, '最小保证金'),
                    max_margin: optionalString(marginProduct.maxMargin),
                    maintenance_margin_rate: requiredString(marginProduct.maintenanceMarginRate, '维持保证金率'),
                    hourly_interest_rate: optionalString(marginProduct.hourlyInterestRate),
                    status: marginProduct.status,
                    reason
                  })
                })
              );
            }}
          />
        </Space>
      </Card>
    </FormModal>
  );
}

export function CreateConvertPairAction() {
  const [convertPair, setConvertPair] = useState(initialConvertPair);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="添加闪兑交易对" size="wide" title="添加闪兑交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加闪兑交易对</Title>
          <div className="admin-action-form">
            <AssetSelect label="源资产" loading={assetLoading} options={assetOptions} value={convertPair.fromAssetId} onChange={(fromAssetId) => setConvertPair({ ...convertPair, fromAssetId })} />
            <AssetSelect label="目标资产" loading={assetLoading} options={assetOptions} value={convertPair.toAssetId} onChange={(toAssetId) => setConvertPair({ ...convertPair, toAssetId })} />
            <label>
              定价模式
              <AdminSelect
                ariaLabel="定价模式"
                onChange={(pricingMode) => setConvertPair({ ...convertPair, pricingMode })}
                optionList={[
                  { value: 'fixed', label: 'fixed' },
                  { value: 'market', label: 'market' }
                ]}
                value={convertPair.pricingMode}
              />
            </label>
            <label>价差率<AdminTextInput ariaLabel="价差率" value={convertPair.spreadRate} onChange={(spreadRate) => setConvertPair({ ...convertPair, spreadRate })} /></label>
            <label>最小金额<AdminTextInput ariaLabel="最小金额" value={convertPair.minAmount} onChange={(minAmount) => setConvertPair({ ...convertPair, minAmount })} /></label>
            <label>最大金额<AdminTextInput ariaLabel="最大金额" value={convertPair.maxAmount} onChange={(maxAmount) => setConvertPair({ ...convertPair, maxAmount })} /></label>
            <label>启用<BooleanSelect label="启用" value={convertPair.enabled} onChange={(enabled) => setConvertPair({ ...convertPair, enabled })} /></label>
          </div>
          <ConfirmAction
            actionText="提交添加闪兑交易对"
            disabled={!isConvertPairCreatable(convertPair)}
            title="确认添加闪兑交易对"
            onConfirm={(reason) =>
              submitAction('添加闪兑交易对', () =>
                apiRequest('/admin/api/v1/convert/pairs', {
                  method: 'POST',
                  body: JSON.stringify({
                    from_asset_id: requiredPositiveInteger(convertPair.fromAssetId, '源资产'),
                    to_asset_id: requiredPositiveInteger(convertPair.toAssetId, '目标资产'),
                    pricing_mode: requiredString(convertPair.pricingMode, '定价模式'),
                    spread_rate: requiredString(convertPair.spreadRate, '价差率'),
                    min_amount: requiredString(convertPair.minAmount, '最小金额'),
                    max_amount: optionalString(convertPair.maxAmount),
                    enabled: booleanFromSelect(convertPair.enabled),
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

export function CreateRiskRuleAction() {
  const [riskRule, setRiskRule] = useState(initialRiskRule);

  return (
    <FormModal actionText="添加风控规则" size="wide" title="添加风控规则">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加风控规则</Title>
          <div className="admin-action-form">
            <label>规则类型<AdminTextInput ariaLabel="规则类型" value={riskRule.ruleType} onChange={(ruleType) => setRiskRule({ ...riskRule, ruleType })} /></label>
            <label>对象类型<AdminTextInput ariaLabel="对象类型" value={riskRule.targetType} onChange={(targetType) => setRiskRule({ ...riskRule, targetType })} /></label>
            <label>对象ID<AdminTextInput ariaLabel="对象ID" value={riskRule.targetId} onChange={(targetId) => setRiskRule({ ...riskRule, targetId })} /></label>
            <label>规则配置JSON<AdminTextArea ariaLabel="规则配置JSON" autosize value={riskRule.configJson} onChange={(configJson) => setRiskRule({ ...riskRule, configJson })} /></label>
            <label>启用<BooleanSelect label="启用" value={riskRule.enabled} onChange={(enabled) => setRiskRule({ ...riskRule, enabled })} /></label>
          </div>
          <ConfirmAction
            actionText="提交添加风控规则"
            disabled={!isRiskRuleCreatable(riskRule)}
            title="确认添加风控规则"
            onConfirm={(reason) => {
              let configJson: unknown;
              try {
                configJson = JSON.parse(riskRule.configJson);
              } catch {
                Toast.error('规则配置JSON格式错误');
                return;
              }

              return submitAction('添加风控规则', () =>
                apiRequest('/admin/api/v1/risk/rules', {
                  method: 'POST',
                  body: JSON.stringify({
                    rule_type: requiredString(riskRule.ruleType, '规则类型'),
                    target_type: requiredString(riskRule.targetType, '对象类型'),
                    target_id: optionalString(riskRule.targetId),
                    config_json: configJson,
                    enabled: booleanFromSelect(riskRule.enabled),
                    reason
                  })
                })
              );
            }}
          />
        </Space>
      </Card>
    </FormModal>
  );
}

export function CreateNewCoinProjectAction() {
  const [project, setProject] = useState(initialNewCoinProject);
  const { assetLoading, assetOptions } = useAssetOptions();
  const unlockFeeEnabled = booleanFromSelect(project.unlockFeeEnabled);

  return (
    <FormModal actionText="添加新币项目" size="extra-wide" title="添加新币项目">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加新币项目</Title>
          <div className="admin-action-form">
            <AssetSelect label="项目资产" loading={assetLoading} options={assetOptions} value={project.assetId} onChange={(assetId) => setProject({ ...project, assetId })} />
            <label>项目符号<AdminTextInput ariaLabel="项目符号" value={project.symbol} onChange={(symbol) => setProject({ ...project, symbol })} /></label>
            <label>
              生命周期
              <AdminSelect
                ariaLabel="生命周期"
                onChange={(lifecycleStatus) => setProject({ ...project, lifecycleStatus })}
                optionList={[
                  { value: 'preheat', label: 'preheat' },
                  { value: 'subscription', label: 'subscription' },
                  { value: 'distribution', label: 'distribution' },
                  { value: 'listed', label: 'listed' }
                ]}
                value={project.lifecycleStatus}
              />
            </label>
            <label>发行总量<AdminTextInput ariaLabel="发行总量" value={project.totalSupply} onChange={(totalSupply) => setProject({ ...project, totalSupply })} /></label>
            <label>发行价<AdminTextInput ariaLabel="发行价" value={project.issuePrice} onChange={(issuePrice) => setProject({ ...project, issuePrice })} /></label>
            <label>
              解禁类型
              <AdminSelect
                ariaLabel="解禁类型"
                onChange={(unlockType) => setProject({ ...project, unlockType })}
                optionList={[
                  { value: 'immediate_on_listing', label: 'immediate_on_listing' },
                  { value: 'fixed_time', label: 'fixed_time' },
                  { value: 'relative_period', label: 'relative_period' }
                ]}
                value={project.unlockType}
              />
            </label>
            {project.unlockType === 'immediate_on_listing' ? <label>上市时间<AdminTextInput ariaLabel="上市时间" value={project.listedAt} onChange={(listedAt) => setProject({ ...project, listedAt })} /></label> : null}
            {project.unlockType === 'fixed_time' ? <label>固定解禁时间<AdminTextInput ariaLabel="固定解禁时间" value={project.fixedUnlockAt} onChange={(fixedUnlockAt) => setProject({ ...project, fixedUnlockAt })} /></label> : null}
            {project.unlockType === 'relative_period' ? <label>相对解禁秒数<AdminTextInput ariaLabel="相对解禁秒数" value={project.relativeUnlockSeconds} onChange={(relativeUnlockSeconds) => setProject({ ...project, relativeUnlockSeconds })} /></label> : null}
            <label>启用解禁矿工费<BooleanSelect label="启用解禁矿工费" value={project.unlockFeeEnabled} onChange={(unlockFeeEnabledValue) => setProject({ ...project, unlockFeeEnabled: unlockFeeEnabledValue })} /></label>
            {unlockFeeEnabled ? (
              <>
                <label>解禁费率<AdminTextInput ariaLabel="解禁费率" value={project.unlockFeeRate} onChange={(unlockFeeRate) => setProject({ ...project, unlockFeeRate })} /></label>
                <label>解禁费计费基准<AdminTextInput ariaLabel="解禁费计费基准" value={project.unlockFeeBasis} onChange={(unlockFeeBasis) => setProject({ ...project, unlockFeeBasis })} /></label>
                <AssetSelect label="解禁费资产" loading={assetLoading} options={assetOptions} value={project.unlockFeeAsset} onChange={(unlockFeeAsset) => setProject({ ...project, unlockFeeAsset })} />
              </>
            ) : null}
          </div>
          <ConfirmAction
            actionText="提交添加新币项目"
            disabled={!isNewCoinProjectCreatable(project)}
            title="确认添加新币项目"
            onConfirm={(reason) => {
              const body: Record<string, unknown> = {
                asset_id: requiredPositiveInteger(project.assetId, '项目资产'),
                symbol: requiredString(project.symbol, '项目符号'),
                lifecycle_status: requiredString(project.lifecycleStatus, '生命周期'),
                total_supply: requiredString(project.totalSupply, '发行总量'),
                issue_price: requiredString(project.issuePrice, '发行价'),
                unlock_type: requiredString(project.unlockType, '解禁类型'),
                unlock_fee_enabled: unlockFeeEnabled,
                reason
              };
              if (project.unlockType === 'immediate_on_listing') {
                body.listed_at = requiredPositiveInteger(project.listedAt, '上市时间');
              }
              if (project.unlockType === 'fixed_time') {
                body.fixed_unlock_at = requiredPositiveInteger(project.fixedUnlockAt, '固定解禁时间');
              }
              if (project.unlockType === 'relative_period') {
                body.relative_unlock_seconds = requiredPositiveInteger(project.relativeUnlockSeconds, '相对解禁秒数');
              }
              if (unlockFeeEnabled) {
                body.unlock_fee_rate = requiredString(project.unlockFeeRate, '解禁费率');
                body.unlock_fee_basis = requiredString(project.unlockFeeBasis, '解禁费计费基准');
                body.unlock_fee_asset = optionalPositiveInteger(project.unlockFeeAsset, '解禁费资产');
              }

              return submitAction('添加新币项目', () =>
                apiRequest('/admin/api/v1/new-coins', {
                  method: 'POST',
                  body: JSON.stringify(body)
                })
              );
            }}
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
    <FormModal actionText="添加秒合约交易对" size="wide" title="添加秒合约交易对">
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Title heading={4}>添加秒合约交易对</Title>
          <div className="admin-action-form">
            <label>秒合约交易对ID<AdminTextInput ariaLabel="秒合约交易对ID" value={secondsProduct.pairId} onChange={(pairId) => setSecondsProduct({ ...secondsProduct, pairId })} /></label>
            <AssetSelect
              label="押注资产"
              loading={assetLoading}
              options={assetOptions}
              value={secondsProduct.stakeAsset}
              onChange={(stakeAsset) => setSecondsProduct({ ...secondsProduct, stakeAsset })}
            />
            <label>周期秒数<AdminTextInput ariaLabel="周期秒数" value={secondsProduct.durationSeconds} onChange={(durationSeconds) => setSecondsProduct({ ...secondsProduct, durationSeconds })} /></label>
            <label>赔率<AdminTextInput ariaLabel="赔率" value={secondsProduct.payoutRate} onChange={(payoutRate) => setSecondsProduct({ ...secondsProduct, payoutRate })} /></label>
            <label>最小押注<AdminTextInput ariaLabel="最小押注" value={secondsProduct.minStake} onChange={(minStake) => setSecondsProduct({ ...secondsProduct, minStake })} /></label>
            <label>最大押注<AdminTextInput ariaLabel="最大押注" value={secondsProduct.maxStake} onChange={(maxStake) => setSecondsProduct({ ...secondsProduct, maxStake })} /></label>
            <label>
              初始状态
              <AdminSelect ariaLabel="初始状态" onChange={(status) => setSecondsProduct({ ...secondsProduct, status })} optionList={statusOptions} value={secondsProduct.status} />
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
