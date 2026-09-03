import { apiRequest, ContractError } from './client';
import { canonicalDecimalText } from '../shared/decimal';

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

export type AgentSubAgent = Record<string, unknown> & {
  id: number;
  parent_agent_id?: number | null;
  root_agent_id: number;
  agent_code: string;
  level: number;
  path: string;
  status: string;
  direct_user_count: number;
  team_user_count: number;
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
  agents: AgentSubAgent[];
  nodes: AgentTeamTreeNode[];
};

export type AgentSubAgentsResponse = {
  agents: AgentSubAgent[];
};

export type AgentPasswordChangeResponse = {
  changed: boolean;
  requires_relogin: boolean;
};

export type AgentFinancialPageQuery = {
  limit?: number;
  offset?: number;
};

export type AgentMarginPositionStatus = 'opened' | 'closed' | 'canceled' | 'liquidated';
export type AgentSecondsContractOrderStatus = 'opened' | 'settled' | 'manual_review';

export type AgentUserAsset = Record<string, unknown> & {
  account_id: number;
  account_type: 'spot' | 'margin';
  asset_id: number;
  asset_symbol: string;
  logo_url: string | null;
  precision_scale: number;
  available: string;
  frozen: string;
  locked: string;
  updated_at: number;
};

export type AgentUserMarginPosition = Record<string, unknown> & {
  id: number;
  user_id: number;
  product_id: number;
  pair_id: number;
  symbol: string;
  margin_asset: number;
  margin_asset_symbol: string;
  wallet_scope: 'spot' | 'margin';
  margin_mode: 'isolated' | 'cross';
  direction: 'long' | 'short';
  order_type: 'market' | 'limit';
  margin_amount: string;
  leverage: string;
  notional_amount: string;
  borrowed_amount: string;
  interest_amount: string;
  entry_price: string | null;
  limit_price: string | null;
  exit_price: string | null;
  realized_pnl: string | null;
  opened_at: number;
  created_at: number;
  closed_at: number | null;
  status: AgentMarginPositionStatus;
};

export type AgentUserSecondsContractOrder = Record<string, unknown> & {
  id: number;
  user_id: number;
  product_id: number;
  pair_id: number;
  symbol: string;
  stake_asset: number;
  stake_asset_symbol: string;
  direction: 'up' | 'down';
  stake_amount: string;
  duration_seconds: number;
  payout_rate: string;
  entry_price: string | null;
  settlement_price: string | null;
  status: AgentSecondsContractOrderStatus;
  result: 'win' | 'loss' | null;
  expires_at: number;
  created_at: number;
  settled_at: number | null;
};

export type AgentUserAssetsResponse = {
  assets: AgentUserAsset[];
  total: number;
};

export type AgentUserMarginPositionsResponse = {
  positions: AgentUserMarginPosition[];
  total: number;
};

export type AgentUserSecondsContractOrdersResponse = {
  orders: AgentUserSecondsContractOrder[];
  total: number;
};

const agentRequest = <T>(path: string, init: RequestInit = {}) =>
  apiRequest<T>(path, {
    ...init,
    authScope: 'agent'
  });

type AgentFinancialRowContract = {
  decimalFields: readonly string[];
  nullableDecimalFields?: readonly string[];
  integerFields?: readonly string[];
  integerRanges?: Readonly<Record<string, readonly [number, number]>>;
  stringFields?: readonly string[];
  nullableStringFields?: readonly string[];
  timestampFields?: readonly string[];
  nullableTimestampFields?: readonly string[];
  enumFields?: Readonly<Record<string, readonly string[]>>;
  nullableEnumFields?: Readonly<Record<string, readonly string[]>>;
  requiredFields: readonly string[];
};

