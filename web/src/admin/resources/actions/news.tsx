import { Button, SideSheet, Space, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { QuillRichTextEditor, type RichTextValue } from '../../../shared/QuillRichTextEditor';
import { AdminModalTriggerButton, AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  type AdminNewsCountryOption,
  type RowActionHelpers,
  createModalProps,
  emptyRichTextValue,
  openRecordDetail,
  optionalString,
  recordString,
  requiredString,
  submitAction,
  useAdminCountryOptions
} from './shared';

const { Text } = Typography;

type AdminNewsTranslationValues = {
  content: RichTextValue;
  countryCode: string;
  locale: string;
  summary: RichTextValue;
  title: string;
};

type AdminNewsValues = {
  additionalContentItems: unknown[];
  bannerUrl: string;
  category: string;
  countryCode: string;
  defaultLocale: string;
  smallLogoUrl: string;
  status: string;
  title: string;
  translations: AdminNewsTranslationValues[];
};

const initialAdminNews: AdminNewsValues = {
  additionalContentItems: [],
  bannerUrl: '',
  title: '',
  category: 'general',
  countryCode: '',
  defaultLocale: 'zh-CN',
  smallLogoUrl: '',
  status: 'draft',
  translations: [{ locale: 'zh-CN', countryCode: 'CN', title: '', summary: emptyRichTextValue, content: emptyRichTextValue }]
};

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
  const syncedValues = syncAdminNewsContent(values);
  return Boolean(
    syncedValues.title.trim() &&
      syncedValues.category.trim() &&
      syncedValues.countryCode.trim() &&
      syncedValues.defaultLocale.trim() &&
      syncedValues.translations.length > 0 &&
      syncedValues.translations.every((item) => item.locale.trim() && item.countryCode.trim() && item.title.trim() && richTextHasContent(item.content))
  );
}

function newAdminNewsTranslation(): AdminNewsTranslationValues {
  return { locale: 'en-US', countryCode: 'US', title: '', summary: emptyRichTextValue, content: emptyRichTextValue };
}

function adminNewsCountrySelectOptions(countries: AdminNewsCountryOption[]): SemiSelectOption[] {
  return countries.map((country) => ({
    value: country.countryCode,
    label: `${country.countryName} (${country.countryCode})`
  }));
}

function syncAdminNewsContent(values: AdminNewsValues): AdminNewsValues {
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
  return syncAdminNewsContent({
    ...values,
    countryCode: country.countryCode,
    defaultLocale: country.defaultLocale
  });
}

