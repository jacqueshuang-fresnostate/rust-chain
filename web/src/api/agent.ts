import { apiRequest } from './client';

export type AgentMe = Record<string, unknown> & {
  agent_admin_id: number;
  agent_id: number;
  username: string;
  agent_code: string;
  level: number;
  agent_status: string;
  admin_status: string;
  last_login_at?: number | null;
};

export type AgentDashboard = Record<string, unknown> & {
  agent_id: number;
  team_user_count: number;
  active_invite_code_count: number;
  commission_record_count: number;
  pending_commission_amount: string | number;
  settled_commission_amount: string | number;
  total_commission_amount: string | number;
};

export type AgentTeamUser = Record<string, unknown> & {
  user_id: number;
  email?: string | null;
  phone?: string | null;
  status: string;
  kyc_level: number;
  root_agent_id: number;
  depth: number;
  referred_at: number;
};

export type AgentInviteCode = Record<string, unknown> & {
  id: number;
  owner_id: number;
  code: string;
  usage_limit?: number | null;
  used_count: number;
  status: string;
  created_at: number;
};

export type AgentCommission = Record<string, unknown> & {
  id: number;
  user_id: number;
  email?: string | null;
  source_type: string;
  source_id: string;
  source_amount: string | number;
  commission_amount: string | number;
  status: string;
  depth: number;
  payout_ledger_id?: number | null;
  payout_asset_id?: number | null;
  payout_amount?: string | number | null;
  payout_balance_after?: string | number | null;
  payout_created_at?: number | null;
  created_at: number;
};

export type AgentConvertStats = Record<string, unknown> & {
  agent_id: number;
  total_orders: number;
  pending_orders: number;
  completed_orders: number;
  total_from_amount: string | number;
  total_to_amount: string | number;
};

export type AgentTeamTreeNode = Record<string, unknown> & {
  user_id: number;
  email?: string | null;
  phone?: string | null;
  status: string;
  direct_inviter_id?: number | null;
  direct_inviter_type?: string | null;
  depth: number;
  path: string;
  referred_at: number;
};

export type AgentUsersResponse = {
  users: AgentTeamUser[];
};

export type AgentInviteCodesResponse = {
  invite_codes: AgentInviteCode[];
};

export type AgentCommissionsResponse = {
  agent_id: number;
  total_records: number;
  total_commission_amount: string | number;
  commissions: AgentCommission[];
};

export type AgentTeamTreeResponse = {
  root_agent_id: number;
  nodes: AgentTeamTreeNode[];
};

const agentRequest = <T>(path: string, init: RequestInit = {}) =>
  apiRequest<T>(path, {
    ...init,
    authScope: 'agent'
  });

export function getAgentMe(): Promise<AgentMe> {
  return agentRequest<AgentMe>('/agent/api/v1/me');
}

export function getAgentDashboard(): Promise<AgentDashboard> {
  return agentRequest<AgentDashboard>('/agent/api/v1/dashboard');
}

export function getAgentUsers(): Promise<AgentUsersResponse> {
  return agentRequest<AgentUsersResponse>('/agent/api/v1/users');
}

export function getAgentInviteCodes(): Promise<AgentInviteCodesResponse> {
  return agentRequest<AgentInviteCodesResponse>('/agent/api/v1/invite-codes');
}

export function createAgentInviteCode(usageLimit?: number): Promise<AgentInviteCode> {
  return agentRequest<AgentInviteCode>('/agent/api/v1/invite-codes', {
    method: 'POST',
    body: JSON.stringify({ usage_limit: usageLimit })
  });
}

export function updateAgentInviteCodeStatus(inviteCodeId: number, status: 'active' | 'disabled'): Promise<AgentInviteCode> {
  return agentRequest<AgentInviteCode>(`/agent/api/v1/invite-codes/${inviteCodeId}/status`, {
    method: 'PATCH',
    body: JSON.stringify({ status })
  });
}

export function getAgentCommissions(): Promise<AgentCommissionsResponse> {
  return agentRequest<AgentCommissionsResponse>('/agent/api/v1/commissions');
}

export function getAgentConvertStats(): Promise<AgentConvertStats> {
  return agentRequest<AgentConvertStats>('/agent/api/v1/convert/stats');
}

export function getAgentTeamTree(): Promise<AgentTeamTreeResponse> {
  return agentRequest<AgentTeamTreeResponse>('/agent/api/v1/team-tree');
}
