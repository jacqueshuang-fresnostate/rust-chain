import { apiRequest } from './client';
import type { AdminLoginRequest, AdminLoginResponse } from './types';

export function adminLogin(payload: AdminLoginRequest): Promise<AdminLoginResponse> {
  return apiRequest<AdminLoginResponse>('/admin/api/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify(payload)
  });
}
