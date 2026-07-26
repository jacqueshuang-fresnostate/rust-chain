import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AgentManagementPage } from './AgentManagementPage';
import { apiRequest } from '../../api/client';

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const apiRequestMock = vi.mocked(apiRequest);

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

let installedResizeObserverMock = false;

const agentsResponse = {
  agents: [
    {
      id: 1,
      user_id: 100,
      email: 'root@example.test',
      agent_code: 'AGT-001',
      level: 1,
      parent_agent_id: null,
      parent_agent_code: null,
      root_agent_id: 1,
      root_agent_code: 'AGT-001',
      status: 'active',
      direct_user_count: 2,
      team_user_count: 5,
      child_agent_count: 1,
      admin_username: 'agent-admin',
      admin_status: 'active',
      created_at: 1_735_732_800_000
    },
    {
      id: 2,
      user_id: 101,
      email: 'child@example.test',
      agent_code: 'AGT-002',
      level: 2,
      parent_agent_id: 1,
      parent_agent_code: 'AGT-001',
      root_agent_id: 1,
      root_agent_code: 'AGT-001',
      status: 'active',
      direct_user_count: 1,
      team_user_count: 1,
      child_agent_count: 0,
      admin_username: 'agent-admin-2',
      admin_status: 'active',
      created_at: 1_735_732_900_000
    }
  ]
};

const agentUsersResponse = {
  users: [
    {
      user_id: 501,
      email: 'team@example.test',
      phone: null,
      status: 'active',
      kyc_level: 2,
      owner_agent_id: 1,
      root_agent_id: 1,
      owner_agent_code: 'AGT-001',
      owner_agent_level: 1,
      direct_inviter_id: 1,
      direct_inviter_type: 'agent',
      depth: 1,
      path: '1/501',
      referred_at: 1_735_732_800_000
    }
  ]
};

async function selectDrawerSemiOption(user: ReturnType<typeof userEvent.setup>, optionLabel: string) {
  const select = document.querySelector('.semi-sidesheet .semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  await user.click(select as HTMLElement);
  await waitFor(() => {
    expect([...document.querySelectorAll('.semi-select-option')].some((option) => option.textContent === optionLabel)).toBe(true);
  });
  const option = [...document.querySelectorAll('.semi-select-option')].find((item) => item.textContent === optionLabel) as HTMLElement | undefined;
  expect(option).toBeDefined();
  fireEvent.mouseEnter(option as HTMLElement);
  fireEvent.mouseDown(option as HTMLElement);
  fireEvent.mouseUp(option as HTMLElement);
  fireEvent.click(option as HTMLElement);
}

describe('AgentManagementPage', () => {
  beforeEach(() => {
    installedResizeObserverMock = !('ResizeObserver' in globalThis);
    if (installedResizeObserverMock) {
      Object.defineProperty(globalThis, 'ResizeObserver', {
        configurable: true,
        value: ResizeObserverMock
      });
    }
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/agents' && !init?.method) {
        return Promise.resolve(agentsResponse);
      }
      if (path === '/admin/api/v1/agents/1' && !init?.method) {
        return Promise.resolve(agentsResponse.agents[0]);
      }
      if (path === '/admin/api/v1/agents/1/users' && !init?.method) {
        return Promise.resolve(agentUsersResponse);
      }
      if (path === '/admin/api/v1/users/501/agent' && init?.method === 'PATCH') {
        return Promise.resolve({});
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    if (installedResizeObserverMock) {
      Reflect.deleteProperty(globalThis, 'ResizeObserver');
    }
  });

  it('shows only the active tab panel', async () => {
    render(<AgentManagementPage />);

    expect((await screen.findAllByText('AGT-001')).length).toBeGreaterThan(0);
    expect(screen.getByRole('heading', { name: '代理列表' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: '创建代理' })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: '创建代理' }));

    expect(await screen.findByRole('heading', { name: '创建代理' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: '代理列表' })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: '代理列表' }));

    expect(await screen.findByRole('heading', { name: '代理列表' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: '创建代理' })).not.toBeInTheDocument();
  });

  it('consolidates detail into a single button with a collapsible raw data section', async () => {
    const user = userEvent.setup();
    render(<AgentManagementPage />);

    expect((await screen.findAllByText('AGT-001')).length).toBeGreaterThan(0);
    expect(screen.queryByRole('button', { name: '查看详情' })).not.toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '详情' })).toHaveLength(2);

    await user.click(screen.getAllByRole('button', { name: '详情' })[0]);

    expect(await screen.findByText('team@example.test')).toBeInTheDocument();
    expect(screen.getByText('原始数据')).toBeInTheDocument();
    expect(screen.queryByText('字段')).not.toBeInTheDocument();

    await user.click(screen.getByText('原始数据'));

    expect(await screen.findByText('字段')).toBeInTheDocument();
    expect(screen.getByText('内容')).toBeInTheDocument();
  });

  it('opens agent detail drawer and reassigns a user to another agent', async () => {
    const user = userEvent.setup();
    render(<AgentManagementPage />);

    expect((await screen.findAllByText('AGT-001')).length).toBeGreaterThan(0);
    await user.click(screen.getAllByRole('button', { name: '详情' })[0]);

    expect(await screen.findByText('team@example.test')).toBeInTheDocument();
    expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agents/1');
    expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agents/1/users');

    await selectDrawerSemiOption(user, 'AGT-002（L2）');
    await user.click(screen.getByRole('button', { name: '转移' }));
    await user.click(await screen.findByRole('button', { name: '确认转移' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/users/501/agent', {
        method: 'PATCH',
        body: JSON.stringify({ agent_id: 2 })
      });
    });
    await waitFor(() => {
      expect(apiRequestMock.mock.calls.filter(([path]) => path === '/admin/api/v1/agents/1/users')).toHaveLength(2);
      expect(apiRequestMock.mock.calls.filter(([path]) => path === '/admin/api/v1/agents')).toHaveLength(2);
    });
  });
});
