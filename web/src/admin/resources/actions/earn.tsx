import { Button, Card, SideSheet, Space, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';

import { listAdminResource } from '../../../api/adminResources';
import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { QuillRichTextEditor, type RichTextValue } from '../../../shared/QuillRichTextEditor';
import { AdminModalTriggerButton, AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  type AdminNewsCountryOption,
  type AssetOption,
  AssetSelect,
  type RowActionHelpers,
  activeStatusOptions,
  createModalProps,
  earnCountrySelectOptions,
  emptyRichTextValue,
  includeCurrentCountrySelectOption,
  includeCurrentOption,
  isNonNegativeIntegerInput,
  nextToggleStatus,
  openRecordDetail,
  optionalString,
  recordString,
  requiredNonNegativeInteger,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  toggleActionText,
  useAdminCountryOptions,
  useAssetOptions
} from './shared';

const { Text, Title } = Typography;

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

const earnEarlyRedeemFeeBasisOptions: SemiSelectOption[] = [
  { value: 'none', label: '不扣费' },
  { value: 'principal', label: '按本金比例扣除' },
  { value: 'profit', label: '按收益比例扣除' }
];

function toEarnCategoryOption(category: ApiRecord): SemiSelectOption | null {
  const code = recordString(category, 'code');
  const name = recordString(category, 'default_name') || code;
  return code ? { value: code, label: `${name}（${code}）` } : null;
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

function updateEarnCategoryName(values: EarnCategoryValues, index: number, patch: Partial<EarnCategoryNameItemValues>): EarnCategoryValues {
  return {
    ...values,
    names: values.names.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item))
  };
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
