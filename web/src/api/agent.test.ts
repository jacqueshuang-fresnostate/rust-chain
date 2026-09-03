import { beforeEach, describe, expect, it, vi } from 'vitest';

import { authStore } from '../auth/authStore';
import { ContractError } from './client';
import {
  createAgentInviteCode,
  getAgentCommissions,
  getAgentConvertStats,
  getAgentDashboard,
  getAgentInviteCodes,
  getAgentMe,
  getAgentTeamTree,
  getAgentUserAssets,
  getAgentUserMarginPositions,
  getAgentUserSecondsContractOrders,
  getAgentUsers,
  updateAgentInviteCodeStatus
} from './agent';

function jsonResponse(payload: unknown) {
  return new Response(JSON.stringify(payload), { status: 200 });
}

describe('agent API', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.unstubAllGlobals();
    authStore.setSession({ accessToken: 'admin-token', refreshToken: 'admin-refresh', scope: 'admin', subject: 'admin:1' });
    authStore.setSession({ accessToken: 'agent-token', refreshToken: 'agent-refresh', scope: 'agent', subject: 'agent:9' });
  });

  it('uses the agent auth scope for portal reads', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ agent_admin_id: 9 }))
      .mockResolvedValueOnce(jsonResponse({ team_user_count: 2 }))
      .mockResolvedValueOnce(jsonResponse({ users: [] }))
      .mockResolvedValueOnce(jsonResponse({ invite_codes: [] }))
      .mockResolvedValueOnce(jsonResponse({ commissions: [] }))
      .mockResolvedValueOnce(jsonResponse({ total_orders: 3 }))
      .mockResolvedValueOnce(jsonResponse({ root_agent_id: 1, nodes: [] }));
    vi.stubGlobal('fetch', fetchMock);

    await getAgentMe();
    await getAgentDashboard();
    await getAgentUsers();
    await getAgentInviteCodes();
    await getAgentCommissions();
    await getAgentConvertStats();
    await getAgentTeamTree();

    expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
      'http://127.0.0.1:8080/agent/api/v1/me',
      'http://127.0.0.1:8080/agent/api/v1/dashboard',
      'http://127.0.0.1:8080/agent/api/v1/users',
      'http://127.0.0.1:8080/agent/api/v1/invite-codes',
      'http://127.0.0.1:8080/agent/api/v1/commissions',
      'http://127.0.0.1:8080/agent/api/v1/convert/stats',
      'http://127.0.0.1:8080/agent/api/v1/team-tree'
    ]);
    fetchMock.mock.calls.forEach((call) => {
      const headers = call[1].headers as Headers;
      expect(headers.get('Authorization')).toBe('Bearer agent-token');
    });
  });

  it('creates invite codes and updates status with the agent auth scope', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ id: 7, code: 'A1B2C3' }))
      .mockResolvedValueOnce(jsonResponse({ id: 7, status: 'disabled' }));
    vi.stubGlobal('fetch', fetchMock);

    await createAgentInviteCode(10);
    await updateAgentInviteCodeStatus(7, 'disabled');

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      'http://127.0.0.1:8080/agent/api/v1/invite-codes',
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ usage_limit: 10 }) })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      'http://127.0.0.1:8080/agent/api/v1/invite-codes/7/status',
      expect.objectContaining({ method: 'PATCH', body: JSON.stringify({ status: 'disabled' }) })
    );
    fetchMock.mock.calls.forEach((call) => {
      const headers = call[1].headers as Headers;
      expect(headers.get('Authorization')).toBe('Bearer agent-token');
    });
  });

  it('loads paged team-user financial views with agent auth and exact query parameters', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({
        assets: [{
          account_id: 1,
          account_type: 'spot',
          asset_id: 2,
          asset_symbol: 'USDT',
          logo_url: null,
          precision_scale: 18,
          available: '123.456789012345678901',
          frozen: '2.000000000000000000',
          locked: '3.000000000000000000',
          updated_at: 1_735_732_800_000
        }],
        total: 2
      }))
      .mockResolvedValueOnce(jsonResponse({
        positions: [{
          id: 3,
          user_id: 42,
          product_id: 4,
          pair_id: 5,
          symbol: 'BTC-USDT',
          margin_asset: 2,
          margin_asset_symbol: 'USDT',
          wallet_scope: 'spot',
          margin_mode: 'isolated',
          direction: 'long',
          order_type: 'market',
          margin_amount: '10.000000000000000000',
          leverage: '2.00000000',
          notional_amount: '20.000000000000000000',
          borrowed_amount: '10.000000000000000000',
          interest_amount: '0.500000000000000000',
          entry_price: '100.000000000000000000',
          limit_price: null,
          exit_price: null,
          realized_pnl: null,
          opened_at: 1_735_732_800_000,
          created_at: 1_735_732_800_000,
          closed_at: null,
          status: 'opened'
        }],
        total: 1
      }))
      .mockResolvedValueOnce(jsonResponse({
        orders: [{
          id: 6,
          user_id: 42,
          product_id: 7,
          pair_id: 5,
          symbol: 'BTC-USDT',
          stake_asset: 2,
          stake_asset_symbol: 'USDT',
          direction: 'up',
          stake_amount: '7.250000000000000000',
          duration_seconds: 60,
          payout_rate: '0.80000000',
          entry_price: '100.000000000000000000',
          settlement_price: null,
          status: 'opened',
          result: null,
          expires_at: 1_735_732_860_000,
          created_at: 1_735_732_800_000,
          settled_at: null
        }],
        total: 3
      }));
    vi.stubGlobal('fetch', fetchMock);

    await expect(getAgentUserAssets(42, { limit: 20, offset: 20 })).resolves.toMatchObject({ total: 2 });
    await expect(getAgentUserMarginPositions(42, { limit: 10, offset: 0, status: 'opened' })).resolves.toMatchObject({ total: 1 });
    await expect(getAgentUserSecondsContractOrders(42, { limit: 50, offset: 0 })).resolves.toMatchObject({ total: 3 });

    expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
      'http://127.0.0.1:8080/agent/api/v1/users/42/assets?limit=20&offset=20',
      'http://127.0.0.1:8080/agent/api/v1/users/42/margin-positions?status=opened&limit=10&offset=0',
      'http://127.0.0.1:8080/agent/api/v1/users/42/seconds-contract-orders?limit=50&offset=0'
    ]);
    fetchMock.mock.calls.forEach((call) => {
      expect((call[1].headers as Headers).get('Authorization')).toBe('Bearer agent-token');
    });
  });

  it('rejects rounded financial numbers and invalid totals at the API boundary', async () => {
    const validAsset = {
      account_id: 1,
      account_type: 'spot',
      asset_id: 2,
      asset_symbol: 'USDT',
      logo_url: null,
      precision_scale: 18,
      available: '1.000000000000000000',
      frozen: '0.000000000000000000',
      locked: '0.000000000000000000',
      updated_at: 1_735_732_800_000
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ assets: [{ ...validAsset, available: 1 }], total: 1 }))
      .mockResolvedValueOnce(jsonResponse({ assets: [validAsset], total: -1 }));
    vi.stubGlobal('fetch', fetchMock);

    await expect(getAgentUserAssets(42)).rejects.toBeInstanceOf(ContractError);
    await expect(getAgentUserAssets(42)).rejects.toBeInstanceOf(ContractError);
  });

  it('rejects incomplete metadata, invalid precision, non-millisecond timestamps, and unknown enums', async () => {
    const validAsset = {
      account_id: 1,
      account_type: 'spot',
      asset_id: 2,
      asset_symbol: 'USDT',
      logo_url: null,
      precision_scale: 18,
      available: '1.000000000000000000',
      frozen: '0.000000000000000000',
      locked: '0.000000000000000000',
      updated_at: 1_735_732_800_000
    };
    const invalidDurationOrder = {
      id: 6,
      user_id: 42,
      product_id: 7,
      pair_id: 5,
      symbol: 'BTC-USDT',
      stake_asset: 2,
      stake_asset_symbol: 'USDT',
      direction: 'up',
      stake_amount: '7.250000000000000000',
      duration_seconds: 0,
      payout_rate: '0.80000000',
      entry_price: '100.000000000000000000',
      settlement_price: null,
      status: 'opened',
      result: null,
      expires_at: 1_735_732_860_000,
      created_at: 1_735_732_800_000,
      settled_at: null
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ assets: [{ ...validAsset, logo_url: undefined }], total: 1 }))
      .mockResolvedValueOnce(jsonResponse({ assets: [{ ...validAsset, precision_scale: 19 }], total: 1 }))
      .mockResolvedValueOnce(jsonResponse({ assets: [{ ...validAsset, updated_at: '1735732800000' }], total: 1 }))
      .mockResolvedValueOnce(jsonResponse({ assets: [{ ...validAsset, account_type: 'savings' }], total: 1 }))
      .mockResolvedValueOnce(jsonResponse({ orders: [invalidDurationOrder], total: 1 }));
    vi.stubGlobal('fetch', fetchMock);

    await expect(getAgentUserAssets(42)).rejects.toBeInstanceOf(ContractError);
    await expect(getAgentUserAssets(42)).rejects.toBeInstanceOf(ContractError);
    await expect(getAgentUserAssets(42)).rejects.toBeInstanceOf(ContractError);
    await expect(getAgentUserAssets(42)).rejects.toBeInstanceOf(ContractError);
    await expect(getAgentUserSecondsContractOrders(42)).rejects.toBeInstanceOf(ContractError);
  });
});
