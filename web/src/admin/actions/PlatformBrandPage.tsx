import { IconRefresh } from '@douyinfe/semi-icons';
import { Button, Card, Image, Space, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminImageUpload } from '../../shared/AdminImageUpload';
import { AdminSelect, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';
import { TimestampText } from '../../shared/TimestampText';

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

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function formFromBrand(brand: PlatformBrand | null): BrandForm {
  return {
    chartProvider: brand?.chart_provider ?? defaultBrandForm.chartProvider,
    logoUrl: brand?.logo_url ?? '',
    platformName: brand?.platform_name ?? defaultBrandForm.platformName
  };
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

export function PlatformBrandPage() {
  const [brand, setBrand] = useState<PlatformBrand | null>(null);
  const [form, setForm] = useState<BrandForm>(defaultBrandForm);
  const [loading, setLoading] = useState(true);

  async function loadBrand() {
    setLoading(true);
    try {
      const response = await apiRequest<PlatformBrand>('/admin/api/v1/platform/brand');
      setBrand(response);
      setForm(formFromBrand(response));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadBrand().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  const canSave = form.platformName.trim().length > 0;
  const previewLogo = form.logoUrl.trim();
  const previewName = form.platformName.trim() || defaultBrandForm.platformName;
  const previewChartProvider = chartProviderOptions.find((option) => option.value === form.chartProvider)?.label ?? form.chartProvider;

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={() => loadBrand().catch((error) => Toast.error(errorMessage(error)))} theme="borderless">
            刷新
          </Button>
        }
        title="PC 品牌配置"
      />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>显示配置</Title>
            <div className="admin-action-form admin-action-form-narrow">
              <label>
                平台名称
                <AdminTextInput ariaLabel="平台名称" onChange={(platformName) => setForm({ ...form, platformName })} value={form.platformName} />
              </label>
              <label>
                K线图引擎
                <AdminSelect ariaLabel="K线图引擎" onChange={(chartProvider) => setForm({ ...form, chartProvider })} optionList={chartProviderOptions} value={form.chartProvider} />
              </label>
              <AdminImageUpload label="PC Logo" value={form.logoUrl} variant="avatar" onChange={(logoUrl) => setForm({ ...form, logoUrl })} />
            </div>
            <ConfirmAction
              actionText="保存品牌配置"
              disabled={!canSave}
              title="确认保存 PC 品牌配置"
              onConfirm={(reason) =>
                submitAction('保存 PC 品牌配置', async () => {
                  const saved = await apiRequest<PlatformBrand>('/admin/api/v1/platform/brand', {
                    method: 'PATCH',
                    body: JSON.stringify({
                      chart_provider: form.chartProvider,
                      logo_url: form.logoUrl.trim() || null,
                      platform_name: form.platformName.trim(),
                      reason
                    })
                  });
                  setBrand(saved);
                  setForm(formFromBrand(saved));
                })
              }
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
    </main>
  );
}
