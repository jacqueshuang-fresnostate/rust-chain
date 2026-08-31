import { ApiError, apiRequest, ContractError } from './client';
import type { AdminLoginRequest, AdminLoginResponse } from './types';

export interface LoginConfigApiResponse {
  username_login_enabled?: boolean;
  cf_turnstile_enabled?: boolean;
  cf_turnstile_site_key?: string;
}

export interface LoginConfig {
  usernameLoginEnabled: boolean;
  cfTurnstileEnabled: boolean;
  cfTurnstileSiteKey: string;
}

export interface AdminLoginTwoFactorChallenge {
  requires_2fa: boolean;
  challenge_id: string;
  expires_in_seconds: number;
}

export type AdminLoginResult = AdminLoginResponse | AdminLoginTwoFactorChallenge;

export interface AdminLoginTwoFactorRequest {
  challenge_id: string;
  totp_code: string;
}

export function isAdminLoginTwoFactorChallenge(result: AdminLoginResult): result is AdminLoginTwoFactorChallenge {
  return (result as AdminLoginTwoFactorChallenge).requires_2fa === true;
}

function parseLoginResult(value: unknown, path: string): AdminLoginResult {
  if (!value || typeof value !== 'object') throw new ContractError('登录响应必须是对象', { path });
  const record = value as Record<string, unknown>;
  if (record.requires_2fa === true) {
    if (typeof record.challenge_id !== 'string' || !record.challenge_id || typeof record.expires_in_seconds !== 'number') {
      throw new ContractError('两步验证挑战响应字段无效', { path });
    }
    return { requires_2fa: true, challenge_id: record.challenge_id, expires_in_seconds: record.expires_in_seconds };
  }
  if (
    typeof record.access_token !== 'string' ||
    !record.access_token ||
    typeof record.refresh_token !== 'string' ||
    !record.refresh_token ||
    typeof record.token_type !== 'string' ||
    (record.scope !== 'admin' && record.scope !== 'agent' && record.scope !== 'user') ||
    (record.subject !== undefined && (typeof record.subject !== 'string' || !record.subject.trim()))
  ) {
    throw new ContractError('登录会话响应字段无效', { path });
  }
  return record as unknown as AdminLoginResponse;
}

export async function adminLogin(payload: AdminLoginRequest): Promise<AdminLoginResult> {
  const body = {
    ...payload,
    ...(payload.cf_turnstile_token?.trim() ? { cf_turnstile_token: payload.cf_turnstile_token.trim() } : {}),
  }
  const path = '/admin/api/v1/auth/login';
  const response = await apiRequest<unknown>(path, {
    auth: 'none',
    method: 'POST',
    body: JSON.stringify(body)
  });
  return parseLoginResult(response, path);
}

function normalizeLoginConfig(response: LoginConfigApiResponse): LoginConfig {
  if (typeof response.cf_turnstile_enabled !== 'boolean') {
    throw new ContractError('登录配置缺少 cf_turnstile_enabled', { path: '/api/v1/auth/login/config' });
  }
  return {
    usernameLoginEnabled: Boolean(response.username_login_enabled),
    cfTurnstileEnabled: Boolean(response.cf_turnstile_enabled),
    cfTurnstileSiteKey: String(response.cf_turnstile_site_key || '').trim(),
  };
}

export async function getLoginConfig(): Promise<LoginConfig> {
  try {
    // This public read-only endpoint carries the same policy and is less likely to be
    // intercepted by an admin-path Cloudflare Managed Challenge rule.
    const response = await apiRequest<LoginConfigApiResponse>('/api/v1/auth/login/config', { auth: 'none' });
    return normalizeLoginConfig(response);
  } catch (error) {
    if (!(error instanceof ApiError)) throw error;
    const response = await apiRequest<LoginConfigApiResponse>('/admin/api/v1/auth/login/config', { auth: 'none' });
    return normalizeLoginConfig(response);
  }
}

export async function adminLoginTwoFactor(payload: AdminLoginTwoFactorRequest): Promise<AdminLoginResponse> {
  const path = '/admin/api/v1/auth/login/2fa';
  const response = await apiRequest<unknown>(path, {
    auth: 'none',
    method: 'POST',
    body: JSON.stringify(payload)
  });
  const parsed = parseLoginResult(response, path);
  if (isAdminLoginTwoFactorChallenge(parsed)) throw new ContractError('2FA 验证不得再返回挑战', { path });
  return parsed;
}

export type AdminTwoFactorStatus = {
  totp_enabled: boolean;
};

export type AdminTwoFactorSetup = {
  otpauth_uri: string;
  secret: string;
};

export function getAdminTwoFactorStatus(): Promise<AdminTwoFactorStatus> {
  return apiRequest<AdminTwoFactorStatus>('/admin/api/v1/auth/2fa');
}

export function setupAdminTwoFactor(): Promise<AdminTwoFactorSetup> {
  return apiRequest<AdminTwoFactorSetup>('/admin/api/v1/auth/2fa/setup', { method: 'POST' });
}

export function confirmAdminTwoFactor(totpCode: string): Promise<AdminTwoFactorStatus> {
  return apiRequest<AdminTwoFactorStatus>('/admin/api/v1/auth/2fa/confirm', {
    method: 'POST',
    body: JSON.stringify({ totp_code: totpCode })
  });
}

export function disableAdminTwoFactor(totpCode: string): Promise<AdminTwoFactorStatus> {
  return apiRequest<AdminTwoFactorStatus>('/admin/api/v1/auth/2fa/disable', {
    method: 'POST',
    body: JSON.stringify({ totp_code: totpCode })
  });
}
