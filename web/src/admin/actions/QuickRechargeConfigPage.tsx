import { IconExternalOpen, IconRefresh } from '@douyinfe/semi-icons';
import { Button, Card, Col, Divider, Row, Space, Switch, Toast, Typography } from '@douyinfe/semi-ui';
import { type ReactNode, useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminPasswordInput, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import { TimestampText } from '../../shared/TimestampText';

const { Text, Title } = Typography;

type QuickRechargeConfig = {
  id: number;
  enabled: boolean;
  api_base_url?: string | null;
  merchant_pid?: string | null;
  merchant_secret_mask?: string | null;
  merchant_secret_set: boolean;
  currency: string;
  token: string;
  network: string;
  notify_url?: string | null;
  redirect_url?: string | null;
  pc_app_redirect_url?: string | null;
  mac_app_redirect_url?: string | null;
  ios_app_redirect_url?: string | null;
  android_app_redirect_url?: string | null;
  mobile_web_redirect_url?: string | null;
  desktop_web_redirect_url?: string | null;
  min_amount: string;
  max_amount?: string | null;
  updated_by?: number | null;
  updated_at: number;
};

type QuickRechargeTestResponse = {
  order_id: string;
  provider_trade_id: string;
  currency: string;
  token: string;
  network: string;
  fiat_amount: string;
  actual_amount: string;
  receive_address: string;
  payment_url: string;
  expiration_time?: number | null;
  tested_at: number;
};

type QuickRechargeForm = {
  enabled: boolean;
  apiBaseUrl: string;
  merchantPid: string;
  merchantSecret: string;
  currency: string;
  token: string;
  network: string;
  notifyUrl: string;
  redirectUrl: string;
  pcAppRedirectUrl: string;
  macAppRedirectUrl: string;
  iosAppRedirectUrl: string;
  androidAppRedirectUrl: string;
  mobileWebRedirectUrl: string;
  desktopWebRedirectUrl: string;
  minAmount: string;
  maxAmount: string;
};

const defaultForm: QuickRechargeForm = {
  enabled: false,
  apiBaseUrl: '',
  merchantPid: '',
  merchantSecret: '',
  currency: 'cny',
  token: 'usdt',
  network: 'tron',
  notifyUrl: '',
  redirectUrl: '',
  pcAppRedirectUrl: '',
  macAppRedirectUrl: '',
  iosAppRedirectUrl: '',
  androidAppRedirectUrl: '',
  mobileWebRedirectUrl: '',
  desktopWebRedirectUrl: '',
  minAmount: '0.01',
  maxAmount: ''
};

const currencyOptions = [
  { label: '人民币 CNY', value: 'cny' },
  { label: '美元 USD', value: 'usd' },
  { label: '港币 HKD', value: 'hkd' }
];

const tokenOptions = [
  { label: 'USDT', value: 'usdt' },
  { label: 'USDC', value: 'usdc' },
  { label: 'TRX', value: 'trx' },
  { label: 'SOL', value: 'sol' },
  { label: 'BTC', value: 'btc' },
  { label: 'ETH', value: 'eth' }
];

const networkOptions = [
  { label: 'Tron 波场', value: 'tron' },
  { label: 'Ethereum 以太坊', value: 'ethereum' },
  { label: 'Base 网络', value: 'base' },
  { label: 'BSC 币安智能链', value: 'bsc' },
  { label: 'Polygon', value: 'polygon' },
  { label: 'Solana', value: 'solana' },
  { label: 'TON', value: 'ton' }
];

const returnUrlFields: Array<{
  formKey: keyof Pick<
    QuickRechargeForm,
    | 'pcAppRedirectUrl'
    | 'macAppRedirectUrl'
    | 'iosAppRedirectUrl'
    | 'androidAppRedirectUrl'
    | 'mobileWebRedirectUrl'
    | 'desktopWebRedirectUrl'
  >;
  label: string;
  placeholder: string;
}> = [
  {
    formKey: 'pcAppRedirectUrl',
    label: 'PC 应用端回跳地址',
    placeholder: 'rustchain://quick-recharge/return'
  },
  {
    formKey: 'macAppRedirectUrl',
    label: 'Mac 应用端回跳地址',
    placeholder: 'rustchain-mac://quick-recharge/return'
  },
  {
    formKey: 'iosAppRedirectUrl',
    label: 'iOS 端回跳地址',
    placeholder: 'rustchain-ios://quick-recharge/return'
  },
  {
    formKey: 'androidAppRedirectUrl',
    label: 'Android 端回跳地址',
    placeholder: 'rustchain-android://quick-recharge/return'
  },
  {
    formKey: 'mobileWebRedirectUrl',
    label: '手机网页端回跳地址',
    placeholder: 'https://m.example.com/user/recharge'
  },
  {
    formKey: 'desktopWebRedirectUrl',
    label: '电脑网页端回跳地址',
    placeholder: 'https://www.example.com/user/recharge'
  }
];

type FieldColumnSize = 'full' | 'half' | 'third' | 'wide';

const fieldColumnProps: Record<FieldColumnSize, { lg?: number; md?: number; xl?: number; xs: number }> = {
  full: { xs: 24 },
  half: { xs: 24, md: 12 },
  third: { xs: 24, md: 12, xl: 8 },
  wide: { xs: 24, lg: 16 }
};

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function formFromConfig(config: QuickRechargeConfig | null): QuickRechargeForm {
  if (!config) return defaultForm;
  return {
    enabled: config.enabled,
    apiBaseUrl: config.api_base_url ?? '',
    merchantPid: config.merchant_pid ?? '',
    merchantSecret: '',
    currency: config.currency || defaultForm.currency,
    token: config.token || defaultForm.token,
    network: config.network || defaultForm.network,
    notifyUrl: config.notify_url ?? '',
    redirectUrl: config.redirect_url ?? '',
    pcAppRedirectUrl: config.pc_app_redirect_url ?? '',
    macAppRedirectUrl: config.mac_app_redirect_url ?? '',
    iosAppRedirectUrl: config.ios_app_redirect_url ?? '',
    androidAppRedirectUrl: config.android_app_redirect_url ?? '',
    mobileWebRedirectUrl: config.mobile_web_redirect_url ?? '',
    desktopWebRedirectUrl: config.desktop_web_redirect_url ?? '',
    minAmount: config.min_amount || defaultForm.minAmount,
    maxAmount: config.max_amount ?? ''
  };
}

function payloadFromForm(form: QuickRechargeForm, reason: string) {
  return {
    enabled: form.enabled,
    api_base_url: form.apiBaseUrl.trim() || null,
    merchant_pid: form.merchantPid.trim() || null,
    merchant_secret: form.merchantSecret.trim() || null,
    currency: form.currency,
    token: form.token,
    network: form.network,
    notify_url: form.notifyUrl.trim() || null,
    redirect_url: form.redirectUrl.trim() || null,
    pc_app_redirect_url: form.pcAppRedirectUrl.trim() || null,
    mac_app_redirect_url: form.macAppRedirectUrl.trim() || null,
    ios_app_redirect_url: form.iosAppRedirectUrl.trim() || null,
    android_app_redirect_url: form.androidAppRedirectUrl.trim() || null,
    mobile_web_redirect_url: form.mobileWebRedirectUrl.trim() || null,
    desktop_web_redirect_url: form.desktopWebRedirectUrl.trim() || null,
    min_amount: form.minAmount.trim() || '0.01',
    max_amount: form.maxAmount.trim() || null,
    reason
  };
}

function missingEnableFields(form: QuickRechargeForm, config: QuickRechargeConfig | null) {
  const fields: string[] = [];
  if (!form.apiBaseUrl.trim()) fields.push('API 基础地址');
  if (!form.merchantPid.trim()) fields.push('商户 PID');
  if (!form.merchantSecret.trim() && !config?.merchant_secret_set) fields.push('商户 Secret Key');
  if (!form.notifyUrl.trim()) fields.push('异步回调地址');
  return fields;
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

function ConfigGrid({ children }: { children: ReactNode }) {
  return (
    <Row gutter={[24, 20]} style={{ width: '100%' }}>
      {children}
    </Row>
  );
}

function ConfigSection({ children, title }: { children: ReactNode; title: string }) {
  return (
    <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
      <Title heading={6} style={{ margin: 0 }}>{title}</Title>
      <ConfigGrid>{children}</ConfigGrid>
    </Space>
  );
}

function FieldColumn({ children, size = 'half' }: { children: ReactNode; size?: FieldColumnSize }) {
  return <Col {...fieldColumnProps[size]}>{children}</Col>;
}

function FieldLabel({ children, label }: { children: ReactNode; label: string }) {
  return (
    <label style={{ display: 'grid', gap: 6, width: '100%' }}>
      {label}
      {children}
    </label>
  );
}

function ResultField({ label, value }: { label: string; value: ReactNode }) {
  return (
    <Space align="start" spacing={2} vertical style={{ minWidth: 180 }}>
      <Text type="secondary">{label}</Text>
      <Text>{value}</Text>
    </Space>
  );
}

export function QuickRechargeConfigPage() {
  const [config, setConfig] = useState<QuickRechargeConfig | null>(null);
  const [form, setForm] = useState<QuickRechargeForm>(defaultForm);
  const [loading, setLoading] = useState(true);
  const [testAmount, setTestAmount] = useState(defaultForm.minAmount);
  const [testResult, setTestResult] = useState<QuickRechargeTestResponse | null>(null);

  async function loadConfig() {
    setLoading(true);
    try {
      const response = await apiRequest<QuickRechargeConfig>('/admin/api/v1/quick-recharge/config');
      setConfig(response);
      setForm(formFromConfig(response));
      setTestAmount(response.min_amount || defaultForm.minAmount);
      setTestResult(null);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadConfig().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  const missingFields = form.enabled ? missingEnableFields(form, config) : [];
  const canSave = !loading;
  const canTest = Boolean(config?.api_base_url?.trim() && config?.merchant_pid?.trim() && config?.merchant_secret_set && config?.notify_url?.trim() && testAmount.trim());
  const enabledChanged = config ? form.enabled !== config.enabled : false;
  const enabledStatusText = enabledChanged ? (form.enabled ? '将启用，保存后生效' : '将停用，保存后生效') : form.enabled ? '当前已启用' : '当前未启用';
  const saveActionText = enabledChanged ? (form.enabled ? '保存并启用GMPay' : '保存并停用GMPay') : '保存快速充值配置';

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={() => loadConfig().catch((error) => Toast.error(errorMessage(error)))} theme="borderless">
            刷新
          </Button>
        }
        title="快速充值配置"
      />
      <Card bordered={false} shadows="always">
        <Space align="start" spacing={24} vertical style={{ width: '100%' }}>
          <Row gutter={[24, 16]} style={{ width: '100%' }} type="flex" align="middle" justify="space-between">
            <Col xs={24} lg={12}>
              <Space align="center" spacing={12}>
                <Switch aria-label="GMPay快速充值开关" checked={form.enabled} onChange={(enabled) => setForm({ ...form, enabled })} />
                <Title heading={5} style={{ margin: 0 }}>GMPay 快速充值</Title>
              </Space>
              <Text type={enabledChanged ? 'warning' : 'secondary'}>{enabledStatusText}</Text>
              {missingFields.length > 0 ? <Text type="danger">启用前需完善：{missingFields.join('、')}</Text> : null}
            </Col>
            <Col xs={24} lg={12}>
              <Space spacing={16} wrap style={{ width: '100%', justifyContent: 'flex-end' }}>
                <Text type="secondary">配置 ID：{config?.id ?? '-'}</Text>
                <Text type="secondary">最后更新：<TimestampText value={config?.updated_at ?? null} /></Text>
                <Text type="secondary">更新管理员：{config?.updated_by ?? '-'}</Text>
              </Space>
            </Col>
          </Row>

          <Space align="start" spacing={24} vertical style={{ width: '100%' }}>
            <section style={{ width: '100%' }}>
              <Title heading={5}>商户接口</Title>
              <ConfigGrid>
                <FieldColumn size="wide">
                  <FieldLabel label="API 基础地址">
                  <AdminTextInput ariaLabel="API 基础地址" onChange={(apiBaseUrl) => setForm({ ...form, apiBaseUrl })} placeholder="https://pay.example.com" value={form.apiBaseUrl} />
                  </FieldLabel>
                </FieldColumn>
                <FieldColumn size="half">
                  <FieldLabel label="商户 PID">
                  <AdminTextInput ariaLabel="商户 PID" onChange={(merchantPid) => setForm({ ...form, merchantPid })} value={form.merchantPid} />
                  </FieldLabel>
                </FieldColumn>
                <FieldColumn size="half">
                  <FieldLabel label="商户 Secret Key">
                  <AdminPasswordInput ariaLabel="商户 Secret Key" onChange={(merchantSecret) => setForm({ ...form, merchantSecret })} placeholder={config?.merchant_secret_set ? '留空则保持当前密钥' : ''} value={form.merchantSecret} />
                  {config?.merchant_secret_mask ? <Text type="secondary">当前密钥：{config.merchant_secret_mask}</Text> : null}
                  </FieldLabel>
                </FieldColumn>
              </ConfigGrid>
            </section>

            <section style={{ width: '100%' }}>
              <Title heading={5}>充值限制</Title>
              <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
                <ConfigSection title="入账范围">
                  <FieldColumn size="third">
                    <FieldLabel label="法币币种">
                      <AdminSelect ariaLabel="法币币种" onChange={(currency) => setForm({ ...form, currency })} optionList={currencyOptions} value={form.currency} />
                    </FieldLabel>
                  </FieldColumn>
                  <FieldColumn size="third">
                    <FieldLabel label="到账资产">
                      <AdminSelect ariaLabel="到账资产" onChange={(token) => setForm({ ...form, token })} optionList={tokenOptions} value={form.token} />
                    </FieldLabel>
                  </FieldColumn>
                  <FieldColumn size="third">
                    <FieldLabel label="收款网络">
                      <AdminSelect ariaLabel="收款网络" onChange={(network) => setForm({ ...form, network })} optionList={networkOptions} value={form.network} />
                    </FieldLabel>
                  </FieldColumn>
                </ConfigSection>

                <ConfigSection title="单笔金额限制">
                  <FieldColumn size="half">
                    <FieldLabel label="单笔最小金额">
                      <AdminTextInput ariaLabel="单笔最小金额" onChange={(minAmount) => setForm({ ...form, minAmount })} type="number" value={form.minAmount} />
                    </FieldLabel>
                  </FieldColumn>
                  <FieldColumn size="half">
                    <FieldLabel label="单笔最大金额">
                      <AdminTextInput ariaLabel="单笔最大金额" onChange={(maxAmount) => setForm({ ...form, maxAmount })} placeholder="留空表示不限制" type="number" value={form.maxAmount} />
                    </FieldLabel>
                  </FieldColumn>
                </ConfigSection>
              </Space>
            </section>

            <section style={{ width: '100%' }}>
              <Title heading={5}>回调跳转</Title>
              <ConfigGrid>
                <FieldColumn size="half">
                  <FieldLabel label="异步回调地址">
                  <AdminTextInput ariaLabel="异步回调地址" onChange={(notifyUrl) => setForm({ ...form, notifyUrl })} placeholder="https://api.example.com/api/v1/payments/gmpay/notify" value={form.notifyUrl} />
                  </FieldLabel>
                </FieldColumn>
                <FieldColumn size="half">
                  <FieldLabel label="默认支付完成跳转地址">
                  <AdminTextInput ariaLabel="默认支付完成跳转地址" onChange={(redirectUrl) => setForm({ ...form, redirectUrl })} placeholder="https://www.example.com/user/recharge" value={form.redirectUrl} />
                  </FieldLabel>
                </FieldColumn>
                {returnUrlFields.map((field) => (
                  <FieldColumn key={field.formKey} size="half">
                    <FieldLabel label={field.label}>
                    <AdminTextInput
                      ariaLabel={field.label}
                      onChange={(value) => setForm((current) => ({ ...current, [field.formKey]: value }))}
                      placeholder={field.placeholder}
                      value={form[field.formKey]}
                    />
                    </FieldLabel>
                  </FieldColumn>
                ))}
              </ConfigGrid>
            </section>

            <section style={{ width: '100%' }}>
              <Title heading={5}>联通测试</Title>
              <ConfigGrid>
                <FieldColumn size="half">
                  <FieldLabel label="测试金额">
                  <AdminTextInput ariaLabel="测试金额" onChange={setTestAmount} type="number" value={testAmount} />
                  </FieldLabel>
                </FieldColumn>
                <FieldColumn size="half">
                  <Space align="center" style={{ height: '100%' }}>
                    <ConfirmAction
                      actionText="测试快速充值"
                      disabled={!canTest}
                      title="确认测试快速充值配置"
                      onConfirm={(reason) =>
                        submitAction('快速充值联通测试', async () => {
                          const response = await apiRequest<QuickRechargeTestResponse>('/admin/api/v1/quick-recharge/config/test', {
                            method: 'POST',
                            body: JSON.stringify({
                              amount: testAmount.trim(),
                              reason
                            })
                          });
                          setTestResult(response);
                        })
                      }
                    />
                  </Space>
                </FieldColumn>
                {testResult ? (
                  <FieldColumn size="full">
                    <Row data-testid="quick-recharge-test-result" gutter={[24, 16]} style={{ width: '100%' }}>
                      <Col xs={24} md={12} xl={8}>
                        <ResultField label="测试订单" value={testResult.order_id} />
                      </Col>
                      <Col xs={24} md={12} xl={8}>
                        <ResultField label="服务商交易号" value={testResult.provider_trade_id} />
                      </Col>
                      <Col xs={24} md={12} xl={8}>
                        <ResultField label="充值金额" value={`${testResult.fiat_amount} ${testResult.currency.toUpperCase()}`} />
                      </Col>
                      <Col xs={24} md={12} xl={8}>
                        <ResultField label="实际支付数量" value={`${testResult.actual_amount} ${testResult.token.toUpperCase()}`} />
                      </Col>
                      <Col xs={24} md={12} xl={8}>
                        <ResultField label="收款网络" value={testResult.network} />
                      </Col>
                      <Col xs={24} md={12} xl={8}>
                        <ResultField label="过期时间" value={testResult.expiration_time ?? '-'} />
                      </Col>
                      <Col xs={24} md={16}>
                        <ResultField label="收款地址" value={testResult.receive_address} />
                      </Col>
                      <Col xs={24} md={8}>
                        <Button icon={<IconExternalOpen aria-hidden="true" />} onClick={() => window.open(testResult.payment_url, '_blank', 'noopener,noreferrer')} theme="borderless" type="primary">
                          打开收银台
                        </Button>
                      </Col>
                    </Row>
                  </FieldColumn>
                ) : null}
              </ConfigGrid>
            </section>
          </Space>

          <Divider margin="0" />
          <Row style={{ width: '100%' }} type="flex" justify="end">
            <ConfirmAction
              actionText={saveActionText}
              disabled={!canSave}
              title="确认保存快速充值配置"
              onConfirm={(reason) =>
                submitAction('保存快速充值配置', async () => {
                  const missing = form.enabled ? missingEnableFields(form, config) : [];
                  if (missing.length > 0) {
                    throw new Error(`启用前需完善：${missing.join('、')}`);
                  }
                  const saved = await apiRequest<QuickRechargeConfig>('/admin/api/v1/quick-recharge/config', {
                    method: 'PATCH',
                    body: JSON.stringify(payloadFromForm(form, reason))
                  });
                  setConfig(saved);
                  setForm(formFromConfig(saved));
                  setTestAmount(saved.min_amount || defaultForm.minAmount);
                  setTestResult(null);
                })
              }
            />
          </Row>
        </Space>
      </Card>
    </main>
  );
}
