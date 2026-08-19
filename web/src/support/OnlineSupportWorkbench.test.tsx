import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  StaffSupportApi,
  SupportConversation,
  SupportConversationStatus,
  SupportMessage
} from '../api/support';
import {
  filterSupportConversations,
  OnlineSupportWorkbench,
  supportAssignmentLabel
} from './OnlineSupportWorkbench';

function conversation(overrides: Partial<SupportConversation> = {}): SupportConversation {
  return {
    id: 11,
    user_id: 101,
    email: 'user101@example.com',
    phone: null,
    assigned_agent_id: 31,
    assigned_agent_code: 'AG-31',
    status: 'open',
    user_read_message_id: null,
    staff_read_message_id: null,
    last_message_preview: '充值问题',
    last_message_id: 501,
    last_message_sender_type: 'user',
    last_message_sender_id: 101,
    last_message_at: 2_000,
    closed_at: null,
    user_unread_count: 0,
    staff_unread_count: 2,
    created_at: 1_000,
    updated_at: 2_000,
    ...overrides
  };
}

const messages: SupportMessage[] = [
  {
    id: 501,
    conversation_id: 11,
    sender_type: 'user',
    sender_id: 101,
    client_message_id: 'customer-message-501',
    body: '请帮我核实充值记录',
    read_by_recipient: false,
    created_at: 2_000
  }
];

