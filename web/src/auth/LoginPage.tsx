import { IconLock, IconShield } from '@douyinfe/semi-icons';
import { Button, Card, Form, Radio, RadioGroup, Toast, Typography } from '@douyinfe/semi-ui';
import { useMutation } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

import {
  adminLogin,
  adminLoginTwoFactor,
  getLoginConfig,
  isAdminLoginTwoFactorChallenge,
} from '../api/adminAuth';
import { agentLogin } from '../api/agentAuth';
import { ApiError } from '../api/client';
import type { AdminLoginResponse } from '../api/types';
import hippoLogoLandscape from '../assets/brand/hippo-logo-landscape.png';
import { authStore, authSubjectFromAccessToken, type AuthScope } from './authStore';
import { safeInternalRedirect } from './internalRedirect';
import { createTurnstileLifecycle, type TurnstileLifecycle } from './turnstile';

const { Title, Text } = Typography;

type LoginFormValues = {
  username?: string;
  password?: string;
};

type TwoFactorFormValues = {
  totp_code?: string;
};

type LoginScope = Extract<AuthScope, 'admin' | 'agent'>;

const turnstileRequiredText = '请先完成 Cloudflare 人机校验。';
const turnstileTokenRequiredError = 'cf_turnstile_token is required';

function isTurnstileTokenMissingError(error: unknown): boolean {
  if (error instanceof ApiError) {
    return error.code === 'CF_TURNSTILE_TOKEN_MISSING' || error.message === turnstileTokenRequiredError;
  }

  const payload = error as { message?: string };
  return payload?.message === turnstileTokenRequiredError;
}

