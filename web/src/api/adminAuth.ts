import { apiRequest } from './client';
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

export function adminLogin(payload: AdminLoginRequest): Promise<AdminLoginResult> {
  const body = {
    ...payload,
    ...(payload.cf_turnstile_token?.trim() ? { cf_turnstile_token: payload.cf_turnstile_token.trim() } : {}),
  }
  return apiRequest<AdminLoginResult>('/admin/api/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify(body)
  });
}

function normalizeLoginConfig(response: LoginConfigApiResponse): LoginConfig {
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
    const response = await apiRequest<LoginConfigApiResponse>('/api/v1/auth/login/config');
    return normalizeLoginConfig(response);
  } catch {
    const response = await apiRequest<LoginConfigApiResponse>('/admin/api/v1/auth/login/config');
    return normalizeLoginConfig(response);
  }
}

export function adminLoginTwoFactor(payload: AdminLoginTwoFactorRequest): Promise<AdminLoginResponse> {
  return apiRequest<AdminLoginResponse>('/admin/api/v1/auth/login/2fa', {
    method: 'POST',
    body: JSON.stringify(payload)
  });
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
