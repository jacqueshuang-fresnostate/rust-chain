import { IconLock, IconRefresh, IconSetting, IconShield } from '@douyinfe/semi-icons';
import { Button, Card, Space, Switch, Tabs, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminCheckbox, AdminSelect, type SemiSelectOption } from '../../shared/SemiFormControls';

const { Text, Title } = Typography;

type LoginTwoFactorMode = 'none' | 'user_enabled' | 'mandatory';
type SecurityVerificationMethod = 'fund_password' | 'two_factor' | 'fund_password_and_two_factor';
type SecurityActionKey = 'withdraw' | 'spot_order' | 'convert' | 'earn_subscribe';
type ThirdPartyBindingKey = 'coinbase_wallet_enabled' | 'telegram_account_enabled';
type SecurityTab = 'login' | 'payment' | 'third-party' | 'summary';

type PaymentPolicy = {
  enabled: boolean;
  method: SecurityVerificationMethod;
};

type PaymentPolicies = Record<SecurityActionKey, PaymentPolicy>;

type ThirdPartyBindingPolicy = Record<ThirdPartyBindingKey, boolean>;

type UserSecurityPolicy = {
  login_2fa_mode: LoginTwoFactorMode;
  registration_invite_required: boolean;
  username_login_enabled: boolean;
  payment_policies: PaymentPolicies;
  third_party_bindings: ThirdPartyBindingPolicy;
};

const loginModeOptions: SemiSelectOption[] = [
  { value: 'none', label: '不要求' },
  { value: 'user_enabled', label: '用户自选' },
  { value: 'mandatory', label: '强制要求' }
];

const methodOptions: SemiSelectOption[] = [
  { value: 'fund_password', label: '资金密码' },
  { value: 'two_factor', label: '双因素认证' },
  { value: 'fund_password_and_two_factor', label: '资金密码 + 双因素认证' }
];

const actionConfigs: Array<{ key: SecurityActionKey; label: string }> = [
  { key: 'withdraw', label: '提现' },
  { key: 'spot_order', label: '现货下单' },
  { key: 'convert', label: '闪兑' },
  { key: 'earn_subscribe', label: '理财申购' }
];

const thirdPartyBindingConfigs: Array<{ key: ThirdPartyBindingKey; label: string; description: string }> = [
  { key: 'coinbase_wallet_enabled', label: 'Coinbase 钱包', description: '允许用户在 PC 安全中心绑定 Coinbase 钱包标识' },
  { key: 'telegram_account_enabled', label: 'TG 账号', description: '允许用户在 PC 安全中心绑定 Telegram 账号标识' }
];

const securityTabs = [
  { itemKey: 'login', tab: '登录策略', icon: <IconLock aria-hidden="true" /> },
  { itemKey: 'payment', tab: '资金动作校验', icon: <IconShield aria-hidden="true" /> },
  { itemKey: 'third-party', tab: '第三方绑定', icon: <IconSetting aria-hidden="true" /> },
  { itemKey: 'summary', tab: '策略摘要', icon: <IconSetting aria-hidden="true" /> }
];

const defaultPolicy: UserSecurityPolicy = {
  login_2fa_mode: 'user_enabled',
  registration_invite_required: false,
  username_login_enabled: false,
  payment_policies: {
    withdraw: { enabled: true, method: 'fund_password' },
    spot_order: { enabled: false, method: 'fund_password' },
    convert: { enabled: false, method: 'fund_password' },
    earn_subscribe: { enabled: false, method: 'fund_password' }
  },
  third_party_bindings: {
    coinbase_wallet_enabled: false,
    telegram_account_enabled: false
  }
};

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function optionLabel(options: SemiSelectOption[], value: string) {
  return options.find((option) => option.value === value)?.label ?? value;
}

function paymentLabel(policy: PaymentPolicy) {
  return policy.enabled ? optionLabel(methodOptions, policy.method) : '未启用';
}

function normalizePolicy(value: UserSecurityPolicy): UserSecurityPolicy {
  return {
    ...defaultPolicy,
    ...value,
    payment_policies: {
      ...defaultPolicy.payment_policies,
      ...value.payment_policies
    },
    third_party_bindings: {
      ...defaultPolicy.third_party_bindings,
      ...value.third_party_bindings
    }
  };
}

