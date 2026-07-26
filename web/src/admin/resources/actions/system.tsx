import { Button, Card, SideSheet, Space } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminModalTriggerButton, AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  BooleanSelect,
  type RowActionHelpers,
  booleanFromSelect,
  createModalProps,
  isNonNegativeIntegerInput,
  nextToggleStatus,
  openRecordDetail,
  recordString,
  requiredNonNegativeInteger,
  requiredString,
  submitAction
} from './shared';

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

const localeOptions: SemiSelectOption[] = [
  { value: 'zh', label: '中文' },
  { value: 'en', label: '英文' }
];

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

const countryBooleanOptions: SemiSelectOption[] = [
  { value: 'true', label: '启用' },
  { value: 'false', label: '停用' }
];

const countryStatusOptions: SemiSelectOption[] = [
  { value: 'active', label: '启用' },
  { value: 'disabled', label: '停用' }
];

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
