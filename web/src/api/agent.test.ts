import { beforeEach, describe, expect, it, vi } from 'vitest';

import { authStore } from '../auth/authStore';
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
} from './agent';

function jsonResponse(payload: unknown) {
  return new Response(JSON.stringify(payload), { status: 200 });
}

describe('agent API', () => {
  beforeEach(() => {
    localStorage.clear();
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
      .mockResolvedValueOnce(jsonResponse({ id: 7, code: 'AGT7' }))
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
});