export function SecurityPolicyPage() {
  const [activeTab, setActiveTab] = useState<SecurityTab>('login');
  const [policy, setPolicy] = useState<UserSecurityPolicy>(defaultPolicy);
  const [loading, setLoading] = useState(true);

  const paymentSummary = useMemo(
    () => actionConfigs.map(({ key, label }) => `${label}：${paymentLabel(policy.payment_policies[key])}`).join('，'),
    [policy]
  );
  const registrationSummary = policy.registration_invite_required ? '邀请码必填' : '邀请码选填';
  const usernameLoginSummary = policy.username_login_enabled ? '用户名登录已开启' : '用户名登录未开启';
  const thirdPartySummary = useMemo(
    () => thirdPartyBindingConfigs.map(({ key, label }) => `${label}：${policy.third_party_bindings[key] ? '已开启' : '未开启'}`).join('，'),
    [policy]
  );

  async function loadPolicy() {
    setLoading(true);
    try {
      setPolicy(normalizePolicy(await apiRequest<UserSecurityPolicy>('/admin/api/v1/security-policy')));
    } finally {
      setLoading(false);
    }
  }

  function updatePaymentPolicy(action: SecurityActionKey, patch: Partial<PaymentPolicy>) {
    setPolicy((current) => ({
      ...current,
      payment_policies: {
        ...current.payment_policies,
        [action]: { ...current.payment_policies[action], ...patch }
      }
    }));
  }

  function updateThirdPartyBinding(key: ThirdPartyBindingKey, enabled: boolean) {
    setPolicy((current) => ({
      ...current,
      third_party_bindings: {
        ...current.third_party_bindings,
        [key]: enabled
      }
    }));
  }

  async function savePolicy(reason: string) {
    try {
      const saved = await apiRequest<UserSecurityPolicy>('/admin/api/v1/security-policy', {
        method: 'PATCH',
        body: JSON.stringify({ ...policy, reason })
      });
      setPolicy(normalizePolicy(saved));
      Toast.success('保存安全策略已提交');
    } catch (error) {
      Toast.error(errorMessage(error));
      throw error;
    }
  }

  useEffect(() => {
    loadPolicy().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader description="统一控制登录、资金动作验证与第三方绑定策略；保存时需要记录操作原因。" title="安全策略" />
      <div className="admin-policy-workbench">
        <Tabs
          activeKey={activeTab}
          className="admin-action-tabs admin-policy-tabs"
          onChange={(key) => setActiveTab(key as SecurityTab)}
          tabBarExtraContent={
            <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={() => loadPolicy().catch((error) => Toast.error(errorMessage(error)))} theme="borderless">
              刷新策略
            </Button>
          }
          tabList={securityTabs}
          type="button"
        />
        <div aria-labelledby={`semiTab${activeTab}`} id={`semiTabPanel${activeTab}`} role="tabpanel" tabIndex={0}>
          <Card bordered={false} className="admin-policy-card">
          {activeTab === 'login' ? (
            <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
              <div className="admin-section-heading">
                <div>
                  <Text className="admin-section-kicker">ACCESS CONTROL</Text>
                  <Title heading={4}>登录策略</Title>
                </div>
                <Text type="tertiary">管理用户进入 PC 端时的身份验证强度</Text>
              </div>
              <div className="admin-action-form admin-policy-form">
                <label>
                  登录 2FA 策略
                  <AdminSelect
                    ariaLabel="登录 2FA 策略"
                    onChange={(login_2fa_mode) => setPolicy({ ...policy, login_2fa_mode: login_2fa_mode as LoginTwoFactorMode })}
                    optionList={loginModeOptions}
                    value={policy.login_2fa_mode}
                  />
                </label>
                <fieldset className="admin-action-choice-group">
                  <legend>注册策略</legend>
                  <div className="admin-action-choice-list">
                    <div className="admin-action-checkbox">
                      <AdminCheckbox
                        checked={policy.registration_invite_required}
                        onChange={(registration_invite_required) => setPolicy({ ...policy, registration_invite_required })}
                      >
                        注册时必须填写邀请码
                      </AdminCheckbox>
                    </div>
                  </div>
                </fieldset>
                <fieldset className="admin-action-choice-group admin-policy-setting">
                  <legend>登录入口</legend>
                  <div className="admin-action-choice-list">
                    <Space align="center" style={{ justifyContent: 'space-between', width: '100%' }}>
                      <span>允许用户使用用户名和密码登录 PC 端</span>
                      <Switch
                        aria-label="允许用户名登录"
                        checked={policy.username_login_enabled}
                        onChange={(username_login_enabled) => setPolicy({ ...policy, username_login_enabled })}
                      />
                    </Space>
                  </div>
                </fieldset>
              </div>
            </Space>
          ) : null}

          {activeTab === 'payment' ? (
            <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
              <div className="admin-section-heading">
                <div>
                  <Text className="admin-section-kicker">PAYMENT VERIFICATION</Text>
                  <Title heading={4}>资金动作校验</Title>
                </div>
                <Text type="tertiary">按动作独立启用资金密码或双因素认证</Text>
              </div>
              <div className="admin-policy-action-grid">
                {actionConfigs.map(({ key, label }) => {
                  const paymentPolicy = policy.payment_policies[key];
                  return (
                    <fieldset className="admin-action-choice-group admin-policy-setting" key={key}>
                      <legend>{label}校验</legend>
                      <div className="admin-action-choice-list">
                        <div className="admin-action-checkbox">
                          <AdminCheckbox checked={paymentPolicy.enabled} onChange={(enabled) => updatePaymentPolicy(key, { enabled })}>
                            启用{label}校验
                          </AdminCheckbox>
                        </div>
                        <label>
                          {label}校验方式
                          <AdminSelect
                            ariaLabel={`${label}校验方式`}
                            onChange={(method) => updatePaymentPolicy(key, { method: method as SecurityVerificationMethod })}
                            optionList={methodOptions}
                            value={paymentPolicy.method}
                          />
                        </label>
                      </div>
                    </fieldset>
                  );
                })}
              </div>
            </Space>
          ) : null}

          {activeTab === 'third-party' ? (
            <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
              <div className="admin-section-heading">
                <div>
                  <Text className="admin-section-kicker">CONNECTED IDENTITIES</Text>
                  <Title heading={4}>第三方账号绑定</Title>
                </div>
                <Text type="tertiary">控制用户安全中心中可见的外部身份入口</Text>
              </div>
              <div className="admin-policy-action-grid">
                {thirdPartyBindingConfigs.map(({ key, label, description }) => (
                  <fieldset className="admin-action-choice-group admin-policy-setting" key={key}>
                    <legend>{label}</legend>
                    <div className="admin-action-choice-list">
                      <Space align="center" style={{ justifyContent: 'space-between', width: '100%' }}>
                        <span>{description}</span>
                        <Switch
                          aria-label={`允许绑定${label}`}
                          checked={policy.third_party_bindings[key]}
                          onChange={(enabled) => updateThirdPartyBinding(key, enabled)}
                        />
                      </Space>
                    </div>
                  </fieldset>
                ))}
              </div>
            </Space>
          ) : null}

          {activeTab === 'summary' ? (
            <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
              <div className="admin-section-heading">
                <div>
                  <Text className="admin-section-kicker">POLICY OVERVIEW</Text>
                  <Title heading={4}>策略摘要</Title>
                </div>
                <Text type="tertiary">提交前复核当前工作区内的完整策略状态</Text>
              </div>
              <div className="admin-action-summary admin-policy-summary">
                <span>登录策略：{optionLabel(loginModeOptions, policy.login_2fa_mode)}</span>
                <span>注册策略：{registrationSummary}</span>
                <span>{usernameLoginSummary}</span>
                <span>{paymentSummary}</span>
                <span>{thirdPartySummary}</span>
              </div>
            </Space>
          ) : null}
          </Card>
        </div>
        <div className="admin-policy-save-bar">
          <div>
            <Text strong>保存当前安全策略</Text>
            <Text type="tertiary">提交前将要求填写操作原因，并沿用现有审计流程。</Text>
          </div>
          <ConfirmAction actionText="保存安全策略" title="确认保存安全策略" onConfirm={savePolicy} />
        </div>
      </div>
    </main>
  );
}