function supportApi(rows: SupportConversation[]): StaffSupportApi {
  return {
    listConversations: vi.fn(async () => ({ conversations: rows, total: rows.length })),
    getConversation: vi.fn(async (id) => rows.find((row) => row.id === id) ?? rows[0]),
    getMessages: vi.fn(async () => ({ has_more: false, messages, next_before_id: null })),
    sendMessage: vi.fn(async (id, input) => ({
      conversation: rows.find((row) => row.id === id) ?? rows[0],
      message: {
        ...messages[0],
        body: input.body,
        client_message_id: input.client_message_id,
        sender_type: 'agent' as const
      },
      replayed: false
    })),
    markRead: vi.fn(async (id) => ({
      ...(rows.find((row) => row.id === id) ?? rows[0]),
      staff_unread_count: 0
    })),
    setStatus: vi.fn(async (id, status) => ({
      ...(rows.find((row) => row.id === id) ?? rows[0]),
      status
    }))
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('OnlineSupportWorkbench', () => {
  it('filters open, closed, all, keyword and admin-only unassigned queues', async () => {
    const rows = [
      conversation(),
      conversation({
        id: 12,
        user_id: 102,
        email: null,
        phone: '13800000002',
        assigned_agent_id: null,
        assigned_agent_code: null,
        status: 'closed',
        last_message_preview: '登录问题',
        staff_unread_count: 0
      }),
      conversation({
        id: 13,
        user_id: 103,
        email: 'user103@example.com',
        assigned_agent_id: null,
        assigned_agent_code: null,
        last_message_preview: '提现问题'
      })
    ];

    render(<OnlineSupportWorkbench api={supportApi(rows)} pollIntervalMs={0} scope="admin" />);

    expect(await screen.findByText('用户 101')).toBeInTheDocument();
    expect(screen.getByText('用户 103')).toBeInTheDocument();
    expect(screen.queryByText('用户 102')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '已关闭' }));
    expect(await screen.findByText('用户 102')).toBeInTheDocument();
    expect(screen.queryByText('用户 101')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '全部' }));
    expect(await screen.findByText('用户 101')).toBeInTheDocument();
    expect(screen.getByText('用户 102')).toBeInTheDocument();
    expect(screen.getAllByText('未分配').length).toBeGreaterThanOrEqual(1);

    fireEvent.change(screen.getByRole('textbox', { name: '搜索会话' }), {
      target: { value: '提现' }
    });
    fireEvent.click(screen.getByRole('button', { name: '搜索' }));
    expect(await screen.findByText('用户 103')).toBeInTheDocument();
    expect(screen.queryByText('用户 101')).not.toBeInTheDocument();

    fireEvent.change(screen.getByRole('textbox', { name: '搜索会话' }), {
      target: { value: '' }
    });
    fireEvent.click(screen.getByRole('button', { name: '搜索' }));
    fireEvent.click(screen.getByRole('button', { name: '未分配客户' }));
    expect(await screen.findByText('用户 102')).toBeInTheDocument();
    expect(screen.getByText('用户 103')).toBeInTheDocument();
    expect(screen.queryByText('用户 101')).not.toBeInTheDocument();
  });

  it('keeps global and unassigned controls out of the exact-agent workbench', async () => {
    render(<OnlineSupportWorkbench api={supportApi([conversation()])} pollIntervalMs={0} scope="agent" />);

    expect(await screen.findByText('用户 101')).toBeInTheDocument();
    expect(screen.queryByRole('group', { name: '会话归属筛选' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '未分配客户' })).not.toBeInTheDocument();
  });

  it('reuses the same client message id when a failed reply is retried', async () => {
    const api = supportApi([conversation()]);
    vi.mocked(api.sendMessage)
      .mockRejectedValueOnce(new Error('网络中断'))
      .mockResolvedValueOnce({
        conversation: conversation(),
        message: { ...messages[0], sender_type: 'agent' },
        replayed: true
      });
    render(<OnlineSupportWorkbench api={api} pollIntervalMs={0} scope="agent" />);

    fireEvent.click(await screen.findByRole('button', { name: '打开用户 101 会话' }));
    const composer = await screen.findByRole('textbox', { name: '回复内容' });
    fireEvent.change(composer, { target: { value: '正在为您核实' } });
    fireEvent.click(screen.getByRole('button', { name: '发送回复' }));

    expect(await screen.findByText(/回复发送失败：网络中断/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重试发送' }));

    await waitFor(() => expect(api.sendMessage).toHaveBeenCalledTimes(2));
    const firstPayload = vi.mocked(api.sendMessage).mock.calls[0][1];
    const retryPayload = vi.mocked(api.sendMessage).mock.calls[1][1];
    expect(firstPayload.body).toBe('正在为您核实');
    expect(retryPayload.body).toBe(firstPayload.body);
    expect(retryPayload.client_message_id).toBe(firstPayload.client_message_id);
    expect(firstPayload.client_message_id).toMatch(/^[A-Za-z0-9-]{8,64}$/);
  });

  it('marks the selected conversation read and changes its status', async () => {
    const api = supportApi([conversation({ staff_unread_count: 3 })]);
    render(<OnlineSupportWorkbench api={api} pollIntervalMs={0} scope="admin" />);

    fireEvent.click(await screen.findByRole('button', { name: '打开用户 101 会话' }));
    fireEvent.click(await screen.findByRole('button', { name: '标记已读' }));
    await waitFor(() => expect(api.markRead).toHaveBeenCalledWith(11, 501));

    fireEvent.click(screen.getByRole('button', { name: '关闭会话' }));
    await waitFor(() => expect(api.setStatus).toHaveBeenCalledWith(11, 'closed'));
  });

  it('cleans up the REST reconciliation interval on unmount', async () => {
    const api = supportApi([conversation()]);
    const setIntervalSpy = vi.spyOn(window, 'setInterval');
    const clearIntervalSpy = vi.spyOn(window, 'clearInterval');
    const { unmount } = render(
      <OnlineSupportWorkbench api={api} pollIntervalMs={60_000} scope="agent" />
    );

    expect(await screen.findByText('用户 101')).toBeInTheDocument();
    const supportIntervalIndex = setIntervalSpy.mock.calls.findIndex(([, delay]) => delay === 60_000);
    expect(supportIntervalIndex).toBeGreaterThanOrEqual(0);
    const intervalId = setIntervalSpy.mock.results[supportIntervalIndex].value as number;

    unmount();

    expect(clearIntervalSpy).toHaveBeenCalledWith(intervalId);
  });

  it('drives queue page two from the backend total and resets filters to page one', async () => {
    const api = supportApi([conversation()]);
    vi.mocked(api.listConversations).mockResolvedValue({
      conversations: [conversation()],
      total: 25
    });
    render(<OnlineSupportWorkbench api={api} pollIntervalMs={0} scope="admin" />);

    expect(await screen.findByText('用户 101')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(api.listConversations).toHaveBeenCalledWith(expect.objectContaining({
      limit: 10,
      offset: 10,
      status: 'open'
    })));

    fireEvent.click(screen.getByRole('button', { name: '已关闭' }));
    await waitFor(() => expect(vi.mocked(api.listConversations).mock.calls.at(-1)?.[0]).toEqual(
      expect.objectContaining({ limit: 10, offset: 0, status: 'closed' })
    ));
    expect(screen.getByText('当前页关键词')).toBeInTheDocument();
  });

  it('loads older messages with bounded cursors, retry state, dedupe and ascending order', async () => {
    const api = supportApi([conversation()]);
    const boundary = { ...messages[0], id: 502, body: '分页边界消息', created_at: 3_000 };
    const newest = { ...messages[0], id: 503, body: '最新消息', created_at: 4_000 };
    const oldest = { ...messages[0], id: 501, body: '更早消息', created_at: 2_000 };
    let rejectOlder: ((error: Error) => void) | undefined;
    const delayedFailure = new Promise<never>((_resolve, reject) => {
      rejectOlder = reject;
    });
    vi.mocked(api.getMessages)
      .mockResolvedValueOnce({
        has_more: true,
        messages: [boundary, newest],
        next_before_id: 502
      })
      .mockImplementationOnce(() => delayedFailure)
      .mockResolvedValueOnce({
        has_more: false,
        messages: [oldest, boundary],
        next_before_id: null
      });
    render(<OnlineSupportWorkbench api={api} pollIntervalMs={0} scope="agent" />);

    fireEvent.click(await screen.findByRole('button', { name: '打开用户 101 会话' }));
    const loadOlder = await screen.findByRole('button', { name: '加载更早消息' });
    fireEvent.click(loadOlder);
    expect(loadOlder).toBeDisabled();
    rejectOlder?.(new Error('历史网络中断'));

    expect(await screen.findByText(/更早消息加载失败：历史网络中断/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重试加载更早消息' }));
    expect(await screen.findByText('更早消息')).toBeInTheDocument();
    await waitFor(() => expect(api.getMessages).toHaveBeenCalledTimes(3));
    expect(vi.mocked(api.getMessages).mock.calls[1]).toEqual([11, { before_id: 502, limit: 100 }]);
    expect(vi.mocked(api.getMessages).mock.calls[2]).toEqual([11, { before_id: 502, limit: 100 }]);
    expect(screen.getAllByText('分页边界消息')).toHaveLength(1);
    expect(
      [...document.querySelectorAll('.support-message p')].map((node) => node.textContent)
    ).toEqual(['更早消息', '分页边界消息', '最新消息']);
  });
});

describe('support queue presentation helpers', () => {
  it('shows the exact owning agent or the unassigned label', () => {
    expect(supportAssignmentLabel(conversation())).toBe('AG-31（ID 31）');
    expect(supportAssignmentLabel(conversation({ assigned_agent_id: null, assigned_agent_code: null }))).toBe('未分配');
  });

  it('applies status, keyword and assignment filters without rewriting DTOs', () => {
    const rows = [
      conversation(),
      conversation({ id: 12, status: 'closed', assigned_agent_id: null, last_message_preview: '需要重置密码' })
    ];

    expect(filterSupportConversations(rows, 'open', '', 'all').map((row) => row.id)).toEqual([11]);
    expect(filterSupportConversations(rows, 'all', '重置密码', 'all').map((row) => row.id)).toEqual([12]);
    expect(filterSupportConversations(rows, 'all', '', 'unassigned').map((row) => row.id)).toEqual([12]);
  });

  it.each<SupportConversationStatus>(['open', 'closed'])('retains the backend %s status in fixtures', (status) => {
    expect(conversation({ status }).status).toBe(status);
  });
});
