import { Button, Card, SideSheet, Space, Typography } from '@douyinfe/semi-ui';
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

const { Text, Title } = Typography;

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

function newAdminNewsTranslation(): AdminNewsTranslationValues {
  return { locale: 'en-US', countryCode: 'US', title: '', summary: emptyRichTextValue, content: emptyRichTextValue };
}

function updateAdminNewsTranslation(values: AdminNewsValues, index: number, patch: Partial<AdminNewsTranslationValues>): AdminNewsValues {
  return {
    ...values,
    translations: values.translations.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item))
  };
}

function adminNewsCountrySelectOptions(countries: AdminNewsCountryOption[]): SemiSelectOption[] {
  return countries.map((country) => ({
    value: country.countryCode,
    label: `${country.countryName} (${country.countryCode})`
  }));
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
