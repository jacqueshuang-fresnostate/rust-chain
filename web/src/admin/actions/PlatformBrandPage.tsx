import { Card, Image, Space, Typography } from '@douyinfe/semi-ui';
import { useMemo } from 'react';

import { apiRequest } from '../../api/client';
import { AdminImageUpload } from '../../shared/AdminImageUpload';
import { AdminSelect, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';
import { TimestampText } from '../../shared/TimestampText';
import {
  AdminSettingsPage,
  buildSettingsDifferences,
  buildSettingsImpactSummary,
  SettingsSaveConfirmation,
  type SettingsFieldDefinition,
  settingsValuesEqual,
  useAdminSettingsEditor,
  validateSettingsFields
} from '../settings';

const { Text, Title } = Typography;

type PlatformBrand = {
  created_at: number;
  id: number;
  logo_url?: string | null;
  name: string;
  platform_name: string;
  chart_provider: string;
  updated_at: number;
  updated_by?: number | null;
};

type BrandForm = {
  chartProvider: string;
  logoUrl: string;
  platformName: string;
};

const defaultBrandForm: BrandForm = {
  chartProvider: 'klinecharts',
  logoUrl: '',
  platformName: 'Hippo Exchange'
};

const chartProviderOptions: SemiSelectOption[] = [
  { value: 'klinecharts', label: '系统 K 线' },
  { value: 'tradingview', label: 'TradingView Lightweight Charts' }
];

const platformBrandApiPath = '/admin/api/v1/platform/brand';
const brandImpactSummary = '保存后将立即影响 PC 端平台名称、Logo 与 K 线图展示。';

function formFromBrand(brand: PlatformBrand | null): BrandForm {
  return {
    chartProvider: brand?.chart_provider ?? defaultBrandForm.chartProvider,
    logoUrl: brand?.logo_url ?? '',
    platformName: brand?.platform_name ?? defaultBrandForm.platformName
  };
}

const brandFieldDefinitions: ReadonlyArray<SettingsFieldDefinition<BrandForm>> = [
  {
    key: 'platformName',
    field: '平台名称',
    impact: brandImpactSummary,
    read: (form) => form.platformName.trim(),
    validate: (value) => String(value).trim() ? null : '平台名称不能为空。'
  },
  {
    key: 'chartProvider',
    field: 'K线图引擎',
    impact: brandImpactSummary,
    read: (form) => form.chartProvider,
    format: (value) => chartProviderOptions.find((option) => option.value === value)?.label ?? String(value),
    validate: (value) => chartProviderOptions.some((option) => option.value === value)
      ? null
      : '请选择受支持的 K 线图引擎。'
  },
  { key: 'logoUrl', field: 'PC Logo', impact: brandImpactSummary, read: (form) => form.logoUrl.trim() }
];

function normalizedBrandForm(form: BrandForm): BrandForm {
  return {
    ...form,
    logoUrl: form.logoUrl.trim(),
    platformName: form.platformName.trim()
  };
}

export function PlatformBrandPage() {
  const editor = useAdminSettingsEditor<PlatformBrand, BrandForm>({
    areEqual: (left, right) => settingsValuesEqual(normalizedBrandForm(left), normalizedBrandForm(right)),
    settingKey: platformBrandApiPath,
    initialForm: defaultBrandForm,
    load: () => apiRequest<PlatformBrand>(platformBrandApiPath),
    selectForm: formFromBrand,
    save: (form, reason) =>
      apiRequest<PlatformBrand>(platformBrandApiPath, {
        method: 'PATCH',
        body: JSON.stringify({
          chart_provider: form.chartProvider,
          logo_url: form.logoUrl.trim() || null,
          platform_name: form.platformName.trim(),
          reason
        })
      }),
    successMessage: 'PC 品牌配置已保存。'
  });
  const { draft: form } = editor;
  const brand = editor.data ?? null;
  const differences = useMemo(
    () => buildSettingsDifferences(editor.baseline ?? form, form, brandFieldDefinitions),
    [editor.baseline, form]
  );
  const validationIssues = useMemo(
    () => validateSettingsFields(form, brandFieldDefinitions),
    [form]
  );
  const impactSummary = useMemo(
    () => buildSettingsImpactSummary(differences, brandFieldDefinitions, brandImpactSummary),
    [differences]
  );

  const previewLogo = form.logoUrl.trim();
  const previewName = form.platformName.trim() || defaultBrandForm.platformName;
  const previewChartProvider = chartProviderOptions.find((option) => option.value === form.chartProvider)?.label ?? form.chartProvider;

  return (
    <AdminSettingsPage
      feedback={editor.feedback}
      isDirty={editor.isDirty}
      isInitialLoading={editor.isInitialLoading}
      isReady={editor.isReady}
      isRefreshing={editor.isFetching}
      loadError={editor.loadError}
      onReload={editor.reloadLatest}
      title="PC 品牌配置"
    >
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>显示配置</Title>
            <div className="admin-action-form admin-action-form-narrow">
              <label>
                平台名称
                <AdminTextInput
                  ariaLabel="平台名称"
                  onChange={(platformName) => editor.setDraft((current) => ({ ...current, platformName }))}
                  value={form.platformName}
                />
              </label>
              <label>
                K线图引擎
                <AdminSelect
                  ariaLabel="K线图引擎"
                  onChange={(chartProvider) => editor.setDraft((current) => ({ ...current, chartProvider }))}
                  optionList={chartProviderOptions}
                  value={form.chartProvider}
                />
              </label>
              <AdminImageUpload
                label="PC Logo"
                value={form.logoUrl}
                variant="avatar"
                onChange={(logoUrl) => editor.setDraft((current) => ({ ...current, logoUrl }))}
              />
            </div>
            <SettingsSaveConfirmation
              actionText="保存品牌配置"
              differences={differences}
              disabled={editor.isSaving}
              impactSummary={impactSummary}
              title="确认保存 PC 品牌配置"
              onConfirm={editor.saveChanges}
              validationIssues={validationIssues}
            />
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>PC 端预览</Title>
            <Space align="center" spacing={12}>
              {previewLogo ? (
                <Image alt={previewName} height={48} imgStyle={{ objectFit: 'contain' }} preview src={previewLogo} width={132} />
              ) : (
                <div aria-label="默认 Logo 占位" style={{ alignItems: 'center', display: 'grid', height: 48, justifyItems: 'center', width: 132 }}>
                  <Text type="tertiary">Logo</Text>
                </div>
              )}
              <Title heading={5} style={{ margin: 0 }}>{previewName}</Title>
            </Space>
            <div className="admin-action-summary">
              <span>配置 ID：{brand?.id ?? '-'}</span>
              <span>K线图引擎：{previewChartProvider}</span>
              <span>最后更新：<TimestampText value={brand?.updated_at ?? null} /></span>
              <span>更新管理员：{brand?.updated_by ?? '-'}</span>
            </div>
          </Space>
        </Card>
      </div>
    </AdminSettingsPage>
  );
}
