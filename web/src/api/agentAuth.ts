import { apiRequest } from './client';
import type { LoginRequest, LoginResponse } from './types';

export function agentLogin(payload: LoginRequest): Promise<LoginResponse> {
  const body = {
    ...payload,
    ...(payload.cf_turnstile_token?.trim() ? { cf_turnstile_token: payload.cf_turnstile_token.trim() } : {}),
  };
  return apiRequest<LoginResponse>('/agent/api/v1/auth/login', {
    authScope: 'agent',
    method: 'POST',
    body: JSON.stringify(body)
  });
}
