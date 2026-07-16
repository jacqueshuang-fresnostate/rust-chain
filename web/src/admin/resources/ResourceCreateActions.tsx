import { IconUpload } from '@douyinfe/semi-icons';
import { Button, Card, Col, Row, SideSheet, Space, Tabs, Typography, Toast, Upload } from '@douyinfe/semi-ui';
import type { customRequestArgs } from '@douyinfe/semi-ui/lib/es/upload';
import { type ReactNode, useEffect, useState } from 'react';

import { listAdminResource } from '../../api/adminResources';
import { ApiError, apiRequest } from '../../api/client';
import type { ApiRecord } from '../../api/types';
import { ConfirmAction } from '../../shared/ConfirmAction';
import type { DetailDrawerData } from '../../shared/DetailDrawer';
import { AdminImageUpload } from '../../shared/AdminImageUpload';
import { QuillRichTextEditor, type RichTextValue } from '../../shared/QuillRichTextEditor';
import { AdminCheckbox, AdminModalTriggerButton, AdminMultiSelect, AdminPasswordInput, AdminSelect, AdminSwitch, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';

const { Text, Title } = Typography;

type AssetValues = {
  logoUrl: string;
  symbol: string;
  name: string;
  precisionScale: string;
  assetType: string;
  status: string;
  depositEnabled: boolean;
  withdrawEnabled: boolean;
  minDepositAmount: string;
  depositFee: string;
  withdrawFee: string;
  withdrawFeeTiers: AssetWithdrawFeeTierValues[];
};

type AssetConfigValues = {
  logoUrl: string;
  name: string;
  precisionScale: string;
  assetType: string;
  status: string;
  depositEnabled: boolean;
  withdrawEnabled: boolean;
  minDepositAmount: string;
  depositFee: string;
  withdrawFee: string;
  withdrawFeeTiers: AssetWithdrawFeeTierValues[];
};

type AssetWithdrawFeeTierValues = {
  minAmount: string;
  maxAmount: string;
  feeRatePercent: string;
};

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

type ConvertPairValues = {
  fromAssetId: string;
  toAssetId: string;
  pricingMode: string;
  spreadRate: string;
  feeRate: string;
  minAmount: string;
  maxAmount: string;
  targetMinAmount: string;
  targetMaxAmount: string;
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
  bannerUrl: string;
  category: string;
  earlyRedeemFeeBasis: string;
  earlyRedeemFeeRate: string;
  introductions: EarnIntroductionItemValues[];
  maturityProfitFeeRate: string;
  maxSubscribe: string;
  minSubscribe: string;
  name: string;
  redemptionFeeRate: string;
  smallLogoUrl: string;
  status: string;
  termDays: string;
};

type LoanProductNameItemValues = {
  country: string;
  locale: string;
  title: string;
};

type LoanProductValues = {
  assetId: string;
  interestCalculationMode: string;
  interestRate: string;
  loanType: string;
  maxAmount: string;
  minAmount: string;
  minKycLevel: string;
  name: string;
  names: LoanProductNameItemValues[];
  status: string;
  termDays: string;
};

type EarnCategoryNameItemValues = {
  country: string;
  locale: string;
  title: string;
};

type EarnCategoryValues = {
  code: string;
  names: EarnCategoryNameItemValues[];
  sortOrder: string;
  status: string;
};

type AdminNewsTranslationValues = {
  content: RichTextValue;
  countryCode: string;
  locale: string;
  summary: RichTextValue;
  title: string;
};

type AdminNewsValues = {
  bannerUrl: string;
  category: string;
  countryCode: string;
  defaultLocale: string;
  smallLogoUrl: string;
  status: string;
  title: string;
  translations: AdminNewsTranslationValues[];
};

type CountryValues = {
  countryCode: string;
  countryName: string;
  remark: string;
  defaultLocale: string;
  supportedLocales: string;
  registrationEnabled: string;
  status: string;
  sortOrder: string;
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

type DepositAddressPoolValues = {
  network: string;
  addressGroupCode: string;
  address: string;
  assetSymbols: string[];
  status: string;
  memo: string;
  remark: string;
};

type DepositAddressPoolEntryValues = {
  address: string;
  memo: string;
  remark: string;
};

type DepositAddressPoolCreateValues = {
  addressGroupCode: string;
  assetSymbols: string[];
  entries: DepositAddressPoolEntryValues[];
  network: string;
  status: string;
};

type DepositNetworkConfigValues = {
  network: string;
  displayName: string;
  addressGroupCode: string;
  addressGroupName: string;
  assetSymbols: string[];
  status: string;
  sortOrder: string;
};

type AssignAgentValues = {
  agentId: string;
};

type AgentCommissionRuleValues = {
  agentId: string;
  commissionRate: string;
  productType: string;
  status: string;
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
  logoUrl: string;
  pairId: string;
  stakeAsset: string;
  periods: SecondsProductPeriodValues[];
  status: string;
};

type SecondsProductPeriodValues = {
  durationSeconds: string;
  payoutRate: string;
  minStake: string;
  maxStake: string;
};

type AssetOption = {
  id: string;
  label: string;
  symbol: string;
};

type DepositNetworkConfigOption = {
  addressGroupCode: string;
  addressGroupName: string;
  assetSymbols: string[];
  displayName: string;
  label: string;
  network: string;
};

type MarketPairOption = {
  id: string;
  label: string;
};

type AdminNewsCountryOption = {
  countryCode: string;
  countryName: string;
  defaultLocale: string;
};

type RowActionHelpers = {
  reload: () => void;
  openDetail: (detail: DetailDrawerData) => void;
};

type CreateModalSize = 'medium' | 'wide' | 'extra-wide';
type MarginProductTab = 'basic' | 'leverage' | 'risk';
type SecondsProductTab = 'basic' | 'trade';
type CreateActionProps = {
  onCreated?: () => void;
};
type FormModalHelpers = {
  close: () => void;
};
type FormModalChildren = ReactNode | ((helpers: FormModalHelpers) => ReactNode);

const createModalWidths: Record<CreateModalSize, string> = {
  medium: 'min(720px, calc(100vw - 48px))',
  wide: 'min(920px, calc(100vw - 48px))',
  'extra-wide': 'min(1120px, calc(100vw - 48px))'
};

function createModalProps(size: CreateModalSize) {
  return {
    bodyStyle: { overflowY: 'auto' as const },
    className: `admin-create-modal admin-create-modal-${size}`,
    closeOnEsc: true,
    maskClosable: false,
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

const initialDepositAddressPoolEntry: DepositAddressPoolEntryValues = {
  address: '',
  memo: '',
  remark: ''
};

function createInitialDepositAddressPoolCreate(): DepositAddressPoolCreateValues {
  return {
    addressGroupCode: 'A',
    assetSymbols: [],
    entries: [{ ...initialDepositAddressPoolEntry }],
    network: 'eth',
    status: 'available'
  };
}

const initialDepositNetworkConfig: DepositNetworkConfigValues = {
  network: 'eth',
  displayName: 'Ethereum',
  addressGroupCode: 'A',
  addressGroupName: 'EVM',
  assetSymbols: [],
  status: 'active',
  sortOrder: '0'
};

const initialAssignAgent: AssignAgentValues = {
  agentId: ''
};

const initialAgentCommissionRule: AgentCommissionRuleValues = {
  agentId: '',
  commissionRate: '',
  productType: 'convert',
  status: 'active'
};

const initialAsset: AssetValues = {
  logoUrl: '',
  symbol: '',
  name: '',
  precisionScale: '8',
  assetType: 'coin',
  status: 'active',
  depositEnabled: true,
  withdrawEnabled: true,
  minDepositAmount: '0',
  depositFee: '0',
  withdrawFee: '0',
  withdrawFeeTiers: []
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

const initialConvertPair: ConvertPairValues = {
  fromAssetId: '',
  toAssetId: '',
  pricingMode: 'fixed',
  spreadRate: '',
  feeRate: '0',
  minAmount: '',
  maxAmount: '',
  targetMinAmount: '',
  targetMaxAmount: '',
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
  bannerUrl: '',
  name: '',
  category: 'fixed_term',
  termDays: '',
  aprRate: '',
  redemptionFeeRate: '0',
  maturityProfitFeeRate: '0',
  earlyRedeemFeeBasis: 'none',
  earlyRedeemFeeRate: '0',
  minSubscribe: '',
  maxSubscribe: '',
  smallLogoUrl: '',
  status: 'active',
  introductions: [{ locale: 'zh-CN', country: 'CN', title: '', content: emptyRichTextValue }]
};

const initialEarnCategory: EarnCategoryValues = {
  code: '',
  names: [{ locale: 'zh-CN', country: 'CN', title: '' }],
  sortOrder: '0',
  status: 'active'
};

const initialLoanProduct: LoanProductValues = {
  assetId: '',
  interestCalculationMode: 'full_term',
  interestRate: '',
  loanType: 'credit',
  maxAmount: '',
  minAmount: '',
  minKycLevel: '0',
  name: '',
  names: [{ locale: 'zh-CN', country: 'CN', title: '' }],
  status: 'active',
  termDays: ''
};

const initialAdminNews: AdminNewsValues = {
  bannerUrl: '',
  title: '',
  category: 'general',
  countryCode: '',
  defaultLocale: 'zh-CN',
  smallLogoUrl: '',
  status: 'draft',
  translations: [{ locale: 'zh-CN', countryCode: 'CN', title: '', summary: emptyRichTextValue, content: emptyRichTextValue }]
};

const initialCountry: CountryValues = {
  countryCode: '',
  countryName: '',
  remark: '',
  defaultLocale: 'zh',
  supportedLocales: 'zh,en',
  registrationEnabled: 'true',
  status: 'active',
  sortOrder: '0'
};

const initialRiskRule: RiskRuleValues = {
  ruleType: '',
  targetType: '',
  targetId: '',
  configJson: '{}',
  enabled: 'true'
};

const earnEarlyRedeemFeeBasisOptions: SemiSelectOption[] = [
  { value: 'none', label: '不扣费' },
  { value: 'principal', label: '按本金比例扣除' },
  { value: 'profit', label: '按收益比例扣除' }
];

const loanTypeOptions: SemiSelectOption[] = [
  { value: 'credit', label: '信用贷' },
  { value: 'collateralized', label: '抵押贷' }
];

const loanInterestModeOptions: SemiSelectOption[] = [
  { value: 'full_term', label: '完整周期计息' },
  { value: 'actual_days', label: '按实际天数计息' }
];

const newsCategoryOptions: SemiSelectOption[] = [
  { value: 'general', label: '通用资讯' },
  { value: 'market', label: '市场资讯' },
  { value: 'product', label: '产品资讯' },
  { value: 'system', label: '系统公告' },
  { value: 'promotion', label: '活动推广' }
];

const newsStatusOptions: SemiSelectOption[] = [
  { value: 'draft', label: '草稿' },
  { value: 'published', label: '已发布' },
  { value: 'archived', label: '已归档' }
];

const activeStatusOptions: SemiSelectOption[] = [
  { value: 'active', label: '启用' },
  { value: 'disabled', label: '禁用' }
];

const localeOptions: SemiSelectOption[] = [
  { value: 'zh', label: '中文' },
  { value: 'en', label: '英文' }
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

const initialSecondsProductPeriod: SecondsProductPeriodValues = {
  durationSeconds: '',
  payoutRate: '',
  minStake: '',
  maxStake: ''
};

const initialSecondsProduct: SecondsProductValues = {
  logoUrl: '',
  pairId: '',
  stakeAsset: '',
  periods: [initialSecondsProductPeriod],
  status: 'active'
};

const secondsProductTabs = [
  { itemKey: 'basic', tab: '基础配置' },
  { itemKey: 'trade', tab: '交易参数' }
];

const newCoinLifecycleOptions: SemiSelectOption[] = [
  { value: 'preheat', label: '预热' },
  { value: 'subscription', label: '申购中' },
  { value: 'distribution', label: '分发中' },
  { value: 'listed', label: '已上市' }
];

const newCoinUnlockTypeOptions: SemiSelectOption[] = [
  { value: 'immediate_on_listing', label: '上市即解禁' },
  { value: 'fixed_time', label: '固定时间解禁' },
  { value: 'relative_period', label: '相对周期解禁' }
];

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

function requiredNonNegativeDecimal(value: string, label: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${label}不能为空`);
  }
  const parsed = Number(trimmed);
  if (!/^\d+(\.\d+)?$/.test(trimmed) || !Number.isFinite(parsed)) {
    throw new Error(`${label}必须为非负数`);
  }
  return trimmed;
}

function isNonNegativeDecimalInput(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }
  return /^\d+(\.\d+)?$/.test(trimmed) && Number.isFinite(Number(trimmed));
}

function isWithdrawFeeTiersInputValid(tiers: AssetWithdrawFeeTierValues[]): boolean {
  return tiers.every((tier) => {
    const minAmount = tier.minAmount.trim();
    const maxAmount = tier.maxAmount.trim();
    return (
      isNonNegativeDecimalInput(minAmount) &&
      (!maxAmount || (isNonNegativeDecimalInput(maxAmount) && Number(maxAmount) > Number(minAmount))) &&
      isNonNegativeDecimalInput(tier.feeRatePercent)
    );
  });
}

function recordWithdrawFeeTiers(record: ApiRecord): AssetWithdrawFeeTierValues[] {
  const rawValue = record.withdraw_fee_tiers;
  const rawTiers = Array.isArray(rawValue) ? rawValue : parseJsonArray(rawValue);
  return rawTiers
    .map((item) => {
      if (!item || typeof item !== 'object') return null;
      const tier = item as Record<string, unknown>;
      return {
        minAmount: formValueString(tier.min_amount),
        maxAmount: formValueString(tier.max_amount),
        feeRatePercent: formValueString(tier.fee_rate_percent)
      };
    })
    .filter((tier): tier is AssetWithdrawFeeTierValues => Boolean(tier && tier.minAmount && tier.feeRatePercent));
}

function withdrawFeeTierPayload(tiers: AssetWithdrawFeeTierValues[]) {
  return tiers.map((tier, index) => {
    const minAmount = requiredNonNegativeDecimal(tier.minAmount, `第${index + 1}档最小金额`);
    const maxAmount = tier.maxAmount.trim() ? requiredNonNegativeDecimal(tier.maxAmount, `第${index + 1}档最大金额`) : null;
    if (maxAmount !== null && Number(maxAmount) <= Number(minAmount)) {
      throw new Error(`第${index + 1}档最大金额必须大于最小金额`);
    }
    return {
      min_amount: minAmount,
      max_amount: maxAmount,
      fee_rate_percent: requiredNonNegativeDecimal(tier.feeRatePercent, `第${index + 1}档手续费比例`)
    };
  });
}

function parseJsonArray(value: unknown): unknown[] {
  if (typeof value !== 'string' || !value.trim()) return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function formValueString(value: unknown): string {
  return typeof value === 'number' || typeof value === 'string' ? String(value) : '';
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
  const symbol = assetFieldToString(asset, 'symbol');
  return id ? { id, label: assetOptionLabel(asset), symbol } : null;
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

function toEarnCategoryOption(category: ApiRecord): SemiSelectOption | null {
  const code = recordString(category, 'code');
  const name = recordString(category, 'default_name') || code;
  return code ? { value: code, label: `${name}（${code}）` } : null;
}

function includeCurrentOption<T extends { id: string; label: string }>(options: T[], id: string, label: string): T[] {
  const optionId = id.trim();
  if (!optionId || options.some((option) => option.id === optionId)) {
    return options;
  }
  return [{ id: optionId, label: label || `ID: ${optionId}` } as T, ...options];
}

function includeCurrentSelectOption(options: SemiSelectOption[], value: string, label: string): SemiSelectOption[] {
  const normalizedValue = value.trim();
  if (!normalizedValue || options.some((option) => option.value === normalizedValue)) {
    return options;
  }
  return [{ value: normalizedValue, label: label || normalizedValue }, ...options];
}

function recordCategoryFallbackLabel(category: string): string {
  return category.trim() ? category : 'fixed_term';
}

function isUserCreatable(values: UserValues): boolean {
  return Boolean((values.email.trim() || values.phone.trim()) && values.password.trim() && values.status.trim() && isNonNegativeIntegerInput(values.kycLevel));
}

function isUserRechargeSubmittable(values: UserRechargeValues): boolean {
  return Boolean(values.assetId.trim() && values.amount.trim() && Number(values.amount) > 0);
}

function isDepositAddressPoolSubmittable(values: DepositAddressPoolValues): boolean {
  return Boolean(values.network.trim() && values.address.trim() && values.status.trim());
}

function isDepositAddressPoolCreateSubmittable(values: DepositAddressPoolCreateValues): boolean {
  return Boolean(values.network.trim() && values.addressGroupCode.trim() && values.status.trim() && values.entries.length > 0 && values.entries.every((entry) => entry.address.trim()));
}

function isDepositNetworkConfigSubmittable(values: DepositNetworkConfigValues): boolean {
  return Boolean(values.network.trim() && values.displayName.trim() && values.addressGroupCode.trim() && values.status.trim() && isNonNegativeIntegerInput(values.sortOrder));
}

function depositAddressPoolFromRecord(record: ApiRecord): DepositAddressPoolValues {
  const assetSymbols = Array.isArray(record.asset_symbols) ? record.asset_symbols.filter((value): value is string => typeof value === 'string') : [];
  const network = recordString(record, 'network') || 'eth';
  return {
    network,
    addressGroupCode: recordString(record, 'address_group_code') || defaultDepositAddressGroupCode(network),
    address: recordString(record, 'address'),
    assetSymbols: assetSymbols.length > 0 ? assetSymbols : recordString(record, 'asset_symbol') ? [recordString(record, 'asset_symbol')] : [],
    status: recordString(record, 'status') === 'disabled' ? 'disabled' : 'available',
    memo: recordString(record, 'memo'),
    remark: recordString(record, 'remark')
  };
}

function normalizedDepositAssetSymbols(values: string[]): string[] {
  return values.map((value) => value.trim().toUpperCase()).filter((value, index, items) => value && items.indexOf(value) === index);
}

function depositAddressPoolRequestBody(values: DepositAddressPoolValues, reason: string) {
  const assetSymbols = normalizedDepositAssetSymbols(values.assetSymbols);
  return {
    network: requiredString(values.network, '网络'),
    address_group_code: requiredString(values.addressGroupCode, '地址集合编号'),
    address: requiredString(values.address, '充值地址'),
    asset_symbols: assetSymbols,
    status: requiredString(values.status, '状态'),
    memo: optionalString(values.memo),
    remark: optionalString(values.remark),
    reason
  };
}

function depositAddressPoolBatchRequestBody(values: DepositAddressPoolCreateValues, reason: string) {
  return {
    network: requiredString(values.network, '网络'),
    address_group_code: requiredString(values.addressGroupCode, '地址集合编号'),
    asset_symbols: normalizedDepositAssetSymbols(values.assetSymbols),
    status: requiredString(values.status, '状态'),
    entries: values.entries.map((entry) => ({
      address: requiredString(entry.address, '充值地址'),
      memo: optionalString(entry.memo),
      remark: optionalString(entry.remark)
    })),
    reason
  };
}

function depositNetworkConfigFromRecord(record: ApiRecord): DepositNetworkConfigValues {
  const assetSymbols = Array.isArray(record.asset_symbols) ? record.asset_symbols.filter((value): value is string => typeof value === 'string') : [];
  return {
    network: recordString(record, 'network') || 'eth',
    displayName: recordString(record, 'display_name'),
    addressGroupCode: recordString(record, 'address_group_code'),
    addressGroupName: recordString(record, 'address_group_name'),
    assetSymbols,
    status: recordString(record, 'status') === 'disabled' ? 'disabled' : 'active',
    sortOrder: recordString(record, 'sort_order') || '0'
  };
}

function depositNetworkConfigRequestBody(values: DepositNetworkConfigValues, reason: string) {
  return {
    network: requiredString(values.network, '网络'),
    display_name: requiredString(values.displayName, '显示名称'),
    address_group_code: requiredString(values.addressGroupCode, '地址集合编号'),
    address_group_name: optionalString(values.addressGroupName),
    asset_symbols: normalizedDepositAssetSymbols(values.assetSymbols),
    status: requiredString(values.status, '状态'),
    sort_order: Number.parseInt(values.sortOrder, 10),
    reason
  };
}

function depositAddressImportDelimiter(line: string): string {
  if (line.includes('\t')) {
    return '\t';
  }
  if (line.includes('|')) {
    return '|';
  }
  return ',';
}

function parseDelimitedImportLine(line: string, delimiter: string): string[] {
  const cells: string[] = [];
  let current = '';
  let quoted = false;

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (char === '"') {
      if (quoted && line[index + 1] === '"') {
        current += '"';
        index += 1;
      } else {
        quoted = !quoted;
      }
      continue;
    }
    if (char === delimiter && !quoted) {
      cells.push(current.trim());
      current = '';
      continue;
    }
    current += char;
  }

  cells.push(current.trim());
  return cells;
}

function isDepositAddressImportHeader(cells: string[]): boolean {
  const firstCell = (cells[0] ?? '').replace(/^\uFEFF/, '').trim().toLowerCase();
  return ['address', 'deposit address', '充值地址', '地址'].includes(firstCell);
}

function parseDepositAddressImportText(content: string): DepositAddressPoolEntryValues[] {
  const entries: DepositAddressPoolEntryValues[] = [];
  const lines = content.replace(/\r\n/g, '\n').replace(/\r/g, '\n').split('\n');

  lines.forEach((line, index) => {
    if (!line.trim()) {
      return;
    }

    const cells = parseDelimitedImportLine(line, depositAddressImportDelimiter(line));
    const address = (cells[0] ?? '').replace(/^\uFEFF/, '').trim();
    if (index === 0 && isDepositAddressImportHeader(cells)) {
      return;
    }
    if (!address) {
      throw new Error(`第 ${index + 1} 行缺少充值地址`);
    }

    entries.push({
      address,
      memo: (cells[1] ?? '').trim(),
      remark: cells.slice(2).join('，').trim()
    });
  });

  if (!entries.length) {
    throw new Error('导入文件没有可用的充值地址');
  }

  return entries;
}

function hasDepositAddressEntryContent(entry: DepositAddressPoolEntryValues): boolean {
  return Boolean(entry.address.trim() || entry.memo.trim() || entry.remark.trim());
}

function readDepositAddressImportFile(file: File): Promise<string> {
  if (typeof file.text === 'function') {
    return file.text();
  }

  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error('读取导入文件失败'));
    reader.onload = () => resolve(String(reader.result ?? ''));
    reader.readAsText(file);
  });
}

function isAssignAgentSubmittable(values: AssignAgentValues): boolean {
  return Boolean(values.agentId.trim() && Number(values.agentId) > 0);
}

function isAgentCommissionRuleSubmittable(values: AgentCommissionRuleValues, includeAgentId: boolean): boolean {
  return Boolean((!includeAgentId || values.agentId.trim()) && values.productType === 'convert' && values.commissionRate.trim() && values.status.trim());
}

function isAssetCreatable(values: AssetValues): boolean {
  return Boolean(
    values.symbol.trim() &&
      values.name.trim() &&
      isNonNegativeIntegerInput(values.precisionScale) &&
      isNonNegativeDecimalInput(values.minDepositAmount) &&
      isNonNegativeDecimalInput(values.depositFee) &&
      isNonNegativeDecimalInput(values.withdrawFee) &&
      isWithdrawFeeTiersInputValid(values.withdrawFeeTiers)
  );
}

function isAssetConfigUpdatable(values: AssetConfigValues): boolean {
  return Boolean(
    values.name.trim() &&
      isNonNegativeIntegerInput(values.precisionScale) &&
      values.assetType.trim() &&
      values.status.trim() &&
      isNonNegativeDecimalInput(values.minDepositAmount) &&
      isNonNegativeDecimalInput(values.depositFee) &&
      isNonNegativeDecimalInput(values.withdrawFee) &&
      isWithdrawFeeTiersInputValid(values.withdrawFeeTiers)
  );
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

function isSecondsProductPeriodSubmittable(period: SecondsProductPeriodValues): boolean {
  return Boolean(period.durationSeconds.trim() && period.payoutRate.trim() && period.minStake.trim());
}

function secondsProductDurationKeys(periods: SecondsProductPeriodValues[]): string[] {
  return periods.map((period) => period.durationSeconds.trim()).filter(Boolean);
}

function isSecondsProductCreatable(values: SecondsProductValues): boolean {
  const durationKeys = secondsProductDurationKeys(values.periods);
  return Boolean(
    values.pairId.trim() &&
      values.stakeAsset.trim() &&
      values.status.trim() &&
      values.periods.length > 0 &&
      values.periods.every(isSecondsProductPeriodSubmittable) &&
      durationKeys.length === new Set(durationKeys).size
  );
}

function secondsProductFromRecord(record: ApiRecord): SecondsProductValues {
  const cycles = Array.isArray(record.cycles) ? record.cycles : [];
  const periods = cycles
    .map((cycle) => {
      if (!cycle || typeof cycle !== 'object') {
        return null;
      }
      const cycleRecord = cycle as ApiRecord;
      return {
        durationSeconds: recordString(cycleRecord, 'duration_seconds'),
        payoutRate: recordString(cycleRecord, 'payout_rate'),
        minStake: recordString(cycleRecord, 'min_stake'),
        maxStake: recordString(cycleRecord, 'max_stake')
      };
    })
    .filter((period): period is SecondsProductPeriodValues => Boolean(period?.durationSeconds));

  return {
    logoUrl: recordString(record, 'logo_url'),
    pairId: recordString(record, 'pair_id'),
    stakeAsset: recordString(record, 'stake_asset'),
    periods: periods.length
      ? periods
      : [
          {
            durationSeconds: recordString(record, 'duration_seconds'),
            payoutRate: recordString(record, 'payout_rate'),
            minStake: recordString(record, 'min_stake'),
            maxStake: recordString(record, 'max_stake')
          }
        ],
    status: recordString(record, 'status') || 'active'
  };
}

function secondsProductRequestBody(values: SecondsProductValues, reason: string) {
  return {
    pair_id: requiredPositiveInteger(values.pairId, '秒合约交易对ID'),
    stake_asset: requiredPositiveInteger(values.stakeAsset, '押注资产ID'),
    logo_url: optionalString(values.logoUrl),
    cycles: values.periods.map((period) => ({
      duration_seconds: requiredPositiveInteger(period.durationSeconds, '周期秒数'),
      payout_rate: requiredString(period.payoutRate, '赔率'),
      min_stake: requiredString(period.minStake, '最小押注'),
      max_stake: optionalString(period.maxStake)
    })),
    status: requiredString(values.status, '状态'),
    reason
  };
}

function isConvertPairCreatable(values: ConvertPairValues): boolean {
  return Boolean(
    values.fromAssetId.trim() &&
      values.toAssetId.trim() &&
      values.fromAssetId !== values.toAssetId &&
      values.pricingMode.trim() &&
      values.spreadRate.trim() &&
      values.feeRate.trim() &&
      values.minAmount.trim() &&
      values.targetMinAmount.trim()
  );
}

function convertPairFromRecord(record: ApiRecord): ConvertPairValues {
  return {
    fromAssetId: recordString(record, 'from_asset_id'),
    toAssetId: recordString(record, 'to_asset_id'),
    pricingMode: recordString(record, 'pricing_mode') || 'fixed',
    spreadRate: recordString(record, 'spread_rate'),
    feeRate: recordString(record, 'fee_rate') || '0',
    minAmount: recordString(record, 'min_amount'),
    maxAmount: recordString(record, 'max_amount'),
    targetMinAmount: recordString(record, 'target_min_amount'),
    targetMaxAmount: recordString(record, 'target_max_amount'),
    enabled: record.enabled === false ? 'false' : 'true'
  };
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
      values.redemptionFeeRate.trim() &&
      values.maturityProfitFeeRate.trim() &&
      values.earlyRedeemFeeBasis.trim() &&
      (values.earlyRedeemFeeBasis === 'none' || values.earlyRedeemFeeRate.trim()) &&
      values.minSubscribe.trim() &&
      values.status.trim() &&
      values.introductions.length > 0 &&
      values.introductions.every((item) => item.locale.trim() && item.country.trim() && item.title.trim())
  );
}

function isEarnCategorySubmittable(values: EarnCategoryValues, includeCode: boolean): boolean {
  return Boolean(
    (!includeCode || values.code.trim()) &&
      isNonNegativeIntegerInput(values.sortOrder) &&
      values.status.trim() &&
      values.names.length > 0 &&
      values.names.every((item) => item.locale.trim() && item.country.trim() && item.title.trim())
  );
}

function richTextHasContent(value: RichTextValue): boolean {
  return value.some((block) => {
    if (block.type === 'image') {
      return Boolean(block.url.trim());
    }
    return block.children.some((leaf) => leaf.text.trim().length > 0);
  });
}

function optionalRichTextValue(value: RichTextValue): RichTextValue | undefined {
  return richTextHasContent(value) ? value : undefined;
}

function isAdminNewsSubmittable(values: AdminNewsValues): boolean {
  return Boolean(
    values.title.trim() &&
      values.category.trim() &&
      values.defaultLocale.trim() &&
      values.translations.length > 0 &&
      values.translations.every((item) => item.locale.trim() && item.countryCode.trim() && item.title.trim() && richTextHasContent(item.content))
  );
}

function isAdminNewsCreateSubmittable(values: AdminNewsValues): boolean {
  return Boolean(values.countryCode.trim() && isAdminNewsSubmittable(syncAdminNewsCreateContent(values)));
}

function countrySupportedLocales(values: CountryValues): string[] {
  return values.supportedLocales
    .split(',')
    .map((locale) => locale.trim().toLowerCase())
    .filter(Boolean)
    .filter((locale, index, locales) => locales.indexOf(locale) === index);
}

function isCountrySubmittable(values: CountryValues, includeCountryCode: boolean): boolean {
  const locales = countrySupportedLocales(values);
  return Boolean(
    (!includeCountryCode || values.countryCode.trim()) &&
      values.countryName.trim() &&
      values.remark.trim() &&
      values.defaultLocale.trim() &&
      locales.length > 0 &&
      locales.includes(values.defaultLocale.trim().toLowerCase()) &&
      values.registrationEnabled.trim() &&
      isNonNegativeIntegerInput(values.sortOrder)
  );
}

function countryFromRecord(record: ApiRecord): CountryValues {
  const supportedLocales = Array.isArray(record.supported_locales)
    ? record.supported_locales.filter((locale): locale is string => typeof locale === 'string').join(',')
    : '';

  return {
    countryCode: recordString(record, 'country_code'),
    countryName: recordString(record, 'country_name'),
    remark: recordString(record, 'remark'),
    defaultLocale: recordString(record, 'default_locale') || 'zh',
    supportedLocales,
    registrationEnabled: record.registration_enabled === false ? 'false' : 'true',
    status: recordString(record, 'status') || 'active',
    sortOrder: recordString(record, 'sort_order') || '0'
  };
}

function countryCreateRequestBody(values: CountryValues, reason: string) {
  return {
    country_code: requiredString(values.countryCode, '国家代码').toUpperCase(),
    country_name: requiredString(values.countryName, '国家名称'),
    remark: requiredString(values.remark, '备注（中文名称）'),
    default_locale: requiredString(values.defaultLocale, '默认语言'),
    supported_locales: countrySupportedLocales(values),
    registration_enabled: booleanFromSelect(values.registrationEnabled),
    status: requiredString(values.status, '状态'),
    sort_order: requiredNonNegativeInteger(values.sortOrder, '排序'),
    reason
  };
}

function countryUpdateRequestBody(values: CountryValues, reason: string) {
  return {
    country_name: requiredString(values.countryName, '国家名称'),
    remark: requiredString(values.remark, '备注（中文名称）'),
    default_locale: requiredString(values.defaultLocale, '默认语言'),
    supported_locales: countrySupportedLocales(values),
    registration_enabled: booleanFromSelect(values.registrationEnabled),
    sort_order: requiredNonNegativeInteger(values.sortOrder, '排序'),
    reason
  };
}

function convertPairRequestBody(values: ConvertPairValues, reason: string) {
  return {
    from_asset_id: requiredPositiveInteger(values.fromAssetId, '源资产'),
    to_asset_id: requiredPositiveInteger(values.toAssetId, '目标资产'),
    pricing_mode: requiredString(values.pricingMode, '定价模式'),
    spread_rate: requiredString(values.spreadRate, '价差率'),
    fee_rate: requiredString(values.feeRate, '手续费率'),
    min_amount: requiredString(values.minAmount, '源资产最小金额'),
    max_amount: optionalString(values.maxAmount),
    target_min_amount: requiredString(values.targetMinAmount, '目标资产最小金额'),
    target_max_amount: optionalString(values.targetMaxAmount),
    enabled: booleanFromSelect(values.enabled),
    reason
  };
}

function convertPairUpdateRequestBody(values: ConvertPairValues, reason: string) {
  return {
    ...convertPairRequestBody(values, reason),
    max_amount: optionalString(values.maxAmount) ?? null,
    target_max_amount: optionalString(values.targetMaxAmount) ?? null
  };
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

function completeCreate(close: () => void, onCreated?: () => void, reset?: () => void) {
  close();
  reset?.();
  onCreated?.();
}

function FormModal({ actionText, children, size = 'medium', title }: { actionText: string; children: FormModalChildren; size?: CreateModalSize; title: string }) {
  const [visible, setVisible] = useState(false);
  const close = () => setVisible(false);
  const content = typeof children === 'function' ? children({ close }) : children;

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>{actionText}</AdminModalTriggerButton>
      <SideSheet onCancel={close} title={title} visible={visible} {...createModalProps(size)}>
        {content}
      </SideSheet>
    </>
  );
}

function useAssetOptions(enabled = true) {
  const [assetOptions, setAssetOptions] = useState<AssetOption[]>([]);
  const [assetLoading, setAssetLoading] = useState(false);

  useEffect(() => {
    if (!enabled) {
      return undefined;
    }

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
  }, [enabled]);

  return { assetLoading, assetOptions };
}

function toDepositNetworkConfigOption(record: ApiRecord): DepositNetworkConfigOption | null {
  const network = recordString(record, 'network');
  const addressGroupCode = recordString(record, 'address_group_code');
  if (!network || !addressGroupCode) {
    return null;
  }
  const displayName = recordString(record, 'display_name') || network;
  const addressGroupName = recordString(record, 'address_group_name');
  const assetSymbols = Array.isArray(record.asset_symbols)
    ? record.asset_symbols.filter((value): value is string => typeof value === 'string')
    : [];
  return {
    addressGroupCode,
    addressGroupName,
    assetSymbols,
    displayName,
    label: addressGroupName ? `${displayName} · ${addressGroupCode}（${addressGroupName}）` : `${displayName} · ${addressGroupCode}`,
    network
  };
}

function useDepositNetworkConfigOptions(enabled = true) {
  const [networkConfigs, setNetworkConfigs] = useState<DepositNetworkConfigOption[]>([]);
  const [networkConfigLoading, setNetworkConfigLoading] = useState(false);

  useEffect(() => {
    if (!enabled) {
      return undefined;
    }

    let active = true;
    setNetworkConfigLoading(true);

    listAdminResource('/admin/api/v1/deposit-network-configs', 'configs', { status: 'active', limit: 100 })
      .then((result) => {
        if (!active) {
          return;
        }

        setNetworkConfigs(result.rows.map(toDepositNetworkConfigOption).filter((config): config is DepositNetworkConfigOption => config !== null));
      })
      .catch(() => {
        if (active) {
          setNetworkConfigs([]);
        }
      })
      .finally(() => {
        if (active) {
          setNetworkConfigLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [enabled]);

  return { networkConfigLoading, networkConfigs };
}

function useMarketPairOptions(enabled = true) {
  const [pairOptions, setPairOptions] = useState<MarketPairOption[]>([]);
  const [pairLoading, setPairLoading] = useState(false);

  useEffect(() => {
    if (!enabled) {
      return undefined;
    }

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
  }, [enabled]);

  return { pairLoading, pairOptions };
}

function useAdminCountryOptions(enabled = true) {
  const [countries, setCountries] = useState<AdminNewsCountryOption[]>([]);
  const [countriesLoading, setCountriesLoading] = useState(false);

  useEffect(() => {
    if (!enabled || countries.length > 0 || countriesLoading) return;
    let cancelled = false;
    setCountriesLoading(true);
    listAdminResource('/admin/api/v1/countries', 'countries', { status: 'active', limit: 500 })
      .then(({ rows }) => {
        if (cancelled) return;
        setCountries(rows.map(adminNewsCountryFromRecord).filter((country): country is AdminNewsCountryOption => country !== null));
        setCountriesLoading(false);
      })
      .catch((error) => {
        if (cancelled) return;
        Toast.error(errorMessage(error));
        setCountries([]);
        setCountriesLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [countries.length, enabled]);

  return { countries, countriesLoading };
}

function useEarnCategoryOptions(enabled = true) {
  const [categoryOptions, setCategoryOptions] = useState<SemiSelectOption[]>([]);
  const [categoryLoading, setCategoryLoading] = useState(false);

  useEffect(() => {
    if (!enabled) {
      return undefined;
    }

    let active = true;
    setCategoryLoading(true);

    listAdminResource('/admin/api/v1/earn/categories', 'categories', { status: 'active', limit: 100 })
      .then((result) => {
        if (!active) {
          return;
        }

        setCategoryOptions(result.rows.map(toEarnCategoryOption).filter((category): category is SemiSelectOption => category !== null));
      })
      .catch(() => {
        if (active) {
          setCategoryOptions([]);
        }
      })
      .finally(() => {
        if (active) {
          setCategoryLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [enabled]);

  return { categoryLoading, categoryOptions };
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

function AssetSymbolSelect({
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
  const symbolOptions = options
    .filter((asset) => asset.symbol)
    .map((asset) => ({ value: asset.symbol, label: asset.label }));

  return (
    <label>
      {label}
      <AdminSelect
        ariaLabel={label}
        disabled={loading}
        loading={loading}
        onChange={onChange}
        optionList={symbolOptions}
        placeholder={loading ? '加载资产中...' : '请选择项目符号'}
        value={value}
      />
    </label>
  );
}

function AssetSymbolMultiSelect({
  label,
  loading,
  onChange,
  options,
  value
}: {
  label: string;
  loading: boolean;
  onChange: (value: string[]) => void;
  options: AssetOption[];
  value: string[];
}) {
  const symbolOptions = options
    .filter((asset) => asset.symbol)
    .map((asset) => ({ value: asset.symbol, label: asset.label }));

  return (
    <label>
      {label}
      <AdminMultiSelect
        ariaLabel={label}
        disabled={loading}
        loading={loading}
        onChange={onChange}
        optionList={symbolOptions}
        placeholder={loading ? '加载资产中...' : '留空表示该网络任意资产'}
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

const depositNetworkOptions: SemiSelectOption[] = [
  { value: 'eth', label: 'ETH' },
  { value: 'base', label: 'Base' },
  { value: 'tron', label: 'Tron' },
  { value: 'btc', label: 'BTC' },
  { value: 'solana', label: 'Solana' }
];

function depositNetworkSelectOptions(configs: DepositNetworkConfigOption[]): SemiSelectOption[] {
  return configs.length > 0 ? configs.map((config) => ({ value: config.network, label: config.label })) : depositNetworkOptions;
}

function depositNetworkConfigForNetwork(configs: DepositNetworkConfigOption[], network: string): DepositNetworkConfigOption | undefined {
  return configs.find((config) => config.network === network);
}

function defaultDepositAddressGroupCode(network: string): string {
  switch (network) {
    case 'eth':
    case 'base':
      return 'A';
    case 'btc':
      return 'B';
    case 'tron':
      return 'C';
    case 'solana':
      return 'D';
    default:
      return network.toUpperCase();
  }
}

function depositAddressGroupForNetwork(configs: DepositNetworkConfigOption[], network: string, fallback = ''): string {
  return depositNetworkConfigForNetwork(configs, network)?.addressGroupCode || fallback || defaultDepositAddressGroupCode(network);
}

function depositAssetOptionsForNetwork(options: AssetOption[], config?: DepositNetworkConfigOption): AssetOption[] {
  if (!config || config.assetSymbols.length === 0) {
    return options;
  }
  const allowed = new Set(config.assetSymbols.map((symbol) => symbol.toUpperCase()));
  return options.filter((asset) => allowed.has(asset.symbol.toUpperCase()));
}

function normalizeSelectedDepositAssetsForNetwork(values: string[], config?: DepositNetworkConfigOption): string[] {
  if (!config || config.assetSymbols.length === 0) {
    return values;
  }
  const allowed = new Set(config.assetSymbols.map((symbol) => symbol.toUpperCase()));
  return values.filter((symbol) => allowed.has(symbol.toUpperCase()));
}

const depositAddressStatusOptions: SemiSelectOption[] = [
  { value: 'available', label: '可用' },
  { value: 'disabled', label: '禁用' }
];

const agentCommissionRuleProductOptions: SemiSelectOption[] = [
  { value: 'convert', label: '闪兑' },
  { value: 'prediction', label: '竞猜' },
  { value: 'spot', label: '现货' },
  { value: 'margin', label: '杠杆' },
  { value: 'seconds_contract', label: '秒合约' }
];

const booleanOptions: SemiSelectOption[] = [
  { value: 'true', label: '启用' },
  { value: 'false', label: '禁用' }
];

const countryBooleanOptions: SemiSelectOption[] = [
  { value: 'true', label: '启用' },
  { value: 'false', label: '停用' }
];

const countryStatusOptions: SemiSelectOption[] = [
  { value: 'active', label: '启用' },
  { value: 'disabled', label: '停用' }
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

function BooleanSelect({ label, onChange, optionList = booleanOptions, value }: { label: string; onChange: (value: string) => void; optionList?: SemiSelectOption[]; value: string }) {
  return <AdminSelect ariaLabel={label} onChange={onChange} optionList={optionList} value={value} />;
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

function newEarnIntroduction(countries: AdminNewsCountryOption[] = []): EarnIntroductionItemValues {
  const country = countries.find((item) => item.countryCode === 'US') ?? countries[0];
  return {
    locale: country?.defaultLocale ?? 'en-US',
    country: country?.countryCode ?? 'US',
    title: '',
    content: emptyRichTextValue
  };
}

function updateEarnIntroduction(values: EarnProductValues, index: number, patch: Partial<EarnIntroductionItemValues>): EarnProductValues {
  return {
    ...values,
    introductions: values.introductions.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item))
  };
}

function newEarnCategoryName(countries: AdminNewsCountryOption[] = []): EarnCategoryNameItemValues {
  const country = countries.find((item) => item.countryCode === 'US') ?? countries[0];
  return {
    locale: country?.defaultLocale ?? 'en-US',
    country: country?.countryCode ?? 'US',
    title: ''
  };
}

function newLoanProductName(countries: AdminNewsCountryOption[] = []): LoanProductNameItemValues {
  const country = countries.find((item) => item.countryCode === 'US') ?? countries[0];
  return {
    locale: country?.defaultLocale ?? 'en-US',
    country: country?.countryCode ?? 'US',
    title: ''
  };
}

function updateEarnCategoryName(values: EarnCategoryValues, index: number, patch: Partial<EarnCategoryNameItemValues>): EarnCategoryValues {
  return {
    ...values,
    names: values.names.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item))
  };
}

function updateLoanProductName(values: LoanProductValues, index: number, patch: Partial<LoanProductNameItemValues>): LoanProductValues {
  const names = values.names.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item));
  return {
    ...values,
    name: names[0]?.title ?? values.name,
    names
  };
}

function newAdminNewsTranslation(): AdminNewsTranslationValues {
  return { locale: 'en-US', countryCode: 'US', title: '', summary: emptyRichTextValue, content: emptyRichTextValue };
}

function updateAdminNewsTranslation(values: AdminNewsValues, index: number, patch: Partial<AdminNewsTranslationValues>): AdminNewsValues {
  return {
    ...values,
    translations: values.translations.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item))
  };
}

function adminNewsCountryFromRecord(record: ApiRecord): AdminNewsCountryOption | null {
  const countryCode = String(record.country_code ?? '').trim().toUpperCase();
  const countryName = String(record.country_name ?? '').trim();
  const defaultLocale = String(record.default_locale ?? '').trim();
  if (!countryCode || !defaultLocale) return null;
  return {
    countryCode,
    countryName: countryName || countryCode,
    defaultLocale
  };
}

function adminNewsCountrySelectOptions(countries: AdminNewsCountryOption[]): SemiSelectOption[] {
  return countries.map((country) => ({
    value: country.countryCode,
    label: `${country.countryName} (${country.countryCode})`
  }));
}

function earnCountrySelectOptions(countries: AdminNewsCountryOption[]): SemiSelectOption[] {
  return countries.map((country) => ({
    value: country.countryCode,
    label: `${country.countryName} (${country.countryCode} / ${country.defaultLocale})`
  }));
}

function includeCurrentCountrySelectOption(options: SemiSelectOption[], countryCode: string, locale: string): SemiSelectOption[] {
  const value = countryCode.trim().toUpperCase();
  if (!value || options.some((option) => option.value === value)) {
    return options;
  }
  const label = locale.trim() ? `${value} (${locale.trim()})` : value;
  return [{ value, label }, ...options];
}

function applyEarnIntroductionCountry(values: EarnProductValues, index: number, countries: AdminNewsCountryOption[], countryCode: string): EarnProductValues {
  const normalizedCountryCode = countryCode.trim().toUpperCase();
  const country = countries.find((item) => item.countryCode === normalizedCountryCode);
  const current = values.introductions[index];
  return updateEarnIntroduction(values, index, {
    country: country?.countryCode ?? normalizedCountryCode,
    locale: country?.defaultLocale ?? current?.locale ?? ''
  });
}

function syncEarnProductCountryLocales(values: EarnProductValues, countries: AdminNewsCountryOption[]): EarnProductValues {
  let changed = false;
  const introductions = values.introductions.map((item) => {
    const country = countries.find((countryOption) => countryOption.countryCode === item.country.trim().toUpperCase());
    if (!country) {
      return item;
    }
    if (item.locale === country.defaultLocale && item.country === country.countryCode) {
      return item;
    }
    changed = true;
    return { ...item, country: country.countryCode, locale: country.defaultLocale };
  });
  return changed ? { ...values, introductions } : values;
}

function applyEarnCategoryNameCountry(values: EarnCategoryValues, index: number, countries: AdminNewsCountryOption[], countryCode: string): EarnCategoryValues {
  const normalizedCountryCode = countryCode.trim().toUpperCase();
  const country = countries.find((item) => item.countryCode === normalizedCountryCode);
  const current = values.names[index];
  return updateEarnCategoryName(values, index, {
    country: country?.countryCode ?? normalizedCountryCode,
    locale: country?.defaultLocale ?? current?.locale ?? ''
  });
}

function applyLoanProductNameCountry(values: LoanProductValues, index: number, countries: AdminNewsCountryOption[], countryCode: string): LoanProductValues {
  const normalizedCountryCode = countryCode.trim().toUpperCase();
  const country = countries.find((item) => item.countryCode === normalizedCountryCode);
  const current = values.names[index];
  return updateLoanProductName(values, index, {
    country: country?.countryCode ?? normalizedCountryCode,
    locale: country?.defaultLocale ?? current?.locale ?? ''
  });
}

function syncEarnCategoryCountryLocales(values: EarnCategoryValues, countries: AdminNewsCountryOption[]): EarnCategoryValues {
  let changed = false;
  const names = values.names.map((item) => {
    const country = countries.find((countryOption) => countryOption.countryCode === item.country.trim().toUpperCase());
    if (!country) {
      return item;
    }
    if (item.locale === country.defaultLocale && item.country === country.countryCode) {
      return item;
    }
    changed = true;
    return { ...item, country: country.countryCode, locale: country.defaultLocale };
  });
  return changed ? { ...values, names } : values;
}

function syncLoanProductCountryLocales(values: LoanProductValues, countries: AdminNewsCountryOption[]): LoanProductValues {
  let changed = false;
  const names = values.names.map((item) => {
    const country = countries.find((countryOption) => countryOption.countryCode === item.country.trim().toUpperCase());
    if (!country) {
      return item;
    }
    if (item.locale === country.defaultLocale && item.country === country.countryCode) {
      return item;
    }
    changed = true;
    return { ...item, country: country.countryCode, locale: country.defaultLocale };
  });
  return changed ? { ...values, name: names[0]?.title ?? values.name, names } : values;
}

function syncAdminNewsCreateContent(values: AdminNewsValues): AdminNewsValues {
  const countryCode = values.countryCode.trim().toUpperCase();
  const defaultLocale = values.defaultLocale.trim();
  const translation = values.translations[0] ?? newAdminNewsTranslation();
  return {
    ...values,
    countryCode,
    defaultLocale,
    translations: [
      {
        ...translation,
        locale: defaultLocale,
        countryCode,
        title: values.title
      }
    ]
  };
}

function applyAdminNewsCountry(values: AdminNewsValues, country: AdminNewsCountryOption): AdminNewsValues {
  return syncAdminNewsCreateContent({
    ...values,
    countryCode: country.countryCode,
    defaultLocale: country.defaultLocale
  });
}

function adminNewsContentJson(values: AdminNewsValues) {
  return {
    version: 1,
    default_locale: requiredString(values.defaultLocale, '默认语言'),
    items: values.translations.map((item) => ({
      locale: requiredString(item.locale, '语言'),
      country_code: requiredString(item.countryCode, '翻译国家'),
      title: requiredString(item.title, '翻译标题'),
      summary: optionalRichTextValue(item.summary),
      content: item.content
    }))
  };
}

function richTextValueFromPlainText(value: string): RichTextValue {
  const lines = value.replace(/\r\n/g, '\n').split('\n');
  return (lines.length > 0 ? lines : ['']).map((line) => ({ type: 'p', children: [{ text: line }] }));
}

function adminNewsSummaryFromRecord(value: unknown): RichTextValue {
  if (Array.isArray(value)) {
    return value as RichTextValue;
  }

  if (typeof value === 'string' && value.trim()) {
    return richTextValueFromPlainText(value);
  }

  return emptyRichTextValue;
}

function adminNewsCreateRequestBody(values: AdminNewsValues, reason: string) {
  const createValues = syncAdminNewsCreateContent(values);
  return {
    title: requiredString(createValues.title, '新闻标题'),
    banner_url: optionalString(createValues.bannerUrl),
    small_logo_url: optionalString(createValues.smallLogoUrl),
    category: requiredString(createValues.category, '分类'),
    status: requiredString(createValues.status, '状态'),
    country_code: requiredString(createValues.countryCode, '国家'),
    default_locale: requiredString(createValues.defaultLocale, '默认语言'),
    content_json: adminNewsContentJson(createValues),
    reason
  };
}

function adminNewsUpdateRequestBody(values: AdminNewsValues, reason: string) {
  return {
    title: requiredString(values.title, '新闻标题'),
    banner_url: optionalString(values.bannerUrl),
    small_logo_url: optionalString(values.smallLogoUrl),
    category: requiredString(values.category, '分类'),
    country_code: optionalString(values.countryCode),
    default_locale: requiredString(values.defaultLocale, '默认语言'),
    content_json: adminNewsContentJson(values),
    reason
  };
}

function adminNewsFromRecord(record: ApiRecord): AdminNewsValues {
  const contentJson = record.content_json as { default_locale?: unknown; items?: unknown } | undefined;
  const items = Array.isArray(contentJson?.items) ? contentJson.items : [];
  const translations = items
    .map((item) => {
      const value = item as Record<string, unknown>;
      const content = Array.isArray(value.content) ? (value.content as RichTextValue) : emptyRichTextValue;
      return {
        locale: typeof value.locale === 'string' ? value.locale : '',
        countryCode: typeof value.country_code === 'string' ? value.country_code : '',
        title: typeof value.title === 'string' ? value.title : '',
        summary: adminNewsSummaryFromRecord(value.summary),
        content
      };
    })
    .filter((item) => item.locale || item.countryCode || item.title);

  return {
    bannerUrl: recordString(record, 'banner_url'),
    title: recordString(record, 'title'),
    category: recordString(record, 'category') || 'general',
    countryCode: recordString(record, 'country_code'),
    defaultLocale: recordString(record, 'default_locale') || (typeof contentJson?.default_locale === 'string' ? contentJson.default_locale : 'zh-CN'),
    smallLogoUrl: recordString(record, 'small_logo_url'),
    status: recordString(record, 'status') || 'draft',
    translations: translations.length > 0 ? translations : initialAdminNews.translations
  };
}

function earnCategoryNameJson(values: EarnCategoryValues) {
  return {
    version: 1,
    default_locale: values.names[0]?.locale.trim() || 'zh-CN',
    items: values.names.map((item) => ({
      locale: requiredString(item.locale, '语言'),
      country: requiredString(item.country, '国家'),
      title: requiredString(item.title, '栏目名称')
    }))
  };
}

function loanProductNameJson(values: LoanProductValues) {
  return {
    version: 1,
    default_locale: values.names[0]?.locale.trim() || 'zh-CN',
    items: values.names.map((item) => ({
      locale: requiredString(item.locale, '语言'),
      country: requiredString(item.country, '国家'),
      title: requiredString(item.title, '产品名称')
    }))
  };
}

function earnCategoryCreateRequestBody(values: EarnCategoryValues, reason: string) {
  return {
    code: requiredString(values.code, '分类代码'),
    name_json: earnCategoryNameJson(values),
    sort_order: requiredNonNegativeInteger(values.sortOrder, '排序值'),
    status: requiredString(values.status, '状态'),
    reason
  };
}

function earnCategoryUpdateRequestBody(values: EarnCategoryValues, reason: string) {
  return {
    name_json: earnCategoryNameJson(values),
    sort_order: requiredNonNegativeInteger(values.sortOrder, '排序值'),
    status: requiredString(values.status, '状态'),
    reason
  };
}

function earnCategoryFromRecord(record: ApiRecord): EarnCategoryValues {
  const nameJson = record.name_json as { items?: unknown } | undefined;
  const items = Array.isArray(nameJson?.items) ? nameJson.items : [];
  const names = items
    .map((item) => {
      const value = item as Record<string, unknown>;
      return {
        locale: typeof value.locale === 'string' ? value.locale : '',
        country: typeof value.country === 'string' ? value.country : typeof value.country_code === 'string' ? value.country_code : '',
        title: typeof value.title === 'string' ? value.title : ''
      };
    })
    .filter((item) => item.locale || item.country || item.title);

  return {
    code: recordString(record, 'code'),
    names: names.length > 0 ? names : initialEarnCategory.names,
    sortOrder: recordString(record, 'sort_order') || '0',
    status: recordString(record, 'status') || 'active'
  };
}

function earnProductRequestBody(values: EarnProductValues, reason: string) {
  return {
    asset_id: requiredPositiveInteger(values.assetId, '理财资产'),
    name: requiredString(values.name, '产品名称'),
    banner_url: optionalString(values.bannerUrl),
    small_logo_url: optionalString(values.smallLogoUrl),
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
    redemption_fee_rate: requiredString(values.redemptionFeeRate, '提现赎回手续费率'),
    maturity_profit_fee_rate: requiredString(values.maturityProfitFeeRate, '到期获利手续费率'),
    early_redeem_fee_basis: requiredString(values.earlyRedeemFeeBasis, '提前赎回扣费基准'),
    early_redeem_fee_rate: values.earlyRedeemFeeBasis === 'none' ? '0' : requiredString(values.earlyRedeemFeeRate, '提前赎回扣费率'),
    min_subscribe: requiredString(values.minSubscribe, '最小申购'),
    max_subscribe: optionalString(values.maxSubscribe),
    status: requiredString(values.status, '状态'),
    reason
  };
}

function earnProductFromRecord(record: ApiRecord): EarnProductValues {
  const introductionJson = record.introduction_json as { items?: unknown } | undefined;
  const items = Array.isArray(introductionJson?.items) ? introductionJson.items : [];
  const introductions = items
    .map((item) => {
      const value = item as Record<string, unknown>;
      return {
        locale: typeof value.locale === 'string' ? value.locale : '',
        country: typeof value.country === 'string' ? value.country : typeof value.country_code === 'string' ? value.country_code : '',
        title: typeof value.title === 'string' ? value.title : '',
        content: Array.isArray(value.content) ? (value.content as RichTextValue) : emptyRichTextValue
      };
    })
    .filter((item) => item.locale || item.country || item.title);

  return {
    assetId: recordString(record, 'asset_id'),
    bannerUrl: recordString(record, 'banner_url'),
    name: recordString(record, 'name'),
    category: recordString(record, 'category') || 'fixed_term',
    termDays: recordString(record, 'term_days'),
    aprRate: recordString(record, 'apr_rate'),
    redemptionFeeRate: recordString(record, 'redemption_fee_rate') || '0',
    maturityProfitFeeRate: recordString(record, 'maturity_profit_fee_rate') || '0',
    earlyRedeemFeeBasis: recordString(record, 'early_redeem_fee_basis') || 'none',
    earlyRedeemFeeRate: recordString(record, 'early_redeem_fee_rate') || '0',
    minSubscribe: recordString(record, 'min_subscribe'),
    maxSubscribe: recordString(record, 'max_subscribe'),
    smallLogoUrl: recordString(record, 'small_logo_url'),
    status: recordString(record, 'status') || 'active',
    introductions: introductions.length > 0 ? introductions : initialEarnProduct.introductions
  };
}

function isLoanProductSubmittable(values: LoanProductValues): boolean {
  const minAmount = Number(values.minAmount);
  const maxAmount = values.maxAmount.trim() ? Number(values.maxAmount) : undefined;
  return Boolean(
    values.names.length > 0 &&
      values.names.every((item) => item.locale.trim() && item.country.trim() && item.title.trim()) &&
      values.assetId.trim() &&
      values.loanType.trim() &&
      values.interestCalculationMode.trim() &&
      values.status.trim() &&
      isNonNegativeIntegerInput(values.minKycLevel) &&
      isNonNegativeIntegerInput(values.termDays) &&
      Number(values.termDays) > 0 &&
      isNonNegativeDecimalInput(values.interestRate) &&
      values.minAmount.trim() &&
      Number.isFinite(minAmount) &&
      minAmount > 0 &&
      (!values.maxAmount.trim() || (Number.isFinite(maxAmount) && Number(maxAmount) >= minAmount))
  );
}

function loanProductRequestBody(values: LoanProductValues, reason: string) {
  const nameJson = loanProductNameJson(values);
  const defaultName = values.names[0]?.title.trim() || values.name.trim();
  return {
    loan_type: requiredString(values.loanType, '贷款类型'),
    asset_id: requiredPositiveInteger(values.assetId, '放款资产'),
    name: requiredString(defaultName, '产品名称'),
    name_json: nameJson,
    term_days: requiredPositiveInteger(values.termDays, '期限天数'),
    interest_rate: requiredNonNegativeDecimal(values.interestRate, '期限利率'),
    interest_calculation_mode: requiredString(values.interestCalculationMode, '计息方式'),
    min_kyc_level: requiredNonNegativeInteger(values.minKycLevel, '最低KYC等级'),
    min_amount: requiredString(values.minAmount, '最小借款金额'),
    max_amount: optionalString(values.maxAmount) ?? null,
    status: requiredString(values.status, '状态'),
    reason
  };
}

function loanProductFromRecord(record: ApiRecord): LoanProductValues {
  const nameJson = record.name_json as { items?: unknown } | undefined;
  const items = Array.isArray(nameJson?.items) ? nameJson.items : [];
  const names = items
    .map((item) => {
      const value = item as Record<string, unknown>;
      return {
        locale: typeof value.locale === 'string' ? value.locale : '',
        country: typeof value.country === 'string' ? value.country : typeof value.country_code === 'string' ? value.country_code : '',
        title: typeof value.title === 'string' ? value.title : ''
      };
    })
    .filter((item) => item.locale || item.country || item.title);
  const fallbackName = recordString(record, 'name');

  return {
    assetId: recordString(record, 'asset_id'),
    interestCalculationMode: recordString(record, 'interest_calculation_mode') || 'full_term',
    interestRate: recordString(record, 'interest_rate'),
    loanType: recordString(record, 'loan_type') || 'credit',
    maxAmount: recordString(record, 'max_amount'),
    minAmount: recordString(record, 'min_amount'),
    minKycLevel: recordString(record, 'min_kyc_level') || '0',
    name: fallbackName,
    names: names.length > 0 ? names : [{ ...initialLoanProduct.names[0], title: fallbackName }],
    status: recordString(record, 'status') || 'active',
    termDays: recordString(record, 'term_days')
  };
}

function LoanProductForm({
  assetLoading,
  assetOptions,
  countries,
  countriesLoading,
  onChange,
  statusLabel,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  countries: AdminNewsCountryOption[];
  countriesLoading: boolean;
  onChange: (values: LoanProductValues) => void;
  statusLabel: string;
  values: LoanProductValues;
}) {
  const countryOptions = earnCountrySelectOptions(countries);

  return (
    <div className="admin-earn-product-layout">
      <section className="admin-earn-product-section" aria-labelledby="loan-product-basic-title">
        <Text strong id="loan-product-basic-title">
          基础配置
        </Text>
        <div className="admin-action-form admin-action-form-wide">
          <label>
            贷款类型
            <AdminSelect ariaLabel="贷款类型" onChange={(loanType) => onChange({ ...values, loanType })} optionList={loanTypeOptions} value={values.loanType} />
          </label>
          <AssetSelect label="放款资产" loading={assetLoading} options={assetOptions} value={values.assetId} onChange={(assetId) => onChange({ ...values, assetId })} />
          <label>期限天数<AdminTextInput ariaLabel="期限天数" value={values.termDays} onChange={(termDays) => onChange({ ...values, termDays })} /></label>
          <label>期限利率<AdminTextInput ariaLabel="期限利率" placeholder="0.02 表示 2%" value={values.interestRate} onChange={(interestRate) => onChange({ ...values, interestRate })} /></label>
          <label>
            提前还款计息方式
            <AdminSelect
              ariaLabel="提前还款计息方式"
              onChange={(interestCalculationMode) => onChange({ ...values, interestCalculationMode })}
              optionList={loanInterestModeOptions}
              value={values.interestCalculationMode}
            />
          </label>
          <label>最低KYC等级<AdminTextInput ariaLabel="最低KYC等级" value={values.minKycLevel} onChange={(minKycLevel) => onChange({ ...values, minKycLevel })} /></label>
          <label>最小借款金额<AdminTextInput ariaLabel="最小借款金额" value={values.minAmount} onChange={(minAmount) => onChange({ ...values, minAmount })} /></label>
          <label>最大借款金额<AdminTextInput ariaLabel="最大借款金额" placeholder="留空表示无上限" value={values.maxAmount} onChange={(maxAmount) => onChange({ ...values, maxAmount })} /></label>
          <label>
            {statusLabel}
            <AdminSelect ariaLabel={statusLabel} onChange={(status) => onChange({ ...values, status })} optionList={activeStatusOptions} value={values.status} />
          </label>
        </div>
      </section>
      <section className="admin-earn-product-section" aria-labelledby="loan-product-name-title">
        <div className="admin-earn-section-header">
          <Text strong id="loan-product-name-title">
            多语言产品名称
          </Text>
          <Button onClick={() => onChange({ ...values, names: [...values.names, newLoanProductName(countries)] })} theme="borderless">
            新增国家名称
          </Button>
        </div>
        <div className="admin-earn-introduction-list">
          {values.names.map((item, index) => {
            const optionList = includeCurrentCountrySelectOption(countryOptions, item.country, item.locale);
            return (
              <Card bordered className="admin-earn-introduction-card" key={index}>
                <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                  <div className="admin-earn-section-header">
                    <Title heading={5}>产品名称 {index + 1}</Title>
                    <Button
                      disabled={values.names.length === 1}
                      onClick={() => {
                        const names = values.names.filter((_, itemIndex) => itemIndex !== index);
                        onChange({ ...values, name: names[0]?.title ?? '', names });
                      }}
                      theme="borderless"
                    >
                      删除
                    </Button>
                  </div>
                  <div className="admin-action-form admin-earn-introduction-meta">
                    <label>
                      国家
                      <AdminSelect
                        ariaLabel="国家"
                        disabled={countriesLoading || optionList.length === 0}
                        filter
                        loading={countriesLoading}
                        onChange={(countryCode) => onChange(applyLoanProductNameCountry(values, index, countries, countryCode))}
                        optionList={optionList}
                        placeholder={countriesLoading ? '加载国家中...' : '请选择国家'}
                        value={item.country}
                      />
                    </label>
                    <label>产品名称<AdminTextInput ariaLabel="产品名称" value={item.title} onChange={(title) => onChange(updateLoanProductName(values, index, { title }))} /></label>
                  </div>
                  <Text type="tertiary">默认语言：{item.locale || '--'}</Text>
                </Space>
              </Card>
            );
          })}
        </div>
      </section>
    </div>
  );
}

export function CreateLoanProductAction({ onCreated }: { onCreated?: () => void }) {
  const [product, setProduct] = useState(initialLoanProduct);
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);

  useEffect(() => {
    if (!visible || countries.length === 0) return;
    setProduct((current) => syncLoanProductCountryLocales(current, countries));
  }, [countries, visible]);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加贷款产品</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加贷款产品" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <LoanProductForm
              assetLoading={assetLoading}
              assetOptions={assetOptions}
              countries={countries}
              countriesLoading={countriesLoading}
              statusLabel="初始状态"
              values={product}
              onChange={setProduct}
            />
            <ConfirmAction
              actionText="提交添加贷款产品"
              disabled={!isLoanProductSubmittable(product)}
              title="确认添加贷款产品"
              onConfirm={async (reason) => {
                await submitAction('添加贷款产品', () =>
                  apiRequest('/admin/api/v1/loan/products', {
                    method: 'POST',
                    body: JSON.stringify(loanProductRequestBody(product, reason))
                  })
                );
                setVisible(false);
                setProduct(initialLoanProduct);
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

function LoanProductEditAction({ helpers, productId, record }: { helpers: RowActionHelpers; productId: string; record: ApiRecord }) {
  const [product, setProduct] = useState(() => loanProductFromRecord(record));
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);
  const assetOptionsWithCurrent = includeCurrentOption(assetOptions, product.assetId, `${recordString(record, 'asset_symbol') || `资产${product.assetId}`}（ID: ${product.assetId}）`);

  useEffect(() => {
    if (!visible || countries.length === 0) return;
    setProduct((current) => syncLoanProductCountryLocales(current, countries));
  }, [countries, visible]);

  return (
    <>
      <Button disabled={!productId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改贷款产品" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <LoanProductForm
              assetLoading={assetLoading}
              assetOptions={assetOptionsWithCurrent}
              countries={countries}
              countriesLoading={countriesLoading}
              statusLabel="状态"
              values={product}
              onChange={setProduct}
            />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isLoanProductSubmittable(product)}
              title="确认修改贷款产品"
              onConfirm={async (reason) => {
                await submitAction('修改贷款产品', () =>
                  apiRequest(`/admin/api/v1/loan/products/${productId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(loanProductRequestBody(product, reason))
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

export function LoanProductRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const productId = recordString(record, 'id');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/loan/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <LoanProductEditAction helpers={helpers} productId={productId} record={record} />
      <ConfirmAction
        actionText={actionText}
        disabled={!productId}
        title={`${actionText}贷款产品`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}贷款产品`, () =>
            apiRequest(`/admin/api/v1/loan/products/${productId}/status`, {
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

export function LoanOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'id');
  const isPending = recordString(record, 'status') === 'pending';

  return (
    <>
      <Button disabled={!orderId} onClick={() => openRecordDetail('/admin/api/v1/loan/orders', orderId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText="审核通过"
        disabled={!orderId || !isPending}
        title="审核通过贷款申请"
        onConfirm={async () => {
          await submitAction('审核通过贷款申请', () => apiRequest(`/admin/api/v1/loan/orders/${orderId}/approve`, { method: 'POST' }));
          helpers.reload();
        }}
      />
      <ConfirmAction
        actionText="拒绝"
        disabled={!orderId || !isPending}
        title="拒绝贷款申请"
        onConfirm={async (reason) => {
          await submitAction('拒绝贷款申请', () =>
            apiRequest(`/admin/api/v1/loan/orders/${orderId}/reject`, {
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

function AssetWithdrawFeeTiersEditor({
  onChange,
  values
}: {
  onChange: (values: AssetWithdrawFeeTierValues[]) => void;
  values: AssetWithdrawFeeTierValues[];
}) {
  const updateTier = (index: number, patch: Partial<AssetWithdrawFeeTierValues>) => {
    onChange(values.map((tier, tierIndex) => (tierIndex === index ? { ...tier, ...patch } : tier)));
  };
  const removeTier = (index: number) => {
    onChange(values.filter((_, tierIndex) => tierIndex !== index));
  };

  return (
    <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
      <Space align="center" style={{ justifyContent: 'space-between', width: '100%' }}>
        <Text strong>提现手续费梯度</Text>
        <Button
          onClick={() => onChange([...values, { minAmount: '', maxAmount: '', feeRatePercent: '' }])}
          size="small"
          theme="light"
          type="primary"
        >
          添加梯度
        </Button>
      </Space>
      {values.length === 0 ? <Text type="tertiary">未配置时使用固定提现手续费。</Text> : null}
      {values.map((tier, index) => (
        <Row gutter={12} key={index} style={{ width: '100%' }}>
          <Col span={7}>
            <label>
              第{index + 1}档最小金额
              <AdminTextInput ariaLabel={`梯度最小金额 ${index + 1}`} value={tier.minAmount} onChange={(minAmount) => updateTier(index, { minAmount })} />
            </label>
          </Col>
          <Col span={7}>
            <label>
              最大金额
              <AdminTextInput ariaLabel={`梯度最大金额 ${index + 1}`} placeholder="留空为无上限" value={tier.maxAmount} onChange={(maxAmount) => updateTier(index, { maxAmount })} />
            </label>
          </Col>
          <Col span={6}>
            <label>
              手续费比例%
              <AdminTextInput ariaLabel={`梯度手续费比例 ${index + 1}`} value={tier.feeRatePercent} onChange={(feeRatePercent) => updateTier(index, { feeRatePercent })} />
            </label>
          </Col>
          <Col span={4}>
            <Button onClick={() => removeTier(index)} size="small" theme="borderless" type="danger">
              删除
            </Button>
          </Col>
        </Row>
      ))}
    </Space>
  );
}

function AssetEditAction({ assetId, helpers, record }: { assetId: string; helpers: RowActionHelpers; record: ApiRecord }) {
  const [config, setConfig] = useState<AssetConfigValues>({
    logoUrl: recordString(record, 'logo_url'),
    name: recordString(record, 'name'),
    precisionScale: recordString(record, 'precision_scale'),
    assetType: recordString(record, 'asset_type') || 'coin',
    status: recordString(record, 'status') || 'active',
    depositEnabled: record.deposit_enabled !== false,
    withdrawEnabled: record.withdraw_enabled !== false,
    minDepositAmount: recordString(record, 'min_deposit_amount') || '0',
    depositFee: recordString(record, 'deposit_fee') || '0',
    withdrawFee: recordString(record, 'withdraw_fee') || '0',
    withdrawFeeTiers: recordWithdrawFeeTiers(record)
  });
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!assetId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改资产配置" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
              <label>资产符号<AdminTextInput ariaLabel="资产符号" readOnly value={recordString(record, 'symbol')} onChange={() => undefined} /></label>
              <label>资产名称<AdminTextInput ariaLabel="资产名称" value={config.name} onChange={(name) => setConfig({ ...config, name })} /></label>
              <AdminImageUpload label="资产 Logo" value={config.logoUrl} variant="avatar" onChange={(logoUrl) => setConfig({ ...config, logoUrl })} />
              <label>资产精度<AdminTextInput ariaLabel="资产精度" value={config.precisionScale} onChange={(precisionScale) => setConfig({ ...config, precisionScale })} /></label>
              <label>资产类型<AssetTypeSelect value={config.assetType} onChange={(assetType) => setConfig({ ...config, assetType })} /></label>
              <label>状态<AssetStatusSelect value={config.status} onChange={(status) => setConfig({ ...config, status })} /></label>
              <AdminSwitch checked={config.depositEnabled} label="支持充值" onChange={(depositEnabled) => setConfig({ ...config, depositEnabled })} />
              <AdminSwitch checked={config.withdrawEnabled} label="支持提现" onChange={(withdrawEnabled) => setConfig({ ...config, withdrawEnabled })} />
              <label>最小充值数量<AdminTextInput ariaLabel="最小充值数量" value={config.minDepositAmount} onChange={(minDepositAmount) => setConfig({ ...config, minDepositAmount })} /></label>
              <label>充值手续费<AdminTextInput ariaLabel="充值手续费" value={config.depositFee} onChange={(depositFee) => setConfig({ ...config, depositFee })} /></label>
              <label>提现手续费<AdminTextInput ariaLabel="提现手续费" value={config.withdrawFee} onChange={(withdrawFee) => setConfig({ ...config, withdrawFee })} /></label>
              <AssetWithdrawFeeTiersEditor values={config.withdrawFeeTiers} onChange={(withdrawFeeTiers) => setConfig({ ...config, withdrawFeeTiers })} />
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
                      logo_url: optionalString(config.logoUrl),
                      precision_scale: requiredNonNegativeInteger(config.precisionScale, '资产精度'),
                      asset_type: requiredString(config.assetType, '资产类型'),
                      status: requiredString(config.status, '状态'),
                      deposit_enabled: config.depositEnabled,
                      withdraw_enabled: config.withdrawEnabled,
                      min_deposit_amount: requiredNonNegativeDecimal(config.minDepositAmount, '最小充值数量'),
                      deposit_fee: requiredNonNegativeDecimal(config.depositFee, '充值手续费'),
                      withdraw_fee: requiredNonNegativeDecimal(config.withdrawFee, '提现手续费'),
                      withdraw_fee_tiers: withdrawFeeTierPayload(config.withdrawFeeTiers),
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

export function AssetRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const assetId = recordString(record, 'id');
  const status = recordString(record, 'status');

  return (
    <>
      <Button disabled={!assetId} onClick={() => openRecordDetail('/admin/api/v1/assets', assetId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AssetEditAction assetId={assetId} helpers={helpers} record={record} />
      {status === 'disabled' ? (
        <ConfirmAction
          actionText="删除"
          disabled={!assetId}
          title="确认删除资产"
          onConfirm={async (reason) => {
            await submitAction('删除资产', () =>
              apiRequest(`/admin/api/v1/assets/${assetId}`, {
                method: 'DELETE',
                body: JSON.stringify({ reason })
              })
            );
            helpers.reload();
          }}
        />
      ) : null}
    </>
  );
}

function DepositAddressPoolCreateFields({
  assetLoading,
  assetOptions,
  networkConfigLoading,
  networkConfigs,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  networkConfigLoading: boolean;
  networkConfigs: DepositNetworkConfigOption[];
  onChange: (values: DepositAddressPoolCreateValues) => void;
  values: DepositAddressPoolCreateValues;
}) {
  const currentNetworkConfig = depositNetworkConfigForNetwork(networkConfigs, values.network);
  const filteredAssetOptions = depositAssetOptionsForNetwork(assetOptions, currentNetworkConfig);
  const networkOptions = depositNetworkSelectOptions(networkConfigs);
  const updateNetwork = (network: string) => {
    const nextConfig = depositNetworkConfigForNetwork(networkConfigs, network);
    onChange({
      ...values,
      network,
      addressGroupCode: nextConfig?.addressGroupCode || defaultDepositAddressGroupCode(network),
      assetSymbols: normalizeSelectedDepositAssetsForNetwork(values.assetSymbols, nextConfig)
    });
  };
  const updateEntry = (index: number, nextEntry: DepositAddressPoolEntryValues) => {
    onChange({
      ...values,
      entries: values.entries.map((entry, entryIndex) => (entryIndex === index ? nextEntry : entry))
    });
  };
  const removeEntry = (index: number) => {
    if (values.entries.length <= 1) {
      return;
    }
    onChange({ ...values, entries: values.entries.filter((_, entryIndex) => entryIndex !== index) });
  };
  const appendEntry = () => {
    onChange({ ...values, entries: [...values.entries, { ...initialDepositAddressPoolEntry }] });
  };
  const importAddressFile = async (request: customRequestArgs) => {
    try {
      const importedEntries = parseDepositAddressImportText(await readDepositAddressImportFile(request.fileInstance));
      const existingEntries = values.entries.filter(hasDepositAddressEntryContent);
      onChange({
        ...values,
        entries: existingEntries.length ? [...existingEntries, ...importedEntries] : importedEntries
      });
      Toast.success(`已导入 ${importedEntries.length} 条充值地址`);
      request.onSuccess({ imported: importedEntries.length });
    } catch (error) {
      Toast.error(errorMessage(error));
      request.onError({ status: 400 });
    }
  };

  return (
    <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
      <section aria-label="充值地址规则" style={{ width: '100%' }}>
        <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
          <strong>地址规则</strong>
          <Row gutter={[16, 16]} style={{ width: '100%' }}>
            <Col xs={24} md={6}>
              <div className="admin-action-form">
                <label>
                  网络
                  <AdminSelect ariaLabel="网络" loading={networkConfigLoading} onChange={updateNetwork} optionList={networkOptions} value={values.network} />
                </label>
              </div>
            </Col>
            <Col xs={24} md={5}>
              <div className="admin-action-form">
                <label>
                  地址集合编号
                  <AdminTextInput ariaLabel="地址集合编号" readOnly value={values.addressGroupCode} onChange={() => undefined} />
                </label>
              </div>
            </Col>
            <Col xs={24} md={8}>
              <div className="admin-action-form">
                <AssetSymbolMultiSelect
                  label="支持币种"
                  loading={assetLoading}
                  options={filteredAssetOptions}
                  value={values.assetSymbols}
                  onChange={(assetSymbols) => onChange({ ...values, assetSymbols })}
                />
              </div>
            </Col>
            <Col xs={24} md={5}>
              <div className="admin-action-form">
                <label>
                  初始状态
                  <AdminSelect ariaLabel="初始状态" onChange={(status) => onChange({ ...values, status })} optionList={depositAddressStatusOptions} value={values.status} />
                </label>
              </div>
            </Col>
            <Col xs={24} md={5}>
              <div className="admin-action-form">
                <label>
                  导入文件
                  <Upload
                    accept=".csv,.txt"
                    action="/admin/local/deposit-address-import"
                    customRequest={importAddressFile}
                    limit={1}
                    onAcceptInvalid={() => Toast.error('请导入 CSV 或 TXT 文件')}
                    showUploadList={false}
                  >
                    <Button icon={<IconUpload aria-hidden="true" />} theme="borderless" type="primary">
                      导入地址
                    </Button>
                  </Upload>
                </label>
              </div>
            </Col>
          </Row>
        </Space>
      </section>

      <section aria-label="充值地址明细" style={{ width: '100%' }}>
        <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
          <Row align="middle" gutter={[16, 16]} justify="space-between" type="flex" style={{ width: '100%' }}>
            <Col>
              <strong>地址明细</strong>
            </Col>
            <Col>
              <Button onClick={appendEntry} theme="borderless" type="primary">
                新增一行
              </Button>
            </Col>
          </Row>
          {values.entries.map((entry, index) => (
            <Card aria-label={`充值地址行 ${index + 1}`} bordered key={index}>
              <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                <Row align="middle" gutter={[16, 16]} justify="space-between" type="flex" style={{ width: '100%' }}>
                  <Col>
                    <strong>地址 {index + 1}</strong>
                  </Col>
                  <Col>
                    <Button disabled={values.entries.length <= 1} onClick={() => removeEntry(index)} theme="borderless" type="danger">
                      删除本行
                    </Button>
                  </Col>
                </Row>
                <Row gutter={[16, 16]}>
                  <Col xs={24} md={14}>
                    <div className="admin-action-form">
                      <label>
                        充值地址
                        <AdminTextInput ariaLabel="充值地址" value={entry.address} onChange={(address) => updateEntry(index, { ...entry, address })} placeholder="0x... / T... / bc1... / ..." />
                      </label>
                    </div>
                  </Col>
                  <Col xs={24} md={10}>
                    <div className="admin-action-form">
                      <label>
                        Memo / Tag
                        <AdminTextInput ariaLabel="Memo / Tag" value={entry.memo} onChange={(memo) => updateEntry(index, { ...entry, memo })} />
                      </label>
                    </div>
                  </Col>
                  <Col span={24}>
                    <div className="admin-action-form">
                      <label>
                        备注
                        <AdminTextArea ariaLabel="备注" autosize value={entry.remark} onChange={(remark) => updateEntry(index, { ...entry, remark })} />
                      </label>
                    </div>
                  </Col>
                </Row>
              </Space>
            </Card>
          ))}
        </Space>
      </section>
    </Space>
  );
}

function DepositAddressPoolFields({
  assetLoading,
  assetOptions,
  networkConfigLoading,
  networkConfigs,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  networkConfigLoading: boolean;
  networkConfigs: DepositNetworkConfigOption[];
  onChange: (values: DepositAddressPoolValues) => void;
  values: DepositAddressPoolValues;
}) {
  const currentNetworkConfig = depositNetworkConfigForNetwork(networkConfigs, values.network);
  const filteredAssetOptions = depositAssetOptionsForNetwork(assetOptions, currentNetworkConfig);
  const updateNetwork = (network: string) => {
    const nextConfig = depositNetworkConfigForNetwork(networkConfigs, network);
    onChange({
      ...values,
      network,
      addressGroupCode: nextConfig?.addressGroupCode || defaultDepositAddressGroupCode(network),
      assetSymbols: normalizeSelectedDepositAssetsForNetwork(values.assetSymbols, nextConfig)
    });
  };
  return (
    <div className="admin-action-form">
      <label>
        网络
        <AdminSelect ariaLabel="网络" loading={networkConfigLoading} onChange={updateNetwork} optionList={depositNetworkSelectOptions(networkConfigs)} value={values.network} />
      </label>
      <label>地址集合编号<AdminTextInput ariaLabel="地址集合编号" readOnly value={values.addressGroupCode} onChange={() => undefined} /></label>
      <label>充值地址<AdminTextInput ariaLabel="充值地址" value={values.address} onChange={(address) => onChange({ ...values, address })} placeholder="0x... / T... / bc1... / ..." /></label>
      <AssetSymbolMultiSelect
        label="限定资产"
        loading={assetLoading}
        options={filteredAssetOptions}
        value={values.assetSymbols}
        onChange={(assetSymbols) => onChange({ ...values, assetSymbols })}
      />
      <label>
        状态
        <AdminSelect ariaLabel="状态" onChange={(status) => onChange({ ...values, status })} optionList={depositAddressStatusOptions} value={values.status} />
      </label>
      <label>Memo / Tag<AdminTextInput ariaLabel="Memo / Tag" value={values.memo} onChange={(memo) => onChange({ ...values, memo })} /></label>
      <label>备注<AdminTextArea ariaLabel="备注" autosize value={values.remark} onChange={(remark) => onChange({ ...values, remark })} /></label>
    </div>
  );
}

function DepositNetworkConfigFields({
  assetLoading,
  assetOptions,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  onChange: (values: DepositNetworkConfigValues) => void;
  values: DepositNetworkConfigValues;
}) {
  return (
    <div className="admin-action-form">
      <label>
        网络
        <AdminSelect ariaLabel="网络" onChange={(network) => onChange({ ...values, network })} optionList={depositNetworkOptions} value={values.network} />
      </label>
      <label>显示名称<AdminTextInput ariaLabel="显示名称" value={values.displayName} onChange={(displayName) => onChange({ ...values, displayName })} /></label>
      <label>地址集合编号<AdminTextInput ariaLabel="地址集合编号" value={values.addressGroupCode} onChange={(addressGroupCode) => onChange({ ...values, addressGroupCode })} placeholder="A / EVM / BTC" /></label>
      <label>地址集合名称<AdminTextInput ariaLabel="地址集合名称" value={values.addressGroupName} onChange={(addressGroupName) => onChange({ ...values, addressGroupName })} placeholder="EVM / Bitcoin / Tron" /></label>
      <AssetSymbolMultiSelect
        label="支持充值币种"
        loading={assetLoading}
        options={assetOptions}
        value={values.assetSymbols}
        onChange={(assetSymbols) => onChange({ ...values, assetSymbols })}
      />
      <label>
        状态
        <AdminSelect ariaLabel="状态" onChange={(status) => onChange({ ...values, status })} optionList={statusOptions} value={values.status} />
      </label>
      <label>排序<AdminTextInput ariaLabel="排序" type="number" value={values.sortOrder} onChange={(sortOrder) => onChange({ ...values, sortOrder })} /></label>
    </div>
  );
}

export function CreateDepositNetworkConfigAction({ onCreated }: CreateActionProps = {}) {
  const [config, setConfig] = useState<DepositNetworkConfigValues>(initialDepositNetworkConfig);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="新增充值网络配置" size="medium" title="新增充值网络配置">
      {({ close }) => (
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <DepositNetworkConfigFields assetLoading={assetLoading} assetOptions={assetOptions} values={config} onChange={setConfig} />
          <Row justify="end" type="flex" style={{ width: '100%' }}>
            <Col>
              <ConfirmAction
                actionText="提交新增"
                disabled={!isDepositNetworkConfigSubmittable(config)}
                title="确认新增充值网络配置"
                onConfirm={async (reason) => {
                  await submitAction('新增充值网络配置', () =>
                    apiRequest('/admin/api/v1/deposit-network-configs', {
                      method: 'POST',
                      body: JSON.stringify(depositNetworkConfigRequestBody(config, reason))
                    })
                  );
                  completeCreate(close, onCreated, () => setConfig(initialDepositNetworkConfig));
                }}
              />
            </Col>
          </Row>
        </Space>
      )}
    </FormModal>
  );
}

export function DepositNetworkConfigRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const configId = recordString(record, 'id');
  const [config, setConfig] = useState<DepositNetworkConfigValues>(() => depositNetworkConfigFromRecord(record));
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);

  return (
    <>
      <Button disabled={!configId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改充值网络配置" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <DepositNetworkConfigFields assetLoading={assetLoading} assetOptions={assetOptions} values={config} onChange={setConfig} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isDepositNetworkConfigSubmittable(config)}
              title="确认修改充值网络配置"
              onConfirm={async (reason) => {
                await submitAction('修改充值网络配置', () =>
                  apiRequest(`/admin/api/v1/deposit-network-configs/${configId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(depositNetworkConfigRequestBody(config, reason))
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

export function CreateDepositAddressPoolAction({ onCreated }: CreateActionProps = {}) {
  const [addressPool, setAddressPool] = useState(createInitialDepositAddressPoolCreate);
  const { assetLoading, assetOptions } = useAssetOptions();
  const { networkConfigLoading, networkConfigs } = useDepositNetworkConfigOptions();

  useEffect(() => {
    if (networkConfigs.length === 0) {
      return;
    }
    setAddressPool((current) => {
      const addressGroupCode = depositAddressGroupForNetwork(networkConfigs, current.network, current.addressGroupCode);
      if (addressGroupCode === current.addressGroupCode) {
        return current;
      }
      return { ...current, addressGroupCode };
    });
  }, [networkConfigs]);

  return (
    <FormModal actionText="添加充值地址" size="extra-wide" title="添加充值地址">
      {({ close }) => (
        <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
          <DepositAddressPoolCreateFields
            assetLoading={assetLoading}
            assetOptions={assetOptions}
            networkConfigLoading={networkConfigLoading}
            networkConfigs={networkConfigs}
            values={addressPool}
            onChange={setAddressPool}
          />
          <Row justify="end" type="flex" style={{ width: '100%' }}>
            <Col>
            <ConfirmAction
              actionText="提交添加"
              disabled={!isDepositAddressPoolCreateSubmittable(addressPool)}
              title="确认添加充值地址"
              onConfirm={async (reason) => {
                await submitAction('添加充值地址', () =>
                  apiRequest('/admin/api/v1/deposit-address-pool/batch', {
                    method: 'POST',
                    body: JSON.stringify(depositAddressPoolBatchRequestBody(addressPool, reason))
                  })
                );
                completeCreate(close, onCreated, () => setAddressPool(createInitialDepositAddressPoolCreate()));
              }}
            />
            </Col>
          </Row>
        </Space>
      )}
    </FormModal>
  );
}

function DepositAddressPoolEditAction({ addressId, helpers, record }: { addressId: string; helpers: RowActionHelpers; record: ApiRecord }) {
  const [config, setConfig] = useState<DepositAddressPoolValues>(() => depositAddressPoolFromRecord(record));
  const [visible, setVisible] = useState(false);
  const assigned = recordString(record, 'status') === 'assigned';
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { networkConfigLoading, networkConfigs } = useDepositNetworkConfigOptions(visible);

  return (
    <>
      <Button disabled={!addressId || assigned} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改充值地址" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <DepositAddressPoolFields
              assetLoading={assetLoading}
              assetOptions={assetOptions}
              networkConfigLoading={networkConfigLoading}
              networkConfigs={networkConfigs}
              values={config}
              onChange={setConfig}
            />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isDepositAddressPoolSubmittable(config)}
              title="确认修改充值地址"
              onConfirm={async (reason) => {
                await submitAction('修改充值地址', () =>
                  apiRequest(`/admin/api/v1/deposit-address-pool/${addressId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(depositAddressPoolRequestBody(config, reason))
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

export function DepositAddressPoolRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const addressId = recordString(record, 'id');
  const assigned = recordString(record, 'status') === 'assigned';

  return (
    <>
      <Button disabled={!addressId} onClick={() => openRecordDetail('/admin/api/v1/deposit-address-pool', addressId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <DepositAddressPoolEditAction addressId={addressId} helpers={helpers} record={record} />
      {assigned ? (
        <ConfirmAction
          actionText="回收"
          disabled={!addressId}
          title="回收充值地址"
          onConfirm={async (reason) => {
            await submitAction('回收充值地址', () =>
              apiRequest(`/admin/api/v1/deposit-address-pool/${addressId}/reclaim`, {
                method: 'POST',
                body: JSON.stringify({ reason })
              })
            );
            helpers.reload();
          }}
        />
      ) : null}
    </>
  );
}

export function QuickRechargeOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'order_id');
  const status = recordString(record, 'status');
  const canDelete = status !== 'paid';

  return (
    <>
      <Button disabled={!orderId} onClick={() => helpers.openDetail({ title: '快速充值订单详情', data: record })} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText="删除"
        disabled={!orderId || !canDelete}
        title="确认删除快速充值订单"
        onConfirm={async (reason) => {
          await submitAction('删除快速充值订单', () =>
            apiRequest(`/admin/api/v1/quick-recharge/orders/${encodeURIComponent(orderId)}`, {
              method: 'DELETE',
              body: JSON.stringify({ reason })
            })
          );
          helpers.reload();
        }}
      />
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
      <SideSheet onCancel={() => setVisible(false)} title="用户充值" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
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
      </SideSheet>
    </>
  );
}

function AssignAgentAction({ helpers, userId }: { helpers: RowActionHelpers; userId: string }) {
  const [assignment, setAssignment] = useState(initialAssignAgent);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!userId} onClick={() => setVisible(true)} size="small" theme="borderless">
        分配代理
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="分配代理" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
              <label>用户ID<AdminTextInput ariaLabel="用户ID" readOnly value={userId} onChange={() => undefined} /></label>
              <label>代理ID<AdminTextInput ariaLabel="代理ID" value={assignment.agentId} onChange={(agentId) => setAssignment({ ...assignment, agentId })} /></label>
            </div>
            <ConfirmAction
              actionText="提交分配代理"
              disabled={!isAssignAgentSubmittable(assignment)}
              title="确认分配代理"
              onConfirm={async (reason) => {
                await submitAction('分配代理', () =>
                  apiRequest(`/admin/api/v1/users/${userId}/agent`, {
                    method: 'PATCH',
                    body: JSON.stringify({ agent_id: requiredPositiveInteger(assignment.agentId, '代理ID'), reason })
                  })
                );
                setVisible(false);
                setAssignment(initialAssignAgent);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

function ResetUserTwoFactorAction({ helpers, userId }: { helpers: RowActionHelpers; userId: string }) {
  return (
    <ConfirmAction
      actionText="重置2FA"
      disabled={!userId}
      title="重置用户2FA"
      onConfirm={async (reason) => {
        await submitAction('重置用户2FA', () =>
          apiRequest(`/admin/api/v1/users/${userId}/2fa/reset`, {
            method: 'POST',
            body: JSON.stringify({ reason })
          })
        );
        helpers.reload();
      }}
    />
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
      <AssignAgentAction helpers={helpers} userId={userId} />
      <ResetUserTwoFactorAction helpers={helpers} userId={userId} />
    </>
  );
}

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

export function SecondsProductRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const productId = recordString(record, 'id');
  const status = recordString(record, 'status');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/seconds-contracts/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <SecondsProductEditAction helpers={helpers} productId={productId} record={record} />
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
      {status === 'disabled' ? (
        <ConfirmAction
          actionText="删除"
          disabled={!productId}
          title="确认删除秒合约产品"
          onConfirm={async (reason) => {
            await submitAction('删除秒合约产品', () =>
              apiRequest(`/admin/api/v1/seconds-contracts/products/${productId}`, {
                method: 'DELETE',
                body: JSON.stringify({ reason })
              })
            );
            helpers.reload();
          }}
        />
      ) : null}
    </>
  );
}

function SecondsProductPeriodsEditor({
  onAdd,
  onRemove,
  onUpdate,
  periods
}: {
  onAdd: () => void;
  onRemove: (index: number) => void;
  onUpdate: (index: number, patch: Partial<SecondsProductPeriodValues>) => void;
  periods: SecondsProductPeriodValues[];
}) {
  return (
    <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
      <div className="admin-earn-section-header">
        <Title heading={5}>周期配置</Title>
        <Button onClick={onAdd} theme="borderless">
          新增周期
        </Button>
      </div>
      {periods.map((period, index) => (
        <div className="admin-action-form admin-action-form-wide" key={index}>
          <label>周期秒数<AdminTextInput ariaLabel="周期秒数" value={period.durationSeconds} onChange={(durationSeconds) => onUpdate(index, { durationSeconds })} /></label>
          <label>赔率<AdminTextInput ariaLabel="赔率" value={period.payoutRate} onChange={(payoutRate) => onUpdate(index, { payoutRate })} /></label>
          <label>最小押注<AdminTextInput ariaLabel="最小押注" value={period.minStake} onChange={(minStake) => onUpdate(index, { minStake })} /></label>
          <label>
            最大押注
            <AdminTextInput ariaLabel="最大押注" placeholder="留空表示无上限" value={period.maxStake} onChange={(maxStake) => onUpdate(index, { maxStake })} />
          </label>
          <Button disabled={periods.length === 1} onClick={() => onRemove(index)} theme="borderless">
            删除周期
          </Button>
        </div>
      ))}
    </Space>
  );
}

function SecondsProductEditAction({ helpers, productId, record }: { helpers: RowActionHelpers; productId: string; record: ApiRecord }) {
  const [config, setConfig] = useState(() => secondsProductFromRecord(record));
  const [activeTab, setActiveTab] = useState<SecondsProductTab>('basic');
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { pairLoading, pairOptions } = useMarketPairOptions(visible);
  const pairOptionsWithCurrent = includeCurrentOption(pairOptions, config.pairId, `${recordString(record, 'symbol') || `交易对${config.pairId}`}（ID: ${config.pairId}）`);
  const assetOptionsWithCurrent = includeCurrentOption(
    assetOptions,
    config.stakeAsset,
    `${recordString(record, 'stake_asset_symbol') || `资产${config.stakeAsset}`}（ID: ${config.stakeAsset}）`
  );
  const updatePeriod = (index: number, patch: Partial<SecondsProductPeriodValues>) => {
    setConfig((current) => ({
      ...current,
      periods: current.periods.map((period, periodIndex) => (periodIndex === index ? { ...period, ...patch } : period))
    }));
  };
  const addPeriod = () => {
    setConfig((current) => ({
      ...current,
      periods: [...current.periods, { ...initialSecondsProductPeriod }]
    }));
  };
  const removePeriod = (index: number) => {
    setConfig((current) => ({
      ...current,
      periods: current.periods.length > 1 ? current.periods.filter((_, periodIndex) => periodIndex !== index) : current.periods
    }));
  };

  return (
    <>
      <Button disabled={!productId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改秒合约产品" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Tabs
              activeKey={activeTab}
              className="admin-seconds-product-tabs"
              onChange={(nextTab) => setActiveTab(nextTab as SecondsProductTab)}
              tabList={secondsProductTabs}
              type="button"
            />
            {activeTab === 'basic' ? (
              <div className="admin-action-form admin-action-form-wide">
                <label>产品ID<AdminTextInput ariaLabel="产品ID" readOnly value={productId} onChange={() => undefined} /></label>
                <MarketPairSelect
                  label="秒合约交易对"
                  loading={pairLoading}
                  options={pairOptionsWithCurrent}
                  value={config.pairId}
                  onChange={(pairId) => setConfig({ ...config, pairId })}
                />
                <AssetSelect
                  label="押注资产"
                  loading={assetLoading}
                  options={assetOptionsWithCurrent}
                  value={config.stakeAsset}
                  onChange={(stakeAsset) => setConfig({ ...config, stakeAsset })}
                />
                <AdminImageUpload label="秒合约交易对 Logo" value={config.logoUrl} variant="avatar" onChange={(logoUrl) => setConfig({ ...config, logoUrl })} />
                <label>
                  状态
                  <AdminSelect ariaLabel="状态" onChange={(status) => setConfig({ ...config, status })} optionList={statusOptions} value={config.status} />
                </label>
              </div>
            ) : (
              <SecondsProductPeriodsEditor periods={config.periods} onAdd={addPeriod} onRemove={removePeriod} onUpdate={updatePeriod} />
            )}
            <div className="admin-action-footer">
              <ConfirmAction
                actionText="提交修改"
                disabled={!isSecondsProductCreatable(config)}
                title="确认修改秒合约产品"
                onConfirm={async (reason) => {
                  await submitAction('修改秒合约产品', () =>
                    apiRequest(`/admin/api/v1/seconds-contracts/products/${productId}`, {
                      method: 'PATCH',
                      body: JSON.stringify(secondsProductRequestBody(config, reason))
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

function EarnCategoryForm({
  countries,
  countriesLoading,
  includeCode,
  onChange,
  values
}: {
  countries: AdminNewsCountryOption[];
  countriesLoading: boolean;
  includeCode: boolean;
  onChange: (values: EarnCategoryValues) => void;
  values: EarnCategoryValues;
}) {
  const countryOptions = earnCountrySelectOptions(countries);

  return (
    <div className="admin-earn-product-layout">
      <section className="admin-earn-product-section" aria-labelledby="earn-category-basic-title">
        <Text strong id="earn-category-basic-title">
          基础配置
        </Text>
        <div className="admin-action-form admin-earn-product-basic-grid">
          <label>
            分类代码
            <AdminTextInput
              ariaLabel="分类代码"
              disabled={!includeCode}
              value={values.code}
              onChange={(code) => onChange({ ...values, code })}
              placeholder="fixed_term"
            />
          </label>
          <label>排序值<AdminTextInput ariaLabel="排序值" value={values.sortOrder} onChange={(sortOrder) => onChange({ ...values, sortOrder })} /></label>
          <label>
            状态
            <AdminSelect ariaLabel="状态" onChange={(status) => onChange({ ...values, status })} optionList={activeStatusOptions} value={values.status} />
          </label>
        </div>
      </section>
      <section className="admin-earn-product-section" aria-labelledby="earn-category-name-title">
        <div className="admin-earn-section-header">
          <Text strong id="earn-category-name-title">
            多语言栏目名称
          </Text>
          <Button onClick={() => onChange({ ...values, names: [...values.names, newEarnCategoryName(countries)] })} theme="borderless">
            新增国家名称
          </Button>
        </div>
        <div className="admin-earn-introduction-list">
          {values.names.map((item, index) => {
            const optionList = includeCurrentCountrySelectOption(countryOptions, item.country, item.locale);
            return (
              <Card bordered className="admin-earn-introduction-card" key={index}>
                <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                  <div className="admin-earn-section-header">
                    <Title heading={5}>国家名称 {index + 1}</Title>
                    <Button disabled={values.names.length === 1} onClick={() => onChange({ ...values, names: values.names.filter((_, itemIndex) => itemIndex !== index) })} theme="borderless">
                      删除
                    </Button>
                  </div>
                  <div className="admin-action-form admin-earn-introduction-meta">
                    <label>
                      国家
                      <AdminSelect
                        ariaLabel="国家"
                        disabled={countriesLoading || optionList.length === 0}
                        filter
                        loading={countriesLoading}
                        onChange={(countryCode) => onChange(applyEarnCategoryNameCountry(values, index, countries, countryCode))}
                        optionList={optionList}
                        placeholder={countriesLoading ? '加载国家中...' : '请选择国家'}
                        value={item.country}
                      />
                    </label>
                    <label>栏目名称<AdminTextInput ariaLabel="栏目名称" value={item.title} onChange={(title) => onChange(updateEarnCategoryName(values, index, { title }))} /></label>
                  </div>
                  <Text type="tertiary">默认语言：{item.locale || '--'}</Text>
                </Space>
              </Card>
            );
          })}
        </div>
      </section>
    </div>
  );
}

export function CreateEarnCategoryAction({ onCreated }: { onCreated?: () => void }) {
  const [category, setCategory] = useState(initialEarnCategory);
  const [visible, setVisible] = useState(false);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);

  useEffect(() => {
    if (!visible || countries.length === 0) return;
    setCategory((current) => syncEarnCategoryCountryLocales(current, countries));
  }, [countries, visible]);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加理财分类</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加理财分类" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <EarnCategoryForm countries={countries} countriesLoading={countriesLoading} includeCode values={category} onChange={setCategory} />
            <div className="admin-earn-product-footer">
              <ConfirmAction
                actionText="提交添加理财分类"
                disabled={!isEarnCategorySubmittable(category, true)}
                title="确认添加理财分类"
                onConfirm={async (reason) => {
                  await submitAction('添加理财分类', () =>
                    apiRequest('/admin/api/v1/earn/categories', {
                      method: 'POST',
                      body: JSON.stringify(earnCategoryCreateRequestBody(category, reason))
                    })
                  );
                  setVisible(false);
                  setCategory(initialEarnCategory);
                  onCreated?.();
                }}
              />
            </div>
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

function EarnCategoryEditAction({ categoryId, helpers, record }: { categoryId: string; helpers: RowActionHelpers; record: ApiRecord }) {
  const [category, setCategory] = useState(() => earnCategoryFromRecord(record));
  const [visible, setVisible] = useState(false);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);

  useEffect(() => {
    if (!visible || countries.length === 0) return;
    setCategory((current) => syncEarnCategoryCountryLocales(current, countries));
  }, [countries, visible]);

  return (
    <>
      <Button disabled={!categoryId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改理财分类" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <EarnCategoryForm countries={countries} countriesLoading={countriesLoading} includeCode={false} values={category} onChange={setCategory} />
            <div className="admin-earn-product-footer">
              <ConfirmAction
                actionText="提交修改"
                disabled={!isEarnCategorySubmittable(category, false)}
                title="确认修改理财分类"
                onConfirm={async (reason) => {
                  await submitAction('修改理财分类', () =>
                    apiRequest(`/admin/api/v1/earn/categories/${categoryId}`, {
                      method: 'PATCH',
                      body: JSON.stringify(earnCategoryUpdateRequestBody(category, reason))
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

export function EarnCategoryRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const categoryId = recordString(record, 'id');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!categoryId} onClick={() => openRecordDetail('/admin/api/v1/earn/categories', categoryId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <EarnCategoryEditAction categoryId={categoryId} helpers={helpers} record={record} />
      <ConfirmAction
        actionText={actionText}
        disabled={!categoryId}
        title={`${actionText}理财分类`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}理财分类`, () =>
            apiRequest(`/admin/api/v1/earn/categories/${categoryId}/status`, {
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

function EarnProductForm({
  assetLoading,
  assetOptions,
  categoryLoading,
  categoryOptions,
  countries,
  countriesLoading,
  onChange,
  statusLabel,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  categoryLoading: boolean;
  categoryOptions: SemiSelectOption[];
  countries: AdminNewsCountryOption[];
  countriesLoading: boolean;
  onChange: (values: EarnProductValues) => void;
  statusLabel: string;
  values: EarnProductValues;
}) {
  const countryOptions = earnCountrySelectOptions(countries);
  const productCategoryOptions = includeCurrentSelectOption(categoryOptions, values.category, recordCategoryFallbackLabel(values.category));

  return (
    <div className="admin-earn-product-layout">
      <section className="admin-earn-product-section" aria-labelledby="earn-product-basic-title">
        <Text strong id="earn-product-basic-title">
          基础信息
        </Text>
        <div className="admin-action-form admin-earn-product-basic-grid">
          <AssetSelect label="理财资产" loading={assetLoading} options={assetOptions} value={values.assetId} onChange={(assetId) => onChange({ ...values, assetId })} />
          <label>产品名称<AdminTextInput ariaLabel="产品名称" value={values.name} onChange={(name) => onChange({ ...values, name })} /></label>
          <AdminImageUpload label="理财 Banner" value={values.bannerUrl} variant="banner" onChange={(bannerUrl) => onChange({ ...values, bannerUrl })} />
          <AdminImageUpload label="理财小 Logo" value={values.smallLogoUrl} variant="avatar" onChange={(smallLogoUrl) => onChange({ ...values, smallLogoUrl })} />
          <label>
            产品分类
            <AdminSelect
              ariaLabel="产品分类"
              disabled={categoryLoading || productCategoryOptions.length === 0}
              filter
              loading={categoryLoading}
              onChange={(category) => onChange({ ...values, category })}
              optionList={productCategoryOptions}
              placeholder={categoryLoading ? '加载分类中...' : '请选择分类'}
              value={values.category}
            />
          </label>
          <label>
            {statusLabel}
            <AdminSelect ariaLabel={statusLabel} onChange={(status) => onChange({ ...values, status })} optionList={activeStatusOptions} value={values.status} />
          </label>
        </div>
      </section>
      <section className="admin-earn-product-section" aria-labelledby="earn-product-yield-title">
        <Text strong id="earn-product-yield-title">
          收益与申购参数
        </Text>
        <div className="admin-action-form admin-earn-product-basic-grid">
          <label>期限天数<AdminTextInput ariaLabel="期限天数" value={values.termDays} onChange={(termDays) => onChange({ ...values, termDays })} /></label>
          <label>年化利率<AdminTextInput ariaLabel="年化利率" value={values.aprRate} onChange={(aprRate) => onChange({ ...values, aprRate })} /></label>
          <label>最小申购<AdminTextInput ariaLabel="最小申购" value={values.minSubscribe} onChange={(minSubscribe) => onChange({ ...values, minSubscribe })} /></label>
          <label>最大申购<AdminTextInput ariaLabel="最大申购" value={values.maxSubscribe} onChange={(maxSubscribe) => onChange({ ...values, maxSubscribe })} /></label>
        </div>
      </section>
      <section className="admin-earn-product-section" aria-labelledby="earn-product-fee-title">
        <Text strong id="earn-product-fee-title">
          手续费配置
        </Text>
        <div className="admin-action-form admin-earn-product-basic-grid">
          <label>提现赎回手续费率<AdminTextInput ariaLabel="提现赎回手续费率" value={values.redemptionFeeRate} onChange={(redemptionFeeRate) => onChange({ ...values, redemptionFeeRate })} /></label>
          <label>到期获利手续费率<AdminTextInput ariaLabel="到期获利手续费率" value={values.maturityProfitFeeRate} onChange={(maturityProfitFeeRate) => onChange({ ...values, maturityProfitFeeRate })} /></label>
          <label>
            提前赎回扣费基准
            <AdminSelect
              ariaLabel="提前赎回扣费基准"
              onChange={(earlyRedeemFeeBasis) =>
                onChange({
                  ...values,
                  earlyRedeemFeeBasis,
                  earlyRedeemFeeRate: earlyRedeemFeeBasis === 'none' ? '0' : values.earlyRedeemFeeRate
                })
              }
              optionList={earnEarlyRedeemFeeBasisOptions}
              value={values.earlyRedeemFeeBasis}
            />
          </label>
          <label>
            提前赎回扣费率
            <AdminTextInput
              ariaLabel="提前赎回扣费率"
              disabled={values.earlyRedeemFeeBasis === 'none'}
              value={values.earlyRedeemFeeRate}
              onChange={(earlyRedeemFeeRate) => onChange({ ...values, earlyRedeemFeeRate })}
            />
          </label>
        </div>
      </section>
      <section className="admin-earn-product-section" aria-labelledby="earn-product-introduction-title">
        <div className="admin-earn-section-header">
          <Text strong id="earn-product-introduction-title">
            多国语言介绍
          </Text>
          <Button onClick={() => onChange({ ...values, introductions: [...values.introductions, newEarnIntroduction(countries)] })} theme="borderless">
            新增国家介绍
          </Button>
        </div>
        <div className="admin-earn-introduction-list">
          {values.introductions.map((item, index) => {
            const optionList = includeCurrentCountrySelectOption(countryOptions, item.country, item.locale);
            return (
              <Card bordered className="admin-earn-introduction-card" key={index}>
                <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                  <Title heading={5}>国家版本 {index + 1}</Title>
                  <div className="admin-action-form admin-earn-introduction-meta">
                    <label>
                      国家
                      <AdminSelect
                        ariaLabel="国家"
                        disabled={countriesLoading || optionList.length === 0}
                        loading={countriesLoading}
                        onChange={(countryCode) => onChange(applyEarnIntroductionCountry(values, index, countries, countryCode))}
                        optionList={optionList}
                        placeholder={countriesLoading ? '加载国家中...' : '请选择国家'}
                        value={item.country}
                      />
                    </label>
                    <label>介绍标题<AdminTextInput ariaLabel="介绍标题" value={item.title} onChange={(title) => onChange(updateEarnIntroduction(values, index, { title }))} /></label>
                  </div>
                  <Text type="tertiary">默认语言：{item.locale || '--'}</Text>
                  <QuillRichTextEditor value={item.content} onChange={(content) => onChange(updateEarnIntroduction(values, index, { content }))} />
                </Space>
              </Card>
            );
          })}
        </div>
      </section>
    </div>
  );
}

export function CreateEarnProductAction({ onCreated }: { onCreated?: () => void }) {
  const [product, setProduct] = useState(initialEarnProduct);
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { categoryLoading, categoryOptions } = useEarnCategoryOptions(visible);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);

  useEffect(() => {
    if (!visible || countries.length === 0) return;
    setProduct((current) => syncEarnProductCountryLocales(current, countries));
  }, [countries, visible]);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加理财产品</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加理财产品" visible={visible} {...createModalProps('extra-wide')}>
        <Card bordered={false}>
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <EarnProductForm
              assetLoading={assetLoading}
              assetOptions={assetOptions}
              categoryLoading={categoryLoading}
              categoryOptions={categoryOptions}
              countries={countries}
              countriesLoading={countriesLoading}
              statusLabel="初始状态"
              values={product}
              onChange={setProduct}
            />
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
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

function EarnProductEditAction({ helpers, productId, record }: { helpers: RowActionHelpers; productId: string; record: ApiRecord }) {
  const [product, setProduct] = useState(() => earnProductFromRecord(record));
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { categoryLoading, categoryOptions } = useEarnCategoryOptions(visible);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);
  const assetOptionsWithCurrent = includeCurrentOption(assetOptions, product.assetId, `${recordString(record, 'asset_symbol') || `资产${product.assetId}`}（ID: ${product.assetId}）`);
  const categoryOptionsWithCurrent = includeCurrentSelectOption(
    categoryOptions,
    product.category,
    recordString(record, 'category_name') ? `${recordString(record, 'category_name')}（${product.category}）` : product.category
  );

  useEffect(() => {
    if (!visible || countries.length === 0) return;
    setProduct((current) => syncEarnProductCountryLocales(current, countries));
  }, [countries, visible]);

  return (
    <>
      <Button disabled={!productId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改理财产品" visible={visible} {...createModalProps('extra-wide')}>
        <Card bordered={false}>
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <EarnProductForm
              assetLoading={assetLoading}
              assetOptions={assetOptionsWithCurrent}
              categoryLoading={categoryLoading}
              categoryOptions={categoryOptionsWithCurrent}
              countries={countries}
              countriesLoading={countriesLoading}
              statusLabel="状态"
              values={product}
              onChange={setProduct}
            />
            <div className="admin-earn-product-footer">
              <ConfirmAction
                actionText="提交修改"
                disabled={!isEarnProductCreatable(product)}
                title="确认修改理财产品"
                onConfirm={async (reason) => {
                  await submitAction('修改理财产品', () =>
                    apiRequest(`/admin/api/v1/earn/products/${productId}`, {
                      method: 'PATCH',
                      body: JSON.stringify(earnProductRequestBody(product, reason))
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

function AdminNewsForm({ includeStatus, onChange, values }: { includeStatus: boolean; onChange: (values: AdminNewsValues) => void; values: AdminNewsValues }) {
  return (
    <div className="admin-earn-product-layout">
      <section className="admin-earn-product-section" aria-labelledby="admin-news-basic-title">
        <Text strong id="admin-news-basic-title">
          基础信息
        </Text>
        <div className="admin-action-form admin-earn-product-basic-grid">
          <label>新闻标题<AdminTextInput ariaLabel="新闻标题" value={values.title} onChange={(title) => onChange({ ...values, title })} /></label>
          <AdminImageUpload label="新闻 Banner" value={values.bannerUrl} variant="banner" onChange={(bannerUrl) => onChange({ ...values, bannerUrl })} />
          <AdminImageUpload label="新闻小 Logo" value={values.smallLogoUrl} variant="avatar" onChange={(smallLogoUrl) => onChange({ ...values, smallLogoUrl })} />
          <label>
            分类
            <AdminSelect ariaLabel="分类" onChange={(category) => onChange({ ...values, category })} optionList={newsCategoryOptions} value={values.category} />
          </label>
          <label>国家<AdminTextInput ariaLabel="国家" value={values.countryCode} onChange={(countryCode) => onChange({ ...values, countryCode })} placeholder="CN" /></label>
          <label>默认语言<AdminTextInput ariaLabel="默认语言" value={values.defaultLocale} onChange={(defaultLocale) => onChange({ ...values, defaultLocale })} placeholder="zh-CN" /></label>
          {includeStatus ? (
            <label>
              初始状态
              <AdminSelect ariaLabel="初始状态" onChange={(status) => onChange({ ...values, status })} optionList={newsStatusOptions} value={values.status} />
            </label>
          ) : null}
        </div>
      </section>
      <section className="admin-earn-product-section" aria-labelledby="admin-news-translations-title">
        <div className="admin-earn-section-header">
          <Text strong id="admin-news-translations-title">
            多语言内容
          </Text>
          <Button onClick={() => onChange({ ...values, translations: [...values.translations, newAdminNewsTranslation()] })} theme="borderless">
            新增语言内容
          </Button>
        </div>
        <div className="admin-earn-introduction-list">
          {values.translations.map((item, index) => (
            <Card bordered className="admin-earn-introduction-card" key={index}>
              <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                <Title heading={5}>语言内容 {index + 1}</Title>
                <div className="admin-action-form admin-earn-introduction-meta">
                  <label>语言<AdminTextInput ariaLabel="语言" value={item.locale} onChange={(locale) => onChange(updateAdminNewsTranslation(values, index, { locale }))} /></label>
                  <label>翻译国家<AdminTextInput ariaLabel="翻译国家" value={item.countryCode} onChange={(countryCode) => onChange(updateAdminNewsTranslation(values, index, { countryCode }))} /></label>
                  <label>翻译标题<AdminTextInput ariaLabel="翻译标题" value={item.title} onChange={(title) => onChange(updateAdminNewsTranslation(values, index, { title }))} /></label>
                </div>
                <div className="admin-news-summary-field">
                  <Text strong>摘要</Text>
                  <QuillRichTextEditor ariaLabel="摘要" placeholder="请输入新闻摘要" value={item.summary} onChange={(summary) => onChange(updateAdminNewsTranslation(values, index, { summary }))} />
                </div>
                <QuillRichTextEditor enableImageUpload placeholder="请输入新闻内容" value={item.content} onChange={(content) => onChange(updateAdminNewsTranslation(values, index, { content }))} />
              </Space>
            </Card>
          ))}
        </div>
      </section>
    </div>
  );
}

function AdminNewsCreateForm({
  countries,
  countriesLoading,
  onChange,
  values
}: {
  countries: AdminNewsCountryOption[];
  countriesLoading: boolean;
  onChange: (values: AdminNewsValues) => void;
  values: AdminNewsValues;
}) {
  const translation = values.translations[0] ?? newAdminNewsTranslation();
  const updateCreateContent = (patch: Partial<AdminNewsTranslationValues>) => {
    onChange(syncAdminNewsCreateContent({ ...values, translations: [{ ...translation, ...patch }] }));
  };
  const selectCountry = (countryCode: string) => {
    const country = countries.find((item) => item.countryCode === countryCode);
    if (!country) {
      onChange(syncAdminNewsCreateContent({ ...values, countryCode, defaultLocale: '' }));
      return;
    }
    onChange(applyAdminNewsCountry(values, country));
  };

  return (
    <div className="admin-news-create-layout">
      <div className="admin-news-create-side">
        <section className="admin-earn-product-section" aria-labelledby="admin-news-create-publish-title">
          <Text strong id="admin-news-create-publish-title">
            发布设置
          </Text>
          <div className="admin-action-form admin-news-create-settings-grid">
            <label className="admin-news-create-title-field">新闻标题<AdminTextInput ariaLabel="新闻标题" value={values.title} onChange={(title) => onChange(syncAdminNewsCreateContent({ ...values, title }))} /></label>
            <label>
              国家
              <AdminSelect
                ariaLabel="国家"
                disabled={countries.length === 0}
                loading={countriesLoading}
                onChange={selectCountry}
                optionList={adminNewsCountrySelectOptions(countries)}
                placeholder={countriesLoading ? '加载国家中...' : '请选择国家'}
                value={values.countryCode}
              />
            </label>
            <label>
              分类
              <AdminSelect ariaLabel="分类" onChange={(category) => onChange({ ...values, category })} optionList={newsCategoryOptions} value={values.category} />
            </label>
            <label>
              初始状态
              <AdminSelect ariaLabel="初始状态" onChange={(status) => onChange({ ...values, status })} optionList={newsStatusOptions} value={values.status} />
            </label>
          </div>
        </section>
        <section className="admin-earn-product-section" aria-labelledby="admin-news-create-media-title">
          <Text strong id="admin-news-create-media-title">
            视觉素材
          </Text>
          <div className="admin-news-create-media-grid">
            <AdminImageUpload label="新闻 Banner" value={values.bannerUrl} variant="banner" onChange={(bannerUrl) => onChange(syncAdminNewsCreateContent({ ...values, bannerUrl }))} />
            <AdminImageUpload label="新闻小 Logo" value={values.smallLogoUrl} variant="avatar" onChange={(smallLogoUrl) => onChange(syncAdminNewsCreateContent({ ...values, smallLogoUrl }))} />
          </div>
        </section>
      </div>
      <section className="admin-earn-product-section admin-news-create-content-panel" aria-labelledby="admin-news-create-content-title">
        <Text strong id="admin-news-create-content-title">
          内容编辑
        </Text>
        <Space align="start" spacing={14} vertical style={{ width: '100%' }}>
          <div className="admin-news-create-summary-field admin-news-summary-field">
            <Text strong>摘要</Text>
            <QuillRichTextEditor ariaLabel="摘要" placeholder="请输入新闻摘要" value={translation.summary} onChange={(summary) => updateCreateContent({ summary })} />
          </div>
          <div className="admin-news-create-editor">
            <QuillRichTextEditor enableImageUpload placeholder="请输入新闻内容" value={translation.content} onChange={(content) => updateCreateContent({ content })} />
          </div>
        </Space>
      </section>
    </div>
  );
}

export function CreateAdminNewsAction({ onCreated }: { onCreated?: () => void }) {
  const [news, setNews] = useState(initialAdminNews);
  const [visible, setVisible] = useState(false);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加新闻</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加新闻" visible={visible} {...createModalProps('extra-wide')}>
        <div className="admin-news-create-shell">
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <AdminNewsCreateForm countries={countries} countriesLoading={countriesLoading} values={news} onChange={setNews} />
            <ConfirmAction
              actionText="提交添加新闻"
              disabled={!isAdminNewsCreateSubmittable(news)}
              title="确认添加新闻"
              onConfirm={async (reason) => {
                await submitAction('添加新闻', () =>
                  apiRequest('/admin/api/v1/news', {
                    method: 'POST',
                    body: JSON.stringify(adminNewsCreateRequestBody(news, reason))
                  })
                );
                setVisible(false);
                setNews(initialAdminNews);
                onCreated?.();
              }}
            />
          </Space>
        </div>
      </SideSheet>
    </>
  );
}

function AdminNewsEditAction({ helpers, newsId, record }: { helpers: RowActionHelpers; newsId: string; record: ApiRecord }) {
  const [news, setNews] = useState(() => adminNewsFromRecord(record));
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!newsId} onClick={() => setVisible(true)} size="small" theme="borderless">
        编辑
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="编辑新闻" visible={visible} {...createModalProps('extra-wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <AdminNewsForm includeStatus={false} values={news} onChange={setNews} />
            <ConfirmAction
              actionText="提交编辑新闻"
              disabled={!isAdminNewsSubmittable(news)}
              title="确认编辑新闻"
              onConfirm={async (reason) => {
                await submitAction('编辑新闻', () =>
                  apiRequest(`/admin/api/v1/news/${newsId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(adminNewsUpdateRequestBody(news, reason))
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

export function AdminNewsRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const newsId = recordString(record, 'id');

  return (
    <>
      <Button disabled={!newsId} onClick={() => openRecordDetail('/admin/api/v1/news', newsId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AdminNewsEditAction helpers={helpers} newsId={newsId} record={record} />
      <ConfirmAction
        actionText="发布"
        disabled={!newsId || recordString(record, 'status') === 'published'}
        title="发布新闻"
        onConfirm={async (reason) => {
          await submitAction('发布新闻', () =>
            apiRequest(`/admin/api/v1/news/${newsId}/status`, {
              method: 'PATCH',
              body: JSON.stringify({ status: 'published', reason })
            })
          );
          helpers.reload();
        }}
      />
      <ConfirmAction
        actionText="归档"
        disabled={!newsId || recordString(record, 'status') === 'archived'}
        title="归档新闻"
        onConfirm={async (reason) => {
          await submitAction('归档新闻', () =>
            apiRequest(`/admin/api/v1/news/${newsId}/status`, {
              method: 'PATCH',
              body: JSON.stringify({ status: 'archived', reason })
            })
          );
          helpers.reload();
        }}
      />
    </>
  );
}

function CountryForm({ includeCountryCode, includeStatus, onChange, values }: { includeCountryCode: boolean; includeStatus: boolean; onChange: (values: CountryValues) => void; values: CountryValues }) {
  return (
    <div className="admin-action-form">
      <label>
        国家代码
        <AdminTextInput ariaLabel="国家代码" readOnly={!includeCountryCode} value={values.countryCode} onChange={(countryCode) => onChange({ ...values, countryCode })} placeholder="CN" />
      </label>
      <label>国家名称<AdminTextInput ariaLabel="国家名称" value={values.countryName} onChange={(countryName) => onChange({ ...values, countryName })} placeholder="日本" /></label>
      <label>备注（中文名称）<AdminTextInput ariaLabel="备注（中文名称）" value={values.remark} onChange={(remark) => onChange({ ...values, remark })} placeholder="中国" /></label>
      <label>
        默认语言
        <AdminSelect ariaLabel="默认语言" onChange={(defaultLocale) => onChange({ ...values, defaultLocale })} optionList={localeOptions} value={values.defaultLocale} />
      </label>
      <label>支持语言<AdminTextInput ariaLabel="支持语言" value={values.supportedLocales} onChange={(supportedLocales) => onChange({ ...values, supportedLocales })} placeholder="zh,en" /></label>
      <label>开放注册<BooleanSelect label="开放注册" optionList={countryBooleanOptions} value={values.registrationEnabled} onChange={(registrationEnabled) => onChange({ ...values, registrationEnabled })} /></label>
      {includeStatus ? (
        <label>
          初始状态
          <AdminSelect ariaLabel="初始状态" onChange={(status) => onChange({ ...values, status })} optionList={countryStatusOptions} value={values.status} />
        </label>
      ) : null}
      <label>排序<AdminTextInput ariaLabel="排序" value={values.sortOrder} onChange={(sortOrder) => onChange({ ...values, sortOrder })} /></label>
    </div>
  );
}

export function CreateCountryAction({ onCreated }: { onCreated?: () => void }) {
  const [country, setCountry] = useState(initialCountry);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加国家</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加国家" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <CountryForm includeCountryCode includeStatus values={country} onChange={setCountry} />
            <ConfirmAction
              actionText="提交添加国家"
              disabled={!isCountrySubmittable(country, true)}
              title="确认添加国家"
              onConfirm={async (reason) => {
                await submitAction('添加国家', () =>
                  apiRequest('/admin/api/v1/countries', {
                    method: 'POST',
                    body: JSON.stringify(countryCreateRequestBody(country, reason))
                  })
                );
                setVisible(false);
                setCountry(initialCountry);
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

function CountryEditAction({ countryId, helpers, record }: { countryId: string; helpers: RowActionHelpers; record: ApiRecord }) {
  const [country, setCountry] = useState(() => countryFromRecord(record));
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!countryId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改国家配置" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <CountryForm includeCountryCode={false} includeStatus={false} values={country} onChange={setCountry} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isCountrySubmittable(country, false)}
              title="确认修改国家配置"
              onConfirm={async (reason) => {
                await submitAction('修改国家配置', () =>
                  apiRequest(`/admin/api/v1/countries/${countryId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(countryUpdateRequestBody(country, reason))
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

export function CountryRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const countryId = recordString(record, 'id');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = nextStatus === 'disabled' ? '停用' : '启用';

  return (
    <>
      <Button disabled={!countryId} onClick={() => openRecordDetail('/admin/api/v1/countries', countryId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <CountryEditAction countryId={countryId} helpers={helpers} record={record} />
      <ConfirmAction
        actionText={actionText}
        disabled={!countryId}
        title={`${actionText}国家配置`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}国家配置`, () =>
            apiRequest(`/admin/api/v1/countries/${countryId}/status`, {
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

export function EarnProductRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const productId = recordString(record, 'id');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/earn/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <EarnProductEditAction helpers={helpers} productId={productId} record={record} />
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

function ConvertPairEditAction({ helpers, pairId, record }: { helpers: RowActionHelpers; pairId: string; record: ApiRecord }) {
  const [config, setConfig] = useState(() => convertPairFromRecord(record));
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const assetOptionsWithCurrent = includeCurrentOption(
    includeCurrentOption(assetOptions, config.fromAssetId, `${recordString(record, 'from_asset_symbol') || `资产${config.fromAssetId}`}（ID: ${config.fromAssetId}）`),
    config.toAssetId,
    `${recordString(record, 'to_asset_symbol') || `资产${config.toAssetId}`}（ID: ${config.toAssetId}）`
  );

  return (
    <>
      <Button disabled={!pairId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改闪兑交易对" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <ConvertPairFields assetLoading={assetLoading} assetOptions={assetOptionsWithCurrent} values={config} onChange={setConfig} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isConvertPairCreatable(config)}
              title="确认修改闪兑交易对"
              onConfirm={async (reason) => {
                await submitAction('修改闪兑交易对', () =>
                  apiRequest(`/admin/api/v1/convert/pairs/${pairId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(convertPairUpdateRequestBody(config, reason))
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
      <ConvertPairEditAction helpers={helpers} pairId={pairId} record={record} />
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
      {!enabled ? (
        <ConfirmAction
          actionText="删除"
          disabled={!pairId}
          title="确认删除闪兑交易对"
          onConfirm={async (reason) => {
            await submitAction('删除闪兑交易对', () =>
              apiRequest(`/admin/api/v1/convert/pairs/${pairId}`, {
                method: 'DELETE',
                body: JSON.stringify({ reason })
              })
            );
            helpers.reload();
          }}
        />
      ) : null}
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

function AgentCommissionRuleForm({ includeAgentId, onChange, values }: { includeAgentId: boolean; onChange: (values: AgentCommissionRuleValues) => void; values: AgentCommissionRuleValues }) {
  return (
    <div className="admin-action-form">
      {includeAgentId ? <label>代理ID<AdminTextInput ariaLabel="代理ID" value={values.agentId} onChange={(agentId) => onChange({ ...values, agentId })} /></label> : null}
      {!includeAgentId ? <label>代理ID<AdminTextInput ariaLabel="代理ID" readOnly value={values.agentId} onChange={() => undefined} /></label> : null}
      <label>
        产品类型
        <AdminSelect ariaLabel="产品类型" onChange={(productType) => onChange({ ...values, productType })} optionList={agentCommissionRuleProductOptions} value={values.productType} />
      </label>
      <label>佣金比例<AdminTextInput ariaLabel="佣金比例" value={values.commissionRate} onChange={(commissionRate) => onChange({ ...values, commissionRate })} /></label>
      <label>
        {includeAgentId ? '初始状态' : '状态'}
        <AdminSelect ariaLabel={includeAgentId ? '初始状态' : '状态'} onChange={(status) => onChange({ ...values, status })} optionList={statusOptions} value={values.status} />
      </label>
    </div>
  );
}

export function CreateAgentCommissionRuleAction({ onCreated }: { onCreated?: () => void }) {
  const [rule, setRule] = useState(initialAgentCommissionRule);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加佣金规则</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加佣金规则" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <AgentCommissionRuleForm includeAgentId values={rule} onChange={setRule} />
            <ConfirmAction
              actionText="提交添加佣金规则"
              disabled={!isAgentCommissionRuleSubmittable(rule, true)}
              title="确认添加佣金规则"
              onConfirm={async (reason) => {
                await submitAction('添加佣金规则', () =>
                  apiRequest('/admin/api/v1/agent-commission-rules', {
                    method: 'POST',
                    body: JSON.stringify({
                      agent_id: requiredPositiveInteger(rule.agentId, '代理ID'),
                      product_type: rule.productType,
                      commission_rate: requiredString(rule.commissionRate, '佣金比例'),
                      status: rule.status,
                      reason
                    })
                  })
                );
                setVisible(false);
                setRule(initialAgentCommissionRule);
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

export function AgentCommissionRuleRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const ruleId = recordString(record, 'id');
  const [rule, setRule] = useState<AgentCommissionRuleValues>({
    agentId: recordString(record, 'agent_id'),
    productType: recordString(record, 'product_type') || 'convert',
    commissionRate: recordString(record, 'commission_rate'),
    status: recordString(record, 'status') || 'active'
  });
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!ruleId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改佣金规则" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <AgentCommissionRuleForm includeAgentId={false} values={rule} onChange={setRule} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isAgentCommissionRuleSubmittable(rule, false)}
              title="确认修改佣金规则"
              onConfirm={async (reason) => {
                await submitAction('修改佣金规则', () =>
                  apiRequest(`/admin/api/v1/agent-commission-rules/${ruleId}`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      commission_rate: requiredString(rule.commissionRate, '佣金比例'),
                      status: rule.status,
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

export function AgentCommissionRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const commissionId = recordString(record, 'id');
  const canUpdate = recordString(record, 'status') === 'pending';

  async function updateStatus(status: 'settled' | 'rejected', reason: string) {
    await submitAction(status === 'settled' ? '结算代理佣金' : '拒绝代理佣金', () =>
      apiRequest(`/admin/api/v1/agent-commissions/${commissionId}/status`, {
        method: 'PATCH',
        body: JSON.stringify({ status, reason })
      })
    );
    helpers.reload();
  }

  return (
    <>
      <ConfirmAction actionText="结算" disabled={!commissionId || !canUpdate} title="结算代理佣金" onConfirm={(reason) => updateStatus('settled', reason)} />
      <ConfirmAction actionText="拒绝" disabled={!commissionId || !canUpdate} title="拒绝代理佣金" onConfirm={(reason) => updateStatus('rejected', reason)} />
    </>
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
      </SideSheet>
    </>
  );
}

export function CreateUserAction({ onCreated }: CreateActionProps = {}) {
  const [user, setUser] = useState(initialUser);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>添加用户</AdminModalTriggerButton>
      <SideSheet onCancel={() => setVisible(false)} title="添加用户" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
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
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

export function CreateAssetAction({ onCreated }: CreateActionProps = {}) {
  const [asset, setAsset] = useState(initialAsset);

  return (
    <FormModal actionText="添加资产" title="添加资产">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <label>资产符号<AdminTextInput ariaLabel="资产符号" value={asset.symbol} onChange={(symbol) => setAsset({ ...asset, symbol })} placeholder="BTC" /></label>
            <label>资产名称<AdminTextInput ariaLabel="资产名称" value={asset.name} onChange={(name) => setAsset({ ...asset, name })} placeholder="Bitcoin" /></label>
            <AdminImageUpload label="资产 Logo" value={asset.logoUrl} variant="avatar" onChange={(logoUrl) => setAsset({ ...asset, logoUrl })} />
            <label>资产精度<AdminTextInput ariaLabel="资产精度" value={asset.precisionScale} onChange={(precisionScale) => setAsset({ ...asset, precisionScale })} /></label>
            <label>资产类型<AssetTypeSelect value={asset.assetType} onChange={(assetType) => setAsset({ ...asset, assetType })} /></label>
            <label>初始状态<AssetStatusSelect value={asset.status} onChange={(status) => setAsset({ ...asset, status })} /></label>
            <AdminSwitch checked={asset.depositEnabled} label="支持充值" onChange={(depositEnabled) => setAsset({ ...asset, depositEnabled })} />
            <AdminSwitch checked={asset.withdrawEnabled} label="支持提现" onChange={(withdrawEnabled) => setAsset({ ...asset, withdrawEnabled })} />
            <label>最小充值数量<AdminTextInput ariaLabel="最小充值数量" value={asset.minDepositAmount} onChange={(minDepositAmount) => setAsset({ ...asset, minDepositAmount })} /></label>
            <label>充值手续费<AdminTextInput ariaLabel="充值手续费" value={asset.depositFee} onChange={(depositFee) => setAsset({ ...asset, depositFee })} /></label>
            <label>提现手续费<AdminTextInput ariaLabel="提现手续费" value={asset.withdrawFee} onChange={(withdrawFee) => setAsset({ ...asset, withdrawFee })} /></label>
            <AssetWithdrawFeeTiersEditor values={asset.withdrawFeeTiers} onChange={(withdrawFeeTiers) => setAsset({ ...asset, withdrawFeeTiers })} />
          </div>
          <ConfirmAction
            actionText="提交添加资产"
            disabled={!isAssetCreatable(asset)}
            title="确认添加资产"
            onConfirm={async (reason) => {
              await submitAction('添加资产', () =>
                apiRequest('/admin/api/v1/assets', {
                  method: 'POST',
                  body: JSON.stringify({
                    symbol: requiredString(asset.symbol, '资产符号'),
                    name: requiredString(asset.name, '资产名称'),
                    logo_url: optionalString(asset.logoUrl),
                    precision_scale: requiredNonNegativeInteger(asset.precisionScale, '资产精度'),
                    asset_type: asset.assetType,
                    status: asset.status,
                    deposit_enabled: asset.depositEnabled,
                    withdraw_enabled: asset.withdrawEnabled,
                    min_deposit_amount: requiredNonNegativeDecimal(asset.minDepositAmount, '最小充值数量'),
                    deposit_fee: requiredNonNegativeDecimal(asset.depositFee, '充值手续费'),
                    withdraw_fee: requiredNonNegativeDecimal(asset.withdrawFee, '提现手续费'),
                    withdraw_fee_tiers: withdrawFeeTierPayload(asset.withdrawFeeTiers),
                    reason
                  })
                })
              );
              completeCreate(close, onCreated, () => setAsset(initialAsset));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
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

function ConvertPairFields({
  assetLoading,
  assetOptions,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  onChange: (values: ConvertPairValues) => void;
  values: ConvertPairValues;
}) {
  const patch = (nextValues: Partial<ConvertPairValues>) => onChange({ ...values, ...nextValues });

  return (
    <div className="admin-action-form">
      <AssetSelect label="源资产" loading={assetLoading} options={assetOptions} value={values.fromAssetId} onChange={(fromAssetId) => patch({ fromAssetId })} />
      <AssetSelect label="目标资产" loading={assetLoading} options={assetOptions} value={values.toAssetId} onChange={(toAssetId) => patch({ toAssetId })} />
      <label>
        定价模式
        <AdminSelect
          ariaLabel="定价模式"
          onChange={(pricingMode) => patch({ pricingMode })}
          optionList={[
            { value: 'fixed', label: '固定价格' },
            { value: 'market', label: '市场价格' }
          ]}
          value={values.pricingMode}
        />
      </label>
      <label>价差率<AdminTextInput ariaLabel="价差率" value={values.spreadRate} onChange={(spreadRate) => patch({ spreadRate })} /></label>
      <label>手续费率<AdminTextInput ariaLabel="手续费率" value={values.feeRate} onChange={(feeRate) => patch({ feeRate })} /></label>
      <label>源资产最小金额<AdminTextInput ariaLabel="源资产最小金额" value={values.minAmount} onChange={(minAmount) => patch({ minAmount })} /></label>
      <label>源资产最大金额<AdminTextInput ariaLabel="源资产最大金额" value={values.maxAmount} onChange={(maxAmount) => patch({ maxAmount })} /></label>
      <label>目标资产最小金额<AdminTextInput ariaLabel="目标资产最小金额" value={values.targetMinAmount} onChange={(targetMinAmount) => patch({ targetMinAmount })} /></label>
      <label>目标资产最大金额<AdminTextInput ariaLabel="目标资产最大金额" value={values.targetMaxAmount} onChange={(targetMaxAmount) => patch({ targetMaxAmount })} /></label>
      <label>启用<BooleanSelect label="启用" value={values.enabled} onChange={(enabled) => patch({ enabled })} /></label>
    </div>
  );
}

export function CreateConvertPairAction({ onCreated }: CreateActionProps = {}) {
  const [convertPair, setConvertPair] = useState(initialConvertPair);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="添加闪兑交易对" size="wide" title="添加闪兑交易对">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <ConvertPairFields assetLoading={assetLoading} assetOptions={assetOptions} values={convertPair} onChange={setConvertPair} />
          <ConfirmAction
            actionText="提交添加闪兑交易对"
            disabled={!isConvertPairCreatable(convertPair)}
            title="确认添加闪兑交易对"
            onConfirm={async (reason) => {
              await submitAction('添加闪兑交易对', async () => {
                await apiRequest('/admin/api/v1/convert/pairs', {
                  method: 'POST',
                  body: JSON.stringify(convertPairRequestBody(convertPair, reason))
                });
              });
              completeCreate(close, onCreated, () => setConvertPair(initialConvertPair));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}

export function CreateRiskRuleAction({ onCreated }: CreateActionProps = {}) {
  const [riskRule, setRiskRule] = useState(initialRiskRule);

  return (
    <FormModal actionText="添加风控规则" size="wide" title="添加风控规则">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
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
            onConfirm={async (reason) => {
              let configJson: unknown;
              try {
                configJson = JSON.parse(riskRule.configJson);
              } catch {
                Toast.error('规则配置JSON格式错误');
                return;
              }

              await submitAction('添加风控规则', () =>
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
              completeCreate(close, onCreated, () => setRiskRule(initialRiskRule));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}

export function CreateNewCoinProjectAction({ onCreated }: CreateActionProps = {}) {
  const [project, setProject] = useState(initialNewCoinProject);
  const { assetLoading, assetOptions } = useAssetOptions();
  const unlockFeeEnabled = booleanFromSelect(project.unlockFeeEnabled);
  const selectProjectAsset = (assetId: string) => {
    const selectedAsset = assetOptions.find((asset) => asset.id === assetId);
    setProject({ ...project, assetId, symbol: selectedAsset?.symbol || project.symbol });
  };
  const selectProjectSymbol = (symbol: string) => {
    const selectedAsset = assetOptions.find((asset) => asset.symbol === symbol);
    setProject({ ...project, assetId: selectedAsset?.id || project.assetId, symbol });
  };

  return (
    <FormModal actionText="添加新币项目" size="extra-wide" title="添加新币项目">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <AssetSelect label="项目资产" loading={assetLoading} options={assetOptions} value={project.assetId} onChange={selectProjectAsset} />
            <AssetSymbolSelect label="项目符号" loading={assetLoading} options={assetOptions} value={project.symbol} onChange={selectProjectSymbol} />
            <label>
              生命周期
              <AdminSelect
                ariaLabel="生命周期"
                onChange={(lifecycleStatus) => setProject({ ...project, lifecycleStatus })}
                optionList={newCoinLifecycleOptions}
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
                optionList={newCoinUnlockTypeOptions}
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
            onConfirm={async (reason) => {
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

              await submitAction('添加新币项目', () =>
                apiRequest('/admin/api/v1/new-coins', {
                  method: 'POST',
                  body: JSON.stringify(body)
                })
              );
              completeCreate(close, onCreated, () => setProject(initialNewCoinProject));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}

export function CreateSecondsPairAction({ onCreated }: CreateActionProps = {}) {
  const [secondsProduct, setSecondsProduct] = useState(initialSecondsProduct);
  const [activeTab, setActiveTab] = useState<SecondsProductTab>('basic');
  const { assetLoading, assetOptions } = useAssetOptions();
  const { pairLoading, pairOptions } = useMarketPairOptions();
  const updatePeriod = (index: number, patch: Partial<SecondsProductPeriodValues>) => {
    setSecondsProduct((current) => ({
      ...current,
      periods: current.periods.map((period, periodIndex) => (periodIndex === index ? { ...period, ...patch } : period))
    }));
  };
  const addPeriod = () => {
    setSecondsProduct((current) => ({
      ...current,
      periods: [...current.periods, { ...initialSecondsProductPeriod }]
    }));
  };
  const removePeriod = (index: number) => {
    setSecondsProduct((current) => ({
      ...current,
      periods: current.periods.length > 1 ? current.periods.filter((_, periodIndex) => periodIndex !== index) : current.periods
    }));
  };

  return (
    <FormModal actionText="添加秒合约交易对" size="wide" title="添加秒合约交易对">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <Tabs
            activeKey={activeTab}
            className="admin-seconds-product-tabs"
            onChange={(nextTab) => setActiveTab(nextTab as SecondsProductTab)}
            tabList={secondsProductTabs}
            type="button"
          />
          {activeTab === 'basic' ? (
            <div className="admin-action-form admin-action-form-wide">
              <MarketPairSelect
                label="秒合约交易对"
                loading={pairLoading}
                options={pairOptions}
                value={secondsProduct.pairId}
                onChange={(pairId) => setSecondsProduct({ ...secondsProduct, pairId })}
              />
              <AssetSelect
                label="押注资产"
                loading={assetLoading}
                options={assetOptions}
                value={secondsProduct.stakeAsset}
                onChange={(stakeAsset) => setSecondsProduct({ ...secondsProduct, stakeAsset })}
              />
              <AdminImageUpload label="秒合约交易对 Logo" value={secondsProduct.logoUrl} variant="avatar" onChange={(logoUrl) => setSecondsProduct({ ...secondsProduct, logoUrl })} />
              <label>
                初始状态
                <AdminSelect ariaLabel="初始状态" onChange={(status) => setSecondsProduct({ ...secondsProduct, status })} optionList={statusOptions} value={secondsProduct.status} />
              </label>
            </div>
          ) : (
            <SecondsProductPeriodsEditor periods={secondsProduct.periods} onAdd={addPeriod} onRemove={removePeriod} onUpdate={updatePeriod} />
          )}
          <div className="admin-action-footer">
            <ConfirmAction
              actionText="提交添加秒合约交易对"
              disabled={!isSecondsProductCreatable(secondsProduct)}
              title="确认添加秒合约交易对"
              onConfirm={async (reason) => {
                await submitAction('添加秒合约交易对', () =>
                  apiRequest('/admin/api/v1/seconds-contracts/products', {
                    method: 'POST',
                    body: JSON.stringify(secondsProductRequestBody(secondsProduct, reason))
                  })
                );
                completeCreate(close, onCreated, () => {
                  setSecondsProduct(initialSecondsProduct);
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
