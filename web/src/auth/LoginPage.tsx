import { IconLock, IconShield } from '@douyinfe/semi-icons';
import { Button, Card, Form, Radio, RadioGroup, Toast, Typography } from '@douyinfe/semi-ui';
import { useMutation } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { adminLogin, adminLoginTwoFactor, isAdminLoginTwoFactorChallenge } from '../api/adminAuth';
import { agentLogin } from '../api/agentAuth';
import { ApiError } from '../api/client';
import type { AdminLoginResponse } from '../api/types';
import hippoLogoLandscape from '../assets/brand/hippo-logo-landscape.png';
import { authStore, type AuthScope } from './authStore';

const { Title, Text } = Typography;

type LoginFormValues = {
  username?: string;
  password?: string;
};

type TwoFactorFormValues = {
  totp_code?: string;
};

type LoginScope = Extract<AuthScope, 'admin' | 'agent'>;

export function LoginPage() {
  const navigate = useNavigate();
  const [loginScope, setLoginScope] = useState<LoginScope>('admin');
  const [challengeId, setChallengeId] = useState<string | null>(null);
  const applySession = (response: AdminLoginResponse) => {
    if (response.scope !== loginScope) {
      Toast.error(loginScope === 'agent' ? '当前账号不是代理' : '当前账号不是管理员');
      return;
    }

    authStore.setSession({
      accessToken: response.access_token,
      refreshToken: response.refresh_token,
      scope: response.scope,
      subject: response.subject ?? loginScope
    });
    navigate(loginScope === 'agent' ? '/agent/dashboard' : '/admin/dashboard', { replace: true });
  };
  const notifyError = (error: unknown) => {
    Toast.error(error instanceof ApiError ? error.message : '登录失败，请稍后重试');
  };
  const loginMutation = useMutation({
    mutationFn: (values: Required<LoginFormValues>) => (loginScope === 'agent' ? agentLogin(values) : adminLogin(values)),
    onSuccess: (response) => {
      // 密码正确但需要二次验证时，后端只返回挑战，不下发任何令牌。
      if (isAdminLoginTwoFactorChallenge(response)) {
        setChallengeId(response.challenge_id);
        return;
      }

      applySession(response);
    },
    onError: notifyError
  });
  const twoFactorMutation = useMutation({
    mutationFn: (totpCode: string) => adminLoginTwoFactor({ challenge_id: challengeId ?? '', totp_code: totpCode }),
    onSuccess: applySession,
    onError: notifyError
  });
  const isAgentLogin = loginScope === 'agent';
  const accountLabel = isAgentLogin ? '代理账号' : '管理员账号';

  useEffect(() => {
    document.title = '登录 · HIPPO Operations';
  }, []);

  return (
    <main className="admin-login-page">
      <section className="admin-login-hero" aria-label="交易所管理后台登录">
        <div className="admin-login-copy">
          <img alt="HIPPO" className="admin-login-logo" src={hippoLogoLandscape} />
          <div className="admin-login-environment">
            <span aria-hidden="true" />
            <Text>PRODUCTION OPERATIONS</Text>
          </div>
          <Text className="admin-login-eyebrow">HIPPO OPERATIONS</Text>
          <Title heading={1}>让每一次运营决策清晰可控</Title>
          <Text className="admin-login-description">
            统一管理用户身份、资产资金、交易市场与风险策略。所有关键操作保留权限校验和审计链路。
          </Text>
          <div className="admin-login-capabilities" aria-label="后台能力">
            <span>实时运营总览</span>
            <span>安全审核工作台</span>
            <span>生产环境审计</span>
          </div>
        </div>
        <Card bordered={false} shadows="always" className="admin-login-card">
          <div className="admin-login-badge">
            <IconShield />
            <span>SECURE ACCESS</span>
          </div>
          <Title heading={3}>登录 HIPPO Operations</Title>
          {challengeId ? (
            <>
              <Text type="tertiary">请输入验证器应用中的 6 位动态码</Text>
              <Form<TwoFactorFormValues>
                className="admin-login-form"
                onSubmit={(values) => {
                  twoFactorMutation.mutate(values.totp_code ?? '');
                }}
              >
                <Form.Input
                  field="totp_code"
                  label="两步验证码"
                  prefix={<IconLock />}
                  placeholder="请输入 6 位验证码"
                  rules={[{ required: true, message: '请输入两步验证码' }]}
                />
                <Button htmlType="submit" theme="solid" type="primary" block loading={twoFactorMutation.isPending}>
                  验证并登录
                </Button>
                <Button theme="borderless" block onClick={() => setChallengeId(null)}>
                  返回重新登录
                </Button>
              </Form>
            </>
          ) : (
            <>
              <Text type="tertiary">请选择登录身份并输入账号密码</Text>
              <Form<LoginFormValues>
                className="admin-login-form"
                onSubmit={(values) => {
                  loginMutation.mutate({
                    username: values.username ?? '',
                    password: values.password ?? ''
                  });
                }}
              >
                <Form.Slot label="登录身份">
                  <RadioGroup value={loginScope} type="button" onChange={(event) => setLoginScope(event.target.value as LoginScope)}>
                    <Radio value="admin">管理员</Radio>
                    <Radio value="agent">代理</Radio>
                  </RadioGroup>
                </Form.Slot>
                <Form.Input
                  field="username"
                  label={accountLabel}
                  prefix={<IconShield />}
                  placeholder={`请输入${accountLabel}`}
                  rules={[{ required: true, message: `请输入${accountLabel}` }]}
                />
                <Form.Input
                  field="password"
                  label="密码"
                  mode="password"
                  prefix={<IconLock />}
                  placeholder="请输入密码"
                  rules={[{ required: true, message: '请输入密码' }]}
                />
                <Button htmlType="submit" theme="solid" type="primary" block loading={loginMutation.isPending}>
                  登录
                </Button>
              </Form>
            </>
          )}
        </Card>
      </section>
    </main>
  );
}
