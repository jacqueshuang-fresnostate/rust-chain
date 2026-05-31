import type { AuthScope } from '../auth/authStore';

export interface ApiErrorPayload {
  code?: string;
  message?: string;
}

export interface AdminLoginRequest {
  username: string;
  password: string;
}

export interface AdminLoginResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  scope: AuthScope;
  subject?: string;
}

export type LoginRequest = AdminLoginRequest;
export type LoginResponse = AdminLoginResponse;

export interface PageResponse<T> {
  items?: T[];
  logs?: T[];
  orders?: T[];
  trades?: T[];
  projects?: T[];
  markets?: T[];
  pairs?: T[];
  strategies?: T[];
  users?: T[];
  commissions?: T[];
  subscriptions?: T[];
  distributions?: T[];
  purchases?: T[];
  lock_positions?: T[];
  unlocks?: T[];
  positions?: T[];
  liquidations?: T[];
  summaries?: T[];
  products?: T[];
}

export type ApiRecord = Record<string, string | number | boolean | null | object | undefined>;
