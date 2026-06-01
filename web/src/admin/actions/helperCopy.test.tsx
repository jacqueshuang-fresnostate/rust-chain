import { render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

describe('Admin action helper copy', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('does not render static helper copy on convert rule actions', () => {
    render(<ConvertRuleActions />);

    expect(screen.getByText('新币闪兑规则')).toBeInTheDocument();
    expect(screen.getByText('新增或更新固定汇率')).toBeInTheDocument();
    expect(screen.queryByText('通过 POST upsert 固定汇率规则；本页面不创建 GET 列表请求。')).not.toBeInTheDocument();
    expect(screen.queryByText('后端仅允许 rate_source=fixed，重复交易对会更新现有规则。')).not.toBeInTheDocument();
  });

  it('does not render static helper copy on new coin actions', () => {
    render(<NewCoinActions />);

    expect(screen.getByText('新币生命周期动作')).toBeInTheDocument();
    expect(screen.getByText('生命周期流转')).toBeInTheDocument();
    expect(screen.queryByText('覆盖生命周期流转、后台派发、解禁规则和矿工费规则更新。')).not.toBeInTheDocument();
    expect(screen.queryByText('按后端顺序推进 preheat → subscription → distribution → listed。')).not.toBeInTheDocument();
    expect(screen.queryByText('项目必须处于 distribution 阶段，幂等键用于避免重复派发。')).not.toBeInTheDocument();
    expect(screen.queryByText('时间字段按 Unix milliseconds 输入，relative_period 使用秒数。')).not.toBeInTheDocument();
    expect(screen.queryByText('启用矿工费时需提供费率、计费依据和费用资产。')).not.toBeInTheDocument();
  });

  it('does not render static helper copy on market strategy actions', () => {
    render(<MarketStrategyActions />);

    expect(screen.getByText('行情策略动作')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '创建策略' })).toBeInTheDocument();
    expect(screen.queryByText('创建 internal/strategy 交易对策略并控制策略运行状态。')).not.toBeInTheDocument();
    expect(screen.queryByText('开始和结束时间均使用 Unix milliseconds。')).not.toBeInTheDocument();
    expect(screen.queryByText('支持 draft、active、paused、disabled。')).not.toBeInTheDocument();
  });

  it('does not render static helper copy on agent management actions', () => {
    render(<AgentManagementPage />);

    expect(screen.getByText('代理管理')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '创建代理' })).toBeInTheDocument();
    expect(screen.queryByText('创建代理账号并调整代理状态；所有变更都必须填写操作原因。')).not.toBeInTheDocument();
    expect(screen.queryByText('绑定已存在用户，创建代理编号和代理后台账号。')).not.toBeInTheDocument();
    expect(screen.queryByText('支持 active、suspended、disabled。')).not.toBeInTheDocument();
  });
});
