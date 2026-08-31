import { beforeEach, describe, expect, it, vi } from 'vitest';

import { authStore } from '../auth/authStore';
import { createStaffSupportApi } from './support';

const conversation = {
  id: 11,
  user_id: 21,
  user_email: null,
  user_phone: null,
  assigned_agent_id: 31,
  assigned_agent_code: 'AG-31',
  status: 'open' as const,
  user_read_message_id: null,
  staff_read_message_id: null,
  user_unread_count: 0,
  staff_unread_count: 2,
  last_message_id: 501,
  last_message_sender_type: 'user' as const,
  last_message_sender_id: 21,
  last_message_preview: '您好',
  last_message_at: 2_000,
  closed_at: null,
  created_at: 1_000,
  updated_at: 2_000
};

const message = {
  id: 501,
  conversation_id: 11,
  sender_type: 'user' as const,
  sender_id: 21,
  client_message_id: 'web-message-11',
  body: '您好',
  read_by_recipient: false,
  created_at: 2_000
};

function jsonResponse(payload: unknown) {
  return new Response(JSON.stringify(payload), { status: 200 });
}

describe('staff support API', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.unstubAllGlobals();
    authStore.setSession({
      accessToken: 'admin-token',
      refreshToken: 'admin-refresh',
      scope: 'admin',
      subject: 'admin:1'
    });
    authStore.setSession({
      accessToken: 'agent-token',
      refreshToken: 'agent-refresh',
      scope: 'agent',
      subject: 'agent:31'
    });
  });

  it('binds every agent operation to the agent prefix and auth scope', async () => {
    const fetchMock = vi.fn().mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input));
      if (url.pathname.endsWith('/conversations')) {
        return jsonResponse({ conversations: [conversation], total: 1 });
      }
      if (url.pathname.endsWith('/messages') && init?.method === 'POST') {
        return jsonResponse({ conversation, message, replayed: false });
      }
      if (url.pathname.endsWith('/messages')) {
        return jsonResponse({ messages: [message], has_more: false, next_before_id: null });
      }
      return jsonResponse(conversation);
    });
    vi.stubGlobal('fetch', fetchMock);
    const api = createStaffSupportApi('agent');

    await api.listConversations({ status: 'open', limit: 100, offset: 0 });
    await api.getConversation(11);
    await api.getMessages(11, { before_id: 501, limit: 100 });
    await api.sendMessage(11, { body: '您好', client_message_id: 'web-message-11' });
    await api.markRead(11, 501);
    await api.setStatus(11, 'closed');

    const paths = fetchMock.mock.calls.map(([input]) => new URL(String(input)));
    expect(paths.map((url) => url.pathname)).toEqual([
      '/agent/api/v1/support/conversations',
      '/agent/api/v1/support/conversations/11',
      '/agent/api/v1/support/conversations/11/messages',
      '/agent/api/v1/support/conversations/11/messages',
      '/agent/api/v1/support/conversations/11/read',
      '/agent/api/v1/support/conversations/11/status'
    ]);
    expect(paths[0].searchParams.get('status')).toBe('open');
    expect(paths[0].searchParams.get('limit')).toBe('100');
    expect(paths[2].searchParams.get('limit')).toBe('100');
    expect(paths[2].searchParams.get('before_id')).toBe('501');
    fetchMock.mock.calls.forEach(([, init]) => {
      expect((init.headers as Headers).get('Authorization')).toBe('Bearer agent-token');
    });
  });

  it('uses only the admin prefix and admin token for the global workbench', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ conversations: [], total: 0 }));
    vi.stubGlobal('fetch', fetchMock);
    const api = createStaffSupportApi('admin');

    await api.listConversations({ unassigned: true, limit: 100, offset: 0 });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe('http://127.0.0.1:8080/admin/api/v1/support/conversations?unassigned=true&limit=100&offset=0');
    expect((fetchMock.mock.calls[0][1].headers as Headers).get('Authorization')).toBe('Bearer admin-token');
  });

  it('keeps reply idempotency and status payloads narrow', async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(jsonResponse({ conversation, message, replayed: false }))
      .mockResolvedValueOnce(jsonResponse(conversation))
      .mockResolvedValueOnce(jsonResponse({ ...conversation, status: 'closed' }));
    vi.stubGlobal('fetch', fetchMock);
    const api = createStaffSupportApi('admin');

    await api.sendMessage(11, { body: '正在为您核实', client_message_id: 'web-message-fixed' });
    await api.markRead(11, 501);
    await api.setStatus(11, 'closed');

    expect(fetchMock.mock.calls[0][1]).toMatchObject({
      method: 'POST',
      body: JSON.stringify({ body: '正在为您核实', client_message_id: 'web-message-fixed' })
    });
    expect(fetchMock.mock.calls[1][1]).toMatchObject({ method: 'POST' });
    expect(fetchMock.mock.calls[1][1].body).toBe(JSON.stringify({ message_id: 501 }));
    expect(fetchMock.mock.calls[2][1]).toMatchObject({
      method: 'PATCH',
      body: JSON.stringify({ status: 'closed' })
    });
  });

  it('normalizes the deployed bare detail and user contact field names', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => jsonResponse({
      ...conversation,
      user_email: 'owner@example.com',
      user_phone: '13800000000'
    }));
    vi.stubGlobal('fetch', fetchMock);

    const detail = await createStaffSupportApi('admin').getConversation(11);

    expect(detail).toMatchObject({
      id: 11,
      email: 'owner@example.com',
      phone: '13800000000'
    });
  });
});
