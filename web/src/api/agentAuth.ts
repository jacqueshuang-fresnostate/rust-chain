import { apiRequest } from './client';
import type { LoginRequest, LoginResponse } from './types';

export function agentLogin(payload: LoginRequest): Promise<LoginResponse> {
  return apiRequest<LoginResponse>('/agent/api/v1/auth/login', {
    authScope: 'agent',
    method: 'POST',
    body: JSON.stringify(payload)
  });
}
