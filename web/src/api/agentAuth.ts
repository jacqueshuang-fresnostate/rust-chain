import { apiRequest, ContractError } from './client';
import type { LoginRequest, LoginResponse } from './types';

export async function agentLogin(payload: LoginRequest): Promise<LoginResponse> {
  const body = {
    ...payload,
    ...(payload.cf_turnstile_token?.trim() ? { cf_turnstile_token: payload.cf_turnstile_token.trim() } : {}),
  };
  const path = '/agent/api/v1/auth/login';
  const response = await apiRequest<unknown>(path, {
    auth: 'none',
    authScope: 'agent',
    method: 'POST',
    body: JSON.stringify(body)
  });
  if (!response || typeof response !== 'object') throw new ContractError('代理登录响应必须是对象', { path });
  const record = response as Record<string, unknown>;
  if (
    typeof record.access_token !== 'string' ||
    !record.access_token ||
    typeof record.refresh_token !== 'string' ||
    !record.refresh_token ||
    typeof record.token_type !== 'string' ||
    record.scope !== 'agent' ||
    (record.subject !== undefined && (typeof record.subject !== 'string' || !record.subject.trim()))
  ) {
    throw new ContractError('代理登录会话响应字段无效', { path });
  }
  return record as unknown as LoginResponse;
}