export function LoginPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const [loginScope, setLoginScope] = useState<LoginScope>('admin');
  const [challengeId, setChallengeId] = useState<string | null>(null);
  const [cfTurnstileToken, setCfTurnstileToken] = useState('');
  const [turnstileSiteKey, setTurnstileSiteKey] = useState(String(import.meta.env.VITE_CF_TURNSTILE_SITE_KEY ?? '').trim());
  const [turnstileRequired, setTurnstileRequired] = useState<boolean | null>(null);
  const [turnstileRefreshRevision, setTurnstileRefreshRevision] = useState(0);
  const turnstileEnabled = Boolean(turnstileSiteKey) && turnstileRequired === true;
  const turnstileContainerRef = useRef<HTMLDivElement | null>(null);
  const turnstileMountedRef = useRef(false);
  const turnstileLifecycleRef = useRef<TurnstileLifecycle | null>(null);
  const loginSubmissionRef = useRef(false);
  const twoFactorSubmissionRef = useRef(false);
  if (!turnstileLifecycleRef.current) {
    turnstileLifecycleRef.current = createTurnstileLifecycle();
  }
  const turnstileLifecycle = turnstileLifecycleRef.current;

  const removeTurnstileWidget = () => {
    turnstileLifecycle.remove();
    if (turnstileMountedRef.current) {
      setCfTurnstileToken('');
    }
  };

  const initializeTurnstile = async (siteKey: string) => {
    const normalizedSiteKey = String(siteKey).trim();
    if (!normalizedSiteKey || !turnstileMountedRef.current) {
      return;
    }

    setCfTurnstileToken('');
    await turnstileLifecycle.render({
      resolveContainer: () => turnstileContainerRef.current,
      isContainerCurrent: (container) => turnstileMountedRef.current && turnstileContainerRef.current === container,
      options: {
        sitekey: normalizedSiteKey,
      },
      callbacks: {
        callback: (token: string) => {
          setCfTurnstileToken(token || '');
        },
        expired: () => {
          setCfTurnstileToken('');
        },
        error: () => {
          setCfTurnstileToken('');
        },
        timeout: () => {
          setCfTurnstileToken('');
        },
      },
      onError: () => {
        Toast.error('Cloudflare 人机校验加载失败，请稍后重试。');
        setCfTurnstileToken('');
      },
    });
  };

  const resetTurnstileWidget = () => {
    if (!turnstileMountedRef.current) {
      turnstileLifecycle.remove();
      return;
    }
    setCfTurnstileToken('');
    if (!turnstileLifecycle.reset() && turnstileEnabled && !challengeId) {
      void initializeTurnstile(turnstileSiteKey);
    }
  };

  const refreshTurnstileConfig = async () => {
    try {
      const config = await getLoginConfig();
      if (!turnstileMountedRef.current) return;
      setTurnstileRequired(config.cfTurnstileEnabled);
      const nextSiteKey = String(config.cfTurnstileSiteKey || '').trim() || String(turnstileSiteKey).trim();
      setTurnstileSiteKey(nextSiteKey);
      setTurnstileRefreshRevision((revision) => revision + 1);
    } catch {
      if (turnstileMountedRef.current) {
        Toast.error('Cloudflare 人机校验配置加载失败，请稍后重试。');
      }
    }
  };

  const applySession = (response: AdminLoginResponse, attemptedScope: LoginScope) => {
    if (response.scope !== attemptedScope) {
      Toast.error(attemptedScope === 'agent' ? '当前账号不是代理' : '当前账号不是管理员');
      return;
    }

    authStore.setSession({
      accessToken: response.access_token,
      refreshToken: response.refresh_token,
      scope: response.scope,
      subject: response.subject?.trim() || authSubjectFromAccessToken(response.access_token, attemptedScope),
    });
    const fallback = attemptedScope === 'agent' ? '/agent/dashboard' : '/admin/dashboard';
    const from = (location.state as { from?: unknown } | null)?.from;
    navigate(safeInternalRedirect(from, fallback, attemptedScope === 'agent' ? '/agent' : '/admin'), { replace: true, state: null });
  };

  const notifyError = (error: unknown) => {
    if (isTurnstileTokenMissingError(error)) {
      Toast.error(turnstileRequiredText);
      resetTurnstileWidget();
      void refreshTurnstileConfig();
      return;
    }

    Toast.error(error instanceof ApiError ? error.message : '登录失败，请稍后重试');
    resetTurnstileWidget();
  };

  const loginMutation = useMutation({
    retry: false,
    mutationFn: (attempt: Required<LoginFormValues> & { scope: LoginScope; turnstileToken: string }) => {
      const payload = {
        username: attempt.username,
        password: attempt.password,
        ...(attempt.turnstileToken ? { cf_turnstile_token: attempt.turnstileToken } : {}),
      };

      return attempt.scope === 'agent' ? agentLogin(payload) : adminLogin(payload);
    },
    onSuccess: (response, attempt) => {
      // 密码正确但需要二次验证时，后端只返回挑战，不下发任何令牌。
      if (isAdminLoginTwoFactorChallenge(response)) {
        removeTurnstileWidget();
        setChallengeId(response.challenge_id);
        return;
      }

      applySession(response, attempt.scope);
    },
    onError: notifyError,
    onSettled: () => {
      loginSubmissionRef.current = false;
    },
  });

  const twoFactorMutation = useMutation({
    retry: false,
    mutationFn: (attempt: { challengeId: string; totpCode: string }) => adminLoginTwoFactor({ challenge_id: attempt.challengeId, totp_code: attempt.totpCode }),
    onSuccess: (response) => applySession(response, 'admin'),
    onError: notifyError,
    onSettled: () => {
      twoFactorSubmissionRef.current = false;
    },
  });

  const isAgentLogin = loginScope === 'agent';
  const accountLabel = isAgentLogin ? '代理账号' : '管理员账号';

  useEffect(() => {
    turnstileMountedRef.current = true;
    document.title = '登录 · HIPPO 管理后台';

    getLoginConfig()
      .then((config) => {
        if (!turnstileMountedRef.current) return;
        setTurnstileRequired(config.cfTurnstileEnabled);
        setTurnstileSiteKey((currentKey) => config.cfTurnstileSiteKey || currentKey);
      })
      .catch(() => {
        if (turnstileMountedRef.current) {
          setTurnstileRequired(Boolean(turnstileSiteKey));
        }
      });

    return () => {
      turnstileMountedRef.current = false;
      turnstileLifecycle.remove();
    };
  }, []);

  useEffect(() => {
    if (!turnstileEnabled || challengeId) {
      removeTurnstileWidget();
      return;
    }

    void initializeTurnstile(turnstileSiteKey);

    return () => {
      removeTurnstileWidget();
    };
  }, [challengeId, turnstileEnabled, turnstileRefreshRevision, turnstileSiteKey]);

  return (
    <main className="admin-login-page">
      <section className="admin-login-hero" aria-label="交易所管理后台登录">
        <div className="admin-login-copy">
          <img alt="HIPPO" className="admin-login-logo" src={hippoLogoLandscape} />
          <div className="admin-login-environment">
            <span aria-hidden="true" />
            <Text>生产环境</Text>
          </div>
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
            <span>安全访问</span>
          </div>
          <Title heading={3}>管理员登录</Title>
          {challengeId ? (
            <>
              <Text type="tertiary">请输入验证器应用中的 6 位动态码</Text>
              <Form<TwoFactorFormValues>
                className="admin-login-form"
                onSubmit={(values) => {
                  if (twoFactorSubmissionRef.current || !challengeId) return;
                  twoFactorSubmissionRef.current = true;
                  twoFactorMutation.mutate({ challengeId, totpCode: values.totp_code ?? '' });
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
                <Button
                  theme="borderless"
                  block
                  onClick={() => {
                    setChallengeId(null);
                  }}
                >
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
                  if (loginSubmissionRef.current) return;
                  if (turnstileEnabled && !cfTurnstileToken) {
                    Toast.error(turnstileRequiredText);
                    return;
                  }
                  loginSubmissionRef.current = true;
                  const turnstileToken = cfTurnstileToken.trim();
                  // Turnstile token 单次使用：请求发出后立即从 UI 状态移除，失败只能由用户换新 token 重试。
                  setCfTurnstileToken('');
                  loginMutation.mutate({
                    username: values.username ?? '',
                    password: values.password ?? '',
                    scope: loginScope,
                    turnstileToken,
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
                {turnstileEnabled ? (
                  <div className="admin-login-turnstile-wrap">
                    <div ref={turnstileContainerRef} className="admin-login-turnstile-widget" />
                  </div>
                ) : null}
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