function adminNewsContentJson(values: AdminNewsValues) {
  return {
    version: 1,
    default_locale: requiredString(values.defaultLocale, '默认语言'),
    items: [
      ...values.translations.map((item) => ({
        locale: requiredString(item.locale, '语言'),
        country_code: requiredString(item.countryCode, '翻译国家'),
        title: requiredString(item.title, '翻译标题'),
        summary: optionalRichTextValue(item.summary),
        content: item.content
      })),
      ...values.additionalContentItems
    ]
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

function adminNewsTranslationFromRecord(value: unknown): AdminNewsTranslationValues | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined;
  const item = value as Record<string, unknown>;
  const translation = {
    locale: typeof item.locale === 'string' ? item.locale : '',
    countryCode: typeof item.country_code === 'string' ? item.country_code : '',
    title: typeof item.title === 'string' ? item.title : '',
    summary: adminNewsSummaryFromRecord(item.summary),
    content: Array.isArray(item.content) ? (item.content as RichTextValue) : emptyRichTextValue
  };
  return translation.locale || translation.countryCode || translation.title ? translation : undefined;
}

function adminNewsCreateRequestBody(values: AdminNewsValues, reason: string) {
  const createValues = syncAdminNewsContent(values);
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
  const updateValues = syncAdminNewsContent(values);
  return {
    title: requiredString(updateValues.title, '新闻标题'),
    banner_url: optionalString(updateValues.bannerUrl),
    small_logo_url: optionalString(updateValues.smallLogoUrl),
    category: requiredString(updateValues.category, '分类'),
    country_code: optionalString(updateValues.countryCode),
    default_locale: requiredString(updateValues.defaultLocale, '默认语言'),
    content_json: adminNewsContentJson(updateValues),
    reason
  };
}

function adminNewsFromRecord(record: ApiRecord): AdminNewsValues {
  const contentJson = record.content_json as { default_locale?: unknown; items?: unknown } | undefined;
  const items = Array.isArray(contentJson?.items) ? contentJson.items : [];
  const primaryTranslation = adminNewsTranslationFromRecord(items[0]);

  return syncAdminNewsContent({
    additionalContentItems: primaryTranslation ? items.slice(1) : items,
    bannerUrl: recordString(record, 'banner_url'),
    title: recordString(record, 'title') || primaryTranslation?.title || '',
    category: recordString(record, 'category') || 'general',
    countryCode: recordString(record, 'country_code') || primaryTranslation?.countryCode || '',
    defaultLocale:
      recordString(record, 'default_locale') ||
      (typeof contentJson?.default_locale === 'string' ? contentJson.default_locale : '') ||
      primaryTranslation?.locale ||
      'zh-CN',
    smallLogoUrl: recordString(record, 'small_logo_url'),
    status: recordString(record, 'status') || 'draft',
    translations: primaryTranslation ? [primaryTranslation] : initialAdminNews.translations
  });
}

function AdminNewsForm({
  countries,
  countriesLoading,
  idPrefix,
  includeStatus,
  onChange,
  values
}: {
  countries: AdminNewsCountryOption[];
  countriesLoading: boolean;
  idPrefix: string;
  includeStatus: boolean;
  onChange: (values: AdminNewsValues) => void;
  values: AdminNewsValues;
}) {
  const translation = values.translations[0] ?? newAdminNewsTranslation();
  const updatePrimaryContent = (patch: Partial<AdminNewsTranslationValues>) => {
    onChange(syncAdminNewsContent({ ...values, translations: [{ ...translation, ...patch }] }));
  };
  const selectCountry = (countryCode: string) => {
    const country = countries.find((item) => item.countryCode === countryCode);
    if (!country) {
      onChange(syncAdminNewsContent({ ...values, countryCode, defaultLocale: '' }));
      return;
    }
    onChange(applyAdminNewsCountry(values, country));
  };

  return (
    <div className="admin-news-create-layout">
      <div className="admin-news-create-side">
        <section className="admin-earn-product-section" aria-labelledby={`${idPrefix}-publish-title`}>
          <Text strong id={`${idPrefix}-publish-title`}>
            发布设置
          </Text>
          <div className="admin-action-form admin-news-create-settings-grid">
            <label className="admin-news-create-title-field">新闻标题<AdminTextInput ariaLabel="新闻标题" value={values.title} onChange={(title) => onChange(syncAdminNewsContent({ ...values, title }))} /></label>
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
              <AdminSelect ariaLabel="分类" onChange={(category) => onChange(syncAdminNewsContent({ ...values, category }))} optionList={newsCategoryOptions} value={values.category} />
            </label>
            {includeStatus ? (
              <label>
                初始状态
                <AdminSelect ariaLabel="初始状态" onChange={(status) => onChange(syncAdminNewsContent({ ...values, status }))} optionList={newsStatusOptions} value={values.status} />
              </label>
            ) : null}
          </div>
        </section>
        <section className="admin-earn-product-section" aria-labelledby={`${idPrefix}-media-title`}>
          <Text strong id={`${idPrefix}-media-title`}>
            视觉素材
          </Text>
          <div className="admin-news-create-media-grid">
            <AdminImageUpload label="新闻 Banner" value={values.bannerUrl} variant="banner" onChange={(bannerUrl) => onChange(syncAdminNewsContent({ ...values, bannerUrl }))} />
            <AdminImageUpload label="新闻小 Logo" value={values.smallLogoUrl} variant="avatar" onChange={(smallLogoUrl) => onChange(syncAdminNewsContent({ ...values, smallLogoUrl }))} />
          </div>
        </section>
      </div>
      <section className="admin-earn-product-section admin-news-create-content-panel" aria-labelledby={`${idPrefix}-content-title`}>
        <Text strong id={`${idPrefix}-content-title`}>
          内容编辑
        </Text>
        <Space align="start" spacing={14} vertical style={{ width: '100%' }}>
          <div className="admin-news-create-summary-field admin-news-summary-field">
            <Text strong>摘要</Text>
            <QuillRichTextEditor ariaLabel="摘要" placeholder="请输入新闻摘要" value={translation.summary} onChange={(summary) => updatePrimaryContent({ summary })} />
          </div>
          <div className="admin-news-create-editor">
            <QuillRichTextEditor enableImageUpload placeholder="请输入新闻内容" value={translation.content} onChange={(content) => updatePrimaryContent({ content })} />
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
            <AdminNewsForm countries={countries} countriesLoading={countriesLoading} idPrefix="admin-news-create" includeStatus values={news} onChange={setNews} />
            <ConfirmAction
              actionText="提交添加新闻"
              disabled={!isAdminNewsSubmittable(news)}
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
  const { countries, countriesLoading } = useAdminCountryOptions(visible);

  return (
    <>
      <Button disabled={!newsId} onClick={() => setVisible(true)} size="small" theme="borderless">
        编辑
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="编辑新闻" visible={visible} {...createModalProps('extra-wide')}>
        <div className="admin-news-create-shell">
          <Space align="end" spacing={16} vertical style={{ width: '100%' }}>
            <AdminNewsForm
              countries={countries}
              countriesLoading={countriesLoading}
              idPrefix="admin-news-edit"
              includeStatus={false}
              values={news}
              onChange={setNews}
            />
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
        </div>
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
