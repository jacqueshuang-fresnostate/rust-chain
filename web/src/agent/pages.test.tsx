import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  createAgentInviteCode,
  getAgentCommissions,
  getAgentConvertStats,
  getAgentDashboard,
  getAgentInviteCodes,
  getAgentMe,
  getAgentTeamTree,
  getAgentUsers,
  updateAgentInviteCodeStatus
} from '../api/agent';
import {
  AgentCommissionsPage,
  AgentConvertStatsPage,
  AgentDashboardPage,
  AgentInviteCodesPage,
  AgentTeamTreePage,
  AgentUsersPage
} from './pages';

vi.mock('../api/agent', () => ({
  createAgentInviteCode: vi.fn(),
  getAgentCommissions: vi.fn(),
  getAgentConvertStats: vi.fn(),
  getAgentDashboard: vi.fn(),
  getAgentInviteCodes: vi.fn(),
  getAgentMe: vi.fn(),
  getAgentTeamTree: vi.fn(),
  getAgentUsers: vi.fn(),
  updateAgentInviteCodeStatus: vi.fn()
}));

const createAgentInviteCodeMock = vi.mocked(createAgentInviteCode);
const getAgentCommissionsMock = vi.mocked(getAgentCommissions);
const getAgentConvertStatsMock = vi.mocked(getAgentConvertStats);
const getAgentDashboardMock = vi.mocked(getAgentDashboard);
const getAgentInviteCodesMock = vi.mocked(getAgentInviteCodes);
const getAgentMeMock = vi.mocked(getAgentMe);
const getAgentTeamTreeMock = vi.mocked(getAgentTeamTree);
const getAgentUsersMock = vi.mocked(getAgentUsers);
const updateAgentInviteCodeStatusMock = vi.mocked(updateAgentInviteCodeStatus);

const now = Date.now();

describe('Agent portal pages', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders dashboard identity and metrics', async () => {
    getAgentMeMock.mockResolvedValueOnce({
      agent_admin_id: 9,
      agent_id: 7,
      username: 'agent-admin',
      agent_code: 'AGT-001',
      level: 1,
      agent_status: 'active',
      admin_status: 'active',
      last_login_at: now
    });
    getAgentDashboardMock.mockResolvedValueOnce({
      agent_id: 7,
      team_user_count: 12,
      active_invite_code_count: 3,
      commission_record_count: 5,
      pending_commission_amount: '10.5',
      settled_commission_amount: '20.5',
      total_commission_amount: '31'
    });
    getAgentConvertStatsMock.mockResolvedValueOnce({
      agent_id: 7,
      total_orders: 4,
      pending_orders: 1,
      completed_orders: 3,
      total_from_amount: '100',
      total_to_amount: '98'
    });

    render(<AgentDashboardPage />);

    expect(await screen.findByText('代理编号：AGT-001')).toBeInTheDocument();
    expect(screen.getByText('团队人数')).toBeInTheDocument();
    expect(screen.getByText('待结算佣金')).toBeInTheDocument();
    expect(screen.getByText('闪兑订单')).toBeInTheDocument();
  });

  it('renders team users', async () => {
    getAgentUsersMock.mockResolvedValueOnce({
      users: [
        {
          user_id: 1,
          email: 'team@example.test',
          phone: null,
          status: 'active',
          kyc_level: 2,
          root_agent_id: 7,
          depth: 1,
          referred_at: now
        }
      ]
    });

    render(<AgentUsersPage />);

    expect(await screen.findByText('team@example.test')).toBeInTheDocument();
    expect(screen.getByText('团队用户')).toBeInTheDocument();
  });

  it('creates invite codes and updates invite code status', async () => {
    getAgentInviteCodesMock
      .mockResolvedValueOnce({
        invite_codes: [
          { id: 7, owner_id: 3, code: 'AGT7', usage_limit: 10, used_count: 1, status: 'active', created_at: now }
        ]
      })
      .mockResolvedValue({ invite_codes: [] });
    createAgentInviteCodeMock.mockResolvedValueOnce({
      id: 8,
      owner_id: 3,
      code: 'AGT8',
      usage_limit: 10,
      used_count: 0,
      status: 'active',
      created_at: now
    });
    updateAgentInviteCodeStatusMock.mockResolvedValueOnce({
      id: 7,
      owner_id: 3,
      code: 'AGT7',
      usage_limit: 10,
      used_count: 1,
      status: 'disabled',
      created_at: now
    });

    render(<AgentInviteCodesPage />);

    expect(await screen.findByText('AGT7')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('使用上限'), { target: { value: '10' } });
    fireEvent.click(screen.getByRole('button', { name: '创建邀请码' }));
    await waitFor(() => expect(createAgentInviteCodeMock).toHaveBeenCalledWith(10));

    fireEvent.click(screen.getByRole('button', { name: '禁用' }));
    await waitFor(() => expect(updateAgentInviteCodeStatusMock).toHaveBeenCalledWith(7, 'disabled'));
  });

  it('renders commission records', async () => {
    getAgentCommissionsMock.mockResolvedValueOnce({
      agent_id: 7,
      total_records: 1,
      total_commission_amount: '12.5',
      commissions: [
        {
          id: 11,
          user_id: 1,
          email: 'buyer@example.test',
          source_type: 'convert',
          source_id: 'quote-1',
          source_amount: '100',
          commission_amount: '12.5',
          status: 'settled',
          depth: 1,
          payout_ledger_id: 99,
          payout_asset_id: 2,
          payout_amount: '12.5',
          payout_balance_after: '20',
          payout_created_at: now,
          created_at: now
        }
      ]
    });

    render(<AgentCommissionsPage />);

    expect(await screen.findByText('buyer@example.test')).toBeInTheDocument();
    expect(screen.getByText('quote-1')).toBeInTheDocument();
  });

  it('renders convert stats', async () => {
    getAgentConvertStatsMock.mockResolvedValueOnce({
      agent_id: 7,
      total_orders: 15,
      pending_orders: 2,
      completed_orders: 13,
      total_from_amount: '500',
      total_to_amount: '498'
    });

    render(<AgentConvertStatsPage />);

    expect(await screen.findByText('闪兑统计')).toBeInTheDocument();
    expect(screen.getByText('待处理订单')).toBeInTheDocument();
    expect(screen.getByText('15.00')).toBeInTheDocument();
  });

  it('renders team tree nodes', async () => {
    getAgentTeamTreeMock.mockResolvedValueOnce({
      root_agent_id: 7,
      nodes: [
        {
          user_id: 1,
          email: 'node@example.test',
          phone: null,
          status: 'active',
          direct_inviter_id: 7,
          direct_inviter_type: 'agent',
          depth: 1,
          path: '7/1',
          referred_at: now
        }
      ]
    });

    render(<AgentTeamTreePage />);

    expect(await screen.findByText('node@example.test')).toBeInTheDocument();
    expect(screen.getByText('7/1')).toBeInTheDocument();
  });
});
