import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { listAdminResource } from '../../api/adminResources';
import { apiRequest } from '../../api/client';
import { MarketStrategyActions } from './MarketStrategyActions';

vi.mock('../../api/adminResources', () => ({
  listAdminResource: vi.fn()
}));

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const listAdminResourceMock = vi.mocked(listAdminResource);
const apiRequestMock = vi.mocked(apiRequest);

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

describe('MarketStrategyActions', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    listAdminResourceMock.mockReset();
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue({});
    listAdminResourceMock.mockResolvedValue({
      rows: [
        {
          id: 91,
          pair_id: 21,
          symbol: 'BTC-USDT',
          strategy_type: 'price_path',
          start_price: '1.000000000000000000',
          target_price: '2.000000000000000000',
          status: 'paused',
          run_status: 'paused',
          created_at: 1_775_027_600_000
        }
      ],
      raw: { strategies: [] }
    });
  });

  it('renders strategy actions as a resource table page', async () => {
    render(<MarketStrategyActions />);

    expect(await screen.findByText('行情策略动作')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '创建策略' })).toBeInTheDocument();
    expect(screen.getByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '启用' })).toBeInTheDocument();
    expect(screen.queryByText('更新策略状态')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
  });
});