function appendAgentFinancialQuery(path: string, query: AgentFinancialPageQuery & { status?: string }) {
  const params = new URLSearchParams();
  if (query.status) params.set('status', query.status);
  if (query.limit !== undefined) params.set('limit', String(query.limit));
  if (query.offset !== undefined) params.set('offset', String(query.offset));
  const suffix = params.toString();
  return suffix ? `${path}?${suffix}` : path;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

async function listAgentFinancialRows<T extends Record<string, unknown>>(
  endpoint: string,
  responseKey: string,
  contract: AgentFinancialRowContract
): Promise<{ rows: T[]; total: number }> {
  const value: unknown = await agentRequest<unknown>(endpoint);
  if (!isRecord(value)) {
    throw new ContractError(`接口 ${endpoint} 的响应必须是对象`, { path: endpoint });
  }
  const rows = value[responseKey];
  if (!Array.isArray(rows) || !rows.every(isRecord)) {
    throw new ContractError(`接口 ${endpoint} 的 ${responseKey} 必须是对象数组`, { path: endpoint });
  }
  if (typeof value.total !== 'number' || !Number.isSafeInteger(value.total) || value.total < 0) {
    throw new ContractError(`接口 ${endpoint} 的 total 必须是非负安全整数`, { path: endpoint });
  }
  rows.forEach((row, index) => {
    const assertContract = (condition: boolean, field: string, expected: string) => {
      if (!condition) {
        throw new ContractError(`接口 ${endpoint} 的第 ${index + 1} 行字段 ${field} 必须是${expected}`, { path: endpoint });
      }
    };
    contract.requiredFields.forEach((field) => {
      if (!Object.prototype.hasOwnProperty.call(row, field)) {
        throw new ContractError(`接口 ${endpoint} 的第 ${index + 1} 行缺少必填字段 ${field}`, { path: endpoint });
      }
    });
    contract.decimalFields.forEach((field) => {
      const decimal = row[field];
      assertContract(typeof decimal === 'string' && canonicalDecimalText(decimal) !== null, field, ' Decimal text');
    });
    contract.nullableDecimalFields?.forEach((field) => {
      const decimal = row[field];
      assertContract(decimal === null || (typeof decimal === 'string' && canonicalDecimalText(decimal) !== null), field, ' Decimal text 或 null');
    });
    contract.integerFields?.forEach((field) => {
      const integer = row[field];
      assertContract(typeof integer === 'number' && Number.isSafeInteger(integer) && integer >= 0, field, '非负安全整数');
    });
    Object.entries(contract.integerRanges ?? {}).forEach(([field, [minimum, maximum]]) => {
      const integer = row[field];
      assertContract(
        typeof integer === 'number' && Number.isSafeInteger(integer) && integer >= minimum && integer <= maximum,
        field,
        `${minimum} 至 ${maximum} 的安全整数`
      );
    });
    contract.stringFields?.forEach((field) => {
      const text = row[field];
      assertContract(typeof text === 'string' && text.trim().length > 0, field, '非空字符串');
    });
    contract.nullableStringFields?.forEach((field) => {
      const text = row[field];
      assertContract(text === null || typeof text === 'string', field, '字符串或 null');
    });
    contract.timestampFields?.forEach((field) => {
      const timestamp = row[field];
      assertContract(typeof timestamp === 'number' && Number.isSafeInteger(timestamp), field, 'Unix 毫秒安全整数');
    });
    contract.nullableTimestampFields?.forEach((field) => {
      const timestamp = row[field];
      assertContract(timestamp === null || (typeof timestamp === 'number' && Number.isSafeInteger(timestamp)), field, 'Unix 毫秒安全整数或 null');
    });
    Object.entries(contract.enumFields ?? {}).forEach(([field, allowed]) => {
      assertContract(typeof row[field] === 'string' && allowed.includes(row[field] as string), field, `枚举 ${allowed.join('/')}`);
    });
    Object.entries(contract.nullableEnumFields ?? {}).forEach(([field, allowed]) => {
      const enumValue = row[field];
      assertContract(enumValue === null || (typeof enumValue === 'string' && allowed.includes(enumValue)), field, `枚举 ${allowed.join('/')} 或 null`);
    });
  });
  return { rows: rows as T[], total: value.total };
}

export function getAgentMe(): Promise<AgentMe> {
  return agentRequest<AgentMe>('/agent/api/v1/me');
}

export function getAgentDashboard(): Promise<AgentDashboard> {
  return agentRequest<AgentDashboard>('/agent/api/v1/dashboard');
}

export function getAgentUsers(): Promise<AgentUsersResponse> {
  return agentRequest<AgentUsersResponse>('/agent/api/v1/users');
}

export async function getAgentUserAssets(userId: number, query: AgentFinancialPageQuery = {}): Promise<AgentUserAssetsResponse> {
  const endpoint = appendAgentFinancialQuery(`/agent/api/v1/users/${encodeURIComponent(String(userId))}/assets`, query);
  const response = await listAgentFinancialRows<AgentUserAsset>(endpoint, 'assets', {
    requiredFields: ['account_id', 'account_type', 'asset_id', 'asset_symbol', 'logo_url', 'precision_scale', 'available', 'frozen', 'locked', 'updated_at'],
    decimalFields: ['available', 'frozen', 'locked'],
    integerFields: ['account_id', 'asset_id'],
    integerRanges: { precision_scale: [0, 18] },
    stringFields: ['asset_symbol'],
    nullableStringFields: ['logo_url'],
    timestampFields: ['updated_at'],
    enumFields: { account_type: ['spot', 'margin'] }
  });
  return { assets: response.rows, total: response.total };
}

export async function getAgentUserMarginPositions(
  userId: number,
  query: AgentFinancialPageQuery & { status?: AgentMarginPositionStatus } = {}
): Promise<AgentUserMarginPositionsResponse> {
  const endpoint = appendAgentFinancialQuery(`/agent/api/v1/users/${encodeURIComponent(String(userId))}/margin-positions`, query);
  const response = await listAgentFinancialRows<AgentUserMarginPosition>(endpoint, 'positions', {
    requiredFields: [
      'id', 'user_id', 'product_id', 'pair_id', 'symbol', 'margin_asset', 'margin_asset_symbol', 'wallet_scope', 'margin_mode',
      'direction', 'order_type', 'margin_amount', 'leverage', 'notional_amount', 'borrowed_amount', 'interest_amount', 'entry_price',
      'limit_price', 'exit_price', 'realized_pnl', 'opened_at', 'created_at', 'closed_at', 'status'
    ],
    decimalFields: ['margin_amount', 'leverage', 'notional_amount', 'borrowed_amount', 'interest_amount'],
    nullableDecimalFields: ['entry_price', 'limit_price', 'exit_price', 'realized_pnl'],
    integerFields: ['id', 'user_id', 'product_id', 'pair_id', 'margin_asset'],
    stringFields: ['symbol', 'margin_asset_symbol'],
    timestampFields: ['opened_at', 'created_at'],
    nullableTimestampFields: ['closed_at'],
    enumFields: {
      wallet_scope: ['spot', 'margin'],
      margin_mode: ['isolated', 'cross'],
      direction: ['long', 'short'],
      order_type: ['market', 'limit'],
      status: ['opened', 'closed', 'canceled', 'liquidated']
    }
  });
  return { positions: response.rows, total: response.total };
}

export async function getAgentUserSecondsContractOrders(
  userId: number,
  query: AgentFinancialPageQuery & { status?: AgentSecondsContractOrderStatus } = {}
): Promise<AgentUserSecondsContractOrdersResponse> {
  const endpoint = appendAgentFinancialQuery(`/agent/api/v1/users/${encodeURIComponent(String(userId))}/seconds-contract-orders`, query);
  const response = await listAgentFinancialRows<AgentUserSecondsContractOrder>(endpoint, 'orders', {
    requiredFields: [
      'id', 'user_id', 'product_id', 'pair_id', 'symbol', 'stake_asset', 'stake_asset_symbol', 'direction', 'stake_amount',
      'duration_seconds', 'payout_rate', 'entry_price', 'settlement_price', 'status', 'result', 'expires_at', 'created_at', 'settled_at'
    ],
    decimalFields: ['stake_amount', 'payout_rate'],
    nullableDecimalFields: ['entry_price', 'settlement_price'],
    integerFields: ['id', 'user_id', 'product_id', 'pair_id', 'stake_asset'],
    integerRanges: { duration_seconds: [1, 4_294_967_295] },
    stringFields: ['symbol', 'stake_asset_symbol'],
    timestampFields: ['expires_at', 'created_at'],
    nullableTimestampFields: ['settled_at'],
    enumFields: {
      direction: ['up', 'down'],
      status: ['opened', 'settled', 'manual_review']
    },
    nullableEnumFields: { result: ['win', 'loss'] }
  });
  return { orders: response.rows, total: response.total };
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

export function getAgentSubAgents(): Promise<AgentSubAgentsResponse> {
  return agentRequest<AgentSubAgentsResponse>('/agent/api/v1/sub-agents');
}

export function changeAgentPassword(currentPassword: string, newPassword: string): Promise<AgentPasswordChangeResponse> {
  return agentRequest<AgentPasswordChangeResponse>('/agent/api/v1/password/change', {
    method: 'POST',
    body: JSON.stringify({ current_password: currentPassword, new_password: newPassword })
  });
}
