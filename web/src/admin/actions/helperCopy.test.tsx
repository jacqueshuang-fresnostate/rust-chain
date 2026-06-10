import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import { AgentManagementPage } from './AgentManagementPage';
import { ConvertRuleActions } from './ConvertRuleActions';
import { MarketStrategyActions } from './MarketStrategyActions';
import { NewCoinActions } from './NewCoinActions';

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

function semiSelectByLabel(label: string): HTMLElement {
  const labelNode = screen.getByText(label).closest('label') as HTMLElement | null;
  expect(labelNode).toBeInTheDocument();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

describe('Admin action helper copy', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/agents') {
        return {
          agents: [
            {
              id: 42,
              user_id: 1001,
              email: 'agent@example.com',
              agent_code: 'AGT-42',
              level: 1,
              status: 'active',
              admin_username: 'agent-admin',
              admin_status: 'active',
              created_at: 1_775_027_600_000
            }
          ]
        };
      }
      if (path === '/admin/api/v1/agents/42') {
        return { id: 42, agent_code: 'AGT-42', detail: 'agent-detail' };
      }

      return {};
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('uses Semi controls and omits static helper copy on convert rule actions', () => {
    render(<ConvertRuleActions />);

    expect(screen.getByText('新币闪兑规则')).toBeInTheDocument();
    expect(screen.getByText('新增或更新固定汇率')).toBeInTheDocument();
    expect(screen.getByLabelText('闪兑交易对ID').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('固定汇率').closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('状态');
    expect(screen.queryByText('通过 POST upsert 固定汇率规则；本页面不创建 GET 列表请求。')).not.toBeInTheDocument();
    expect(screen.queryByText('后端仅允许 rate_source=fixed，重复交易对会更新现有规则。')).not.toBeInTheDocument();
  });

  it('uses Semi controls and omits static helper copy on new coin actions', () => {
    render(<NewCoinActions />);

    expect(screen.getByText('新币生命周期动作')).toBeInTheDocument();
    expect(screen.getByText('生命周期流转')).toBeInTheDocument();
    expect(screen.getAllByLabelText('项目ID')[0].closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('目标阶段');
    semiSelectByLabel('解禁类型');
    expect(screen.getByRole('checkbox', { name: '启用矿工费' })).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: '启用矿工费' }).closest('.semi-checkbox')).toBeInTheDocument();
    semiSelectByLabel('计费依据');
    expect(screen.queryByText('覆盖生命周期流转、后台派发、解禁规则和矿工费规则更新。')).not.toBeInTheDocument();
    expect(screen.queryByText('按后端顺序推进 preheat → subscription → distribution → listed。')).not.toBeInTheDocument();
    expect(screen.queryByText('项目必须处于 distribution 阶段，幂等键用于避免重复派发。')).not.toBeInTheDocument();
    expect(screen.queryByText('时间字段按 Unix milliseconds 输入，relative_period 使用秒数。')).not.toBeInTheDocument();
    expect(screen.queryByText('启用矿工费时需提供费率、计费依据和费用资产。')).not.toBeInTheDocument();
  });

  it('does not render static helper copy on market strategy actions', () => {
    render(<MarketStrategyActions />);

    expect(screen.getByText('行情策略动作')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '创建策略' })).toBeInTheDocument();
    expect(screen.queryByText('创建 internal/strategy 交易对策略并控制策略运行状态。')).not.toBeInTheDocument();
    expect(screen.queryByText('开始和结束时间均使用 Unix milliseconds。')).not.toBeInTheDocument();
    expect(screen.queryByText('支持 draft、active、paused、disabled。')).not.toBeInTheDocument();
  });

  it('uses an agent list, initial password, and row status actions on agent management actions', async () => {
    const user = userEvent.setup();
    render(<AgentManagementPage />);

    expect(screen.getByText('代理管理')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '创建代理' })).toBeInTheDocument();
    expect(screen.getByLabelText('用户ID').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('代理编号').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('代理后台账号').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('初始密码').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.queryByLabelText('密码哈希')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: '更新代理状态' })).not.toBeInTheDocument();
    expect(await screen.findByText('AGT-42')).toBeInTheDocument();
    expect(screen.getByText('agent@example.com')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '暂停' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '禁用' })).toBeInTheDocument();

    await user.type(screen.getByLabelText('用户ID'), '1001');
    await user.type(screen.getByLabelText('代理编号'), 'AGT-NEW');
    await user.type(screen.getByLabelText('代理后台账号'), 'agent-new');
    await user.type(screen.getByLabelText('初始密码'), 'Password123!');
    await user.click(screen.getByRole('button', { name: '创建代理' }));
    await user.type(screen.getByLabelText('操作原因'), 'create agent');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agents', expect.objectContaining({ method: 'POST' }));
    });
    const createRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/agents' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(createRequest?.body))).toEqual({
      user_id: 1001,
      agent_code: 'AGT-NEW',
      admin_username: 'agent-new',
      admin_password: 'Password123!',
      level: 1,
      reason: 'create agent'
    });
    expect(JSON.parse(String(createRequest?.body))).not.toHaveProperty('admin_password_hash');

    await user.click(screen.getByRole('button', { name: '暂停' }));
    await user.type(screen.getByLabelText('操作原因'), 'suspend agent');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agents/42/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'suspended', reason: 'suspend agent' })
      });
    });
    expect(screen.queryByText('创建代理账号并调整代理状态；所有变更都必须填写操作原因。')).not.toBeInTheDocument();
    expect(screen.queryByText('绑定已存在用户，创建代理编号和代理后台账号。')).not.toBeInTheDocument();
    expect(screen.queryByText('支持 active、suspended、disabled。')).not.toBeInTheDocument();
  });
});
