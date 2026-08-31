import { Button, Card, SideSheet, Space, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { AdminRequestActionBoundary } from '../../access';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { compareDecimalText, isPositiveDecimalText } from '../../../shared/decimal';
import { AdminModalTriggerButton, AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  type AdminNewsCountryOption,
  type AssetOption,
  AssetSelect,
  type RowActionHelpers,
  activeStatusOptions,
  createModalProps,
  earnCountrySelectOptions,
  includeCurrentCountrySelectOption,
  includeCurrentOption,
  isNonNegativeDecimalInput,
  isNonNegativeIntegerInput,
  nextToggleStatus,
  openRecordDetail,
  recordString,
  requiredNonNegativeDecimal,
  requiredNonNegativeInteger,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  toggleActionText,
  useAdminCountryOptions,
  useAssetOptions
} from './shared';

const { Text, Title } = Typography;

export const LOAN_PRODUCT_REVISION_CONFLICT_MESSAGE = '贷款产品已被其他管理员更新，列表已刷新，请重新打开后确认最新配置。';

class LoanProductRevisionConflictError extends Error {
  constructor() {
    super(LOAN_PRODUCT_REVISION_CONFLICT_MESSAGE);
    this.name = 'LoanProductRevisionConflictError';
  }
}

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

const loanTypeOptions: SemiSelectOption[] = [
  { value: 'credit', label: '信用贷' },
  { value: 'collateralized', label: '抵押贷' }
];

const loanInterestModeOptions: SemiSelectOption[] = [
  { value: 'full_term', label: '完整周期计息' },
  { value: 'actual_days', label: '按实际天数计息' }
];

function newLoanProductName(countries: AdminNewsCountryOption[] = []): LoanProductNameItemValues {
  const country = countries.find((item) => item.countryCode === 'US') ?? countries[0];
  return {
    locale: country?.defaultLocale ?? 'en-US',
    country: country?.countryCode ?? 'US',
    title: ''
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

function applyLoanProductNameCountry(values: LoanProductValues, index: number, countries: AdminNewsCountryOption[], countryCode: string): LoanProductValues {
  const normalizedCountryCode = countryCode.trim().toUpperCase();
  const country = countries.find((item) => item.countryCode === normalizedCountryCode);
  const current = values.names[index];
  return updateLoanProductName(values, index, {
    country: country?.countryCode ?? normalizedCountryCode,
    locale: country?.defaultLocale ?? current?.locale ?? ''
  });
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

function isLoanProductSubmittable(values: LoanProductValues): boolean {
  const maximumComparison = values.maxAmount.trim() ? compareDecimalText(values.maxAmount, values.minAmount) : null;
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
      isPositiveDecimalText(values.minAmount) &&
      (!values.maxAmount.trim() || maximumComparison === 0 || maximumComparison === 1)
  );
}

function loanProductRequestBody(values: LoanProductValues, reason: string, revision?: number) {
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
    min_amount: requiredNonNegativeDecimal(values.minAmount, '最小借款金额'),
    max_amount: values.maxAmount.trim() ? requiredNonNegativeDecimal(values.maxAmount, '最大借款金额') : null,
    status: requiredString(values.status, '状态'),
    reason,
    ...(revision === undefined ? {} : { revision })
  };
}

function loanProductRevision(record: ApiRecord): number | null {
  const revision = Number(recordString(record, 'revision'));
  return Number.isSafeInteger(revision) && revision > 0 ? revision : null;
}

async function submitLoanProductMutation(label: string, request: () => Promise<unknown>, onConflict: () => void): Promise<boolean> {
  try {
    await submitAction(label, async () => {
      try {
        await request();
      } catch (error) {
        if (error instanceof ApiError && error.status === 409) {
          onConflict();
          throw new LoanProductRevisionConflictError();
        }
        throw error;
      }
    });
    return true;
  } catch (error) {
    if (error instanceof LoanProductRevisionConflictError) {
      return false;
    }
    throw error;
  }
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
  const revision = loanProductRevision(record);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { countries, countriesLoading } = useAdminCountryOptions(visible);
  const assetOptionsWithCurrent = includeCurrentOption(assetOptions, product.assetId, `${recordString(record, 'asset_symbol') || `资产${product.assetId}`}（ID: ${product.assetId}）`);

  useEffect(() => {
    if (!visible || countries.length === 0) return;
    setProduct((current) => syncLoanProductCountryLocales(current, countries));
  }, [countries, visible]);

  return (
    <>
      <Button
        disabled={!productId || revision === null}
        onClick={() => {
          setProduct(loanProductFromRecord(record));
          setVisible(true);
        }}
        size="small"
        theme="borderless"
      >
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
              disabled={!isLoanProductSubmittable(product) || revision === null}
              title="确认修改贷款产品"
              onConfirm={async (reason) => {
                if (revision === null) return;
                const updated = await submitLoanProductMutation(
                  '修改贷款产品',
                  () =>
                    apiRequest(`/admin/api/v1/loan/products/${productId}`, {
                      method: 'PATCH',
                      body: JSON.stringify(loanProductRequestBody(product, reason, revision))
                    }),
                  () => {
                    setVisible(false);
                    helpers.reload();
                  }
                );
                if (!updated) return;
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
  const revision = loanProductRevision(record);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/loan/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/loan/products/${productId}`} method="PATCH">
        <LoanProductEditAction helpers={helpers} productId={productId} record={record} />
        <ConfirmAction
        actionText={actionText}
        disabled={!productId || revision === null}
        title={`${actionText}贷款产品`}
        onConfirm={async (reason) => {
          if (revision === null) return;
          const updated = await submitLoanProductMutation(
            `${actionText}贷款产品`,
            () =>
              apiRequest(`/admin/api/v1/loan/products/${productId}/status`, {
                method: 'PATCH',
                body: JSON.stringify({ status: nextStatus, reason, revision })
              }),
            helpers.reload
          );
          if (!updated) return;
          helpers.reload();
        }}
        />
      </AdminRequestActionBoundary>
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
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/loan/orders/${orderId}/approve`} method="POST">
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
      </AdminRequestActionBoundary>
    </>
  );
}
