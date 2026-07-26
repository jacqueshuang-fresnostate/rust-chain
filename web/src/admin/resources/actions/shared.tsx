import { SideSheet, Toast } from '@douyinfe/semi-ui';
import { type ReactNode, useEffect, useState } from 'react';

import { listAdminResource } from '../../../api/adminResources';
import { ApiError, apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import type { DetailDrawerData } from '../../../shared/DetailDrawer';
import type { RichTextValue } from '../../../shared/QuillRichTextEditor';
import { AdminModalTriggerButton, AdminSelect, type SemiSelectOption } from '../../../shared/SemiFormControls';

export type AssetOption = {
  id: string;
  label: string;
  symbol: string;
};

export type MarketPairOption = {
  id: string;
  label: string;
};

export type AdminNewsCountryOption = {
  countryCode: string;
  countryName: string;
  defaultLocale: string;
};

export type RowActionHelpers = {
  reload: () => void;
  openDetail: (detail: DetailDrawerData) => void;
};

type CreateModalSize = 'medium' | 'wide' | 'extra-wide';

export type CreateActionProps = {
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

export function createModalProps(size: CreateModalSize) {
  return {
    bodyStyle: { overflowY: 'auto' as const },
    className: `admin-create-modal admin-create-modal-${size}`,
    closeOnEsc: true,
    maskClosable: false,
    width: createModalWidths[size]
  };
}

export const emptyRichTextValue: RichTextValue = [{ type: 'p', children: [{ text: '' }] }];

export const activeStatusOptions: SemiSelectOption[] = [
  { value: 'active', label: '启用' },
  { value: 'disabled', label: '禁用' }
];

export function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

export function requiredPositiveInteger(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
  }
  return parsed;
}

export function requiredNonNegativeInteger(value: string, label: string): number {
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

export function isNonNegativeIntegerInput(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }
  const parsed = Number(trimmed);
  return Number.isInteger(parsed) && parsed >= 0;
}

export function requiredNonNegativeDecimal(value: string, label: string): string {
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

export function isNonNegativeDecimalInput(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }
  return /^\d+(\.\d+)?$/.test(trimmed) && Number.isFinite(Number(trimmed));
}

export function requiredString(value: string, label: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${label}不能为空`);
  }
  return trimmed;
}

export function optionalString(value: string): string | undefined {
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

export function includeCurrentOption<T extends { id: string; label: string }>(options: T[], id: string, label: string): T[] {
  const optionId = id.trim();
  if (!optionId || options.some((option) => option.id === optionId)) {
    return options;
  }
  return [{ id: optionId, label: label || `ID: ${optionId}` } as T, ...options];
}

export async function submitAction(label: string, request: () => Promise<unknown>) {
  try {
    await request();
    Toast.success(`${label}已提交`);
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

export function completeCreate(close: () => void, onCreated?: () => void, reset?: () => void) {
  close();
  reset?.();
  onCreated?.();
}

export function FormModal({ actionText, children, size = 'medium', title }: { actionText: string; children: FormModalChildren; size?: CreateModalSize; title: string }) {
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

export function useAssetOptions(enabled = true) {
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

export function useMarketPairOptions(enabled = true) {
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

export function useAdminCountryOptions(enabled = true) {
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

export function AssetSelect({
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

export function MarketPairSelect({
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

export function recordString(record: ApiRecord, key: string): string {
  const value = record[key];
  return typeof value === 'number' || typeof value === 'string' ? String(value) : '';
}

export async function openRecordDetail(endpoint: string, recordId: string, helpers: RowActionHelpers) {
  try {
    helpers.openDetail({ title: '详情', data: await apiRequest<ApiRecord>(`${endpoint}/${recordId}`) });
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

export function nextToggleStatus(status: string): 'active' | 'disabled' {
  return status === 'active' ? 'disabled' : 'active';
}

export const statusOptions: SemiSelectOption[] = [
  { value: 'active', label: '启用' },
  { value: 'disabled', label: '禁用' }
];

const booleanOptions: SemiSelectOption[] = [
  { value: 'true', label: '启用' },
  { value: 'false', label: '禁用' }
];

export function AssetStatusSelect({ onChange, value }: { onChange: (value: string) => void; value: string }) {
  return <AdminSelect ariaLabel="状态" onChange={onChange} optionList={statusOptions} value={value} />;
}

export function BooleanSelect({ label, onChange, optionList = booleanOptions, value }: { label: string; onChange: (value: string) => void; optionList?: SemiSelectOption[]; value: string }) {
  return <AdminSelect ariaLabel={label} onChange={onChange} optionList={optionList} value={value} />;
}

export function booleanFromSelect(value: string): boolean {
  return value !== 'false';
}

export function toggleActionText(nextStatus: string): string {
  return nextStatus === 'disabled' ? '禁用' : '启用';
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

export function earnCountrySelectOptions(countries: AdminNewsCountryOption[]): SemiSelectOption[] {
  return countries.map((country) => ({
    value: country.countryCode,
    label: `${country.countryName} (${country.countryCode} / ${country.defaultLocale})`
  }));
}

export function includeCurrentCountrySelectOption(options: SemiSelectOption[], countryCode: string, locale: string): SemiSelectOption[] {
  const value = countryCode.trim().toUpperCase();
  if (!value || options.some((option) => option.value === value)) {
    return options;
  }
  const label = locale.trim() ? `${value} (${locale.trim()})` : value;
  return [{ value, label }, ...options];
}
