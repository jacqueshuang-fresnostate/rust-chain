import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  getAgentUserAssets,
  getAgentUserMarginPositions,
  getAgentUserSecondsContractOrders
} from '../api/agent';
import { AgentUserPortfolioPage } from './UserPortfolioPage';

vi.mock('../api/agent', () => ({
  getAgentUserAssets: vi.fn(),
  getAgentUserMarginPositions: vi.fn(),
  getAgentUserSecondsContractOrders: vi.fn()
}));

const getAssetsMock = vi.mocked(getAgentUserAssets);
const getMarginMock = vi.mocked(getAgentUserMarginPositions);
const getSecondsMock = vi.mocked(getAgentUserSecondsContractOrders);
const now = 1_735_732_800_000;

function renderPortfolio() {
  const router = createMemoryRouter(
    [{ path: '/agent/users/:userId/portfolio', element: <AgentUserPortfolioPage /> }],
    {
      initialEntries: [{
        pathname: '/agent/users/42/portfolio',
        state: { email: 'portfolio@example.test' }
      }]
    }
  );
  return render(<RouterProvider router={router} />);
}

async function selectSemiOption(optionLabel: string) {
  const user = userEvent.setup();
  const select = document.querySelector('#semiTabPanelseconds .semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  await user.click(select as HTMLElement);
  await waitFor(() => {
    expect([...document.querySelectorAll('.semi-select-option')].some((option) => option.textContent === optionLabel)).toBe(true);
  });
  const option = [...document.querySelectorAll('.semi-select-option')].find((item) => item.textContent === optionLabel) as HTMLElement;
  fireEvent.mouseDown(option);
  fireEvent.mouseUp(option);
  fireEvent.click(option);
}

describe('AgentUserPortfolioPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getAssetsMock.mockResolvedValue({
      assets: [{
        account_id: 1,
        account_type: 'spot',
        asset_id: 2,
        asset_symbol: 'USDT',
        logo_url: 'https://cdn.example.test/usdt.png',
        precision_scale: 18,
        available: '123.456789012345678901',
        frozen: '2.000000000000000000',
        locked: '3.000000000000000000',
        updated_at: now
      }],
      total: 1
    });
    getMarginMock.mockResolvedValue({
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
        opened_at: now,
        created_at: now,
        closed_at: null,
        status: 'opened'
      }],
      total: 1
    });
    getSecondsMock.mockResolvedValue({
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
        expires_at: now + 60_000,
        created_at: now,
        settled_at: null
      }],
      total: 1
    });
  });

  it('loads tabs on demand, keeps successful query caches, and includes opened orders by default', async () => {
    const user = userEvent.setup();
    renderPortfolio();

    expect(await screen.findByText('portfolio@example.test')).toBeInTheDocument();
    await waitFor(() => expect(getAssetsMock).toHaveBeenCalledWith(42, { limit: 20, offset: 0 }));
    expect(getMarginMock).not.toHaveBeenCalled();
    expect(getSecondsMock).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(document.querySelector('#semiTabPanelassets')?.textContent).toContain('123.46');
      expect(document.querySelector('#semiTabPanelassets')?.textContent).toContain('USDT');
    });
    expect(screen.getByRole('img', { name: 'USDT Logo' })).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: '杠杆仓位' }));
    await waitFor(() => expect(getMarginMock).toHaveBeenCalledWith(42, { limit: 20, offset: 0, status: undefined }));
    expect(await screen.findByText('做多')).toBeInTheDocument();
    expect(screen.getByText('持仓中')).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: '资产' }));
    await user.click(screen.getByRole('tab', { name: '杠杆仓位' }));
    await waitFor(() => expect(getMarginMock).toHaveBeenCalledTimes(1));

    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    await waitFor(() => expect(getSecondsMock).toHaveBeenCalledWith(42, { limit: 20, offset: 0, status: undefined }));
    expect(await screen.findByText('进行中')).toBeInTheDocument();
    expect(document.querySelector('#semiTabPanelseconds')?.textContent).not.toContain('持仓中');
    expect(screen.getByText('“全部状态”包含进行中订单。')).toBeInTheDocument();
  });

  it('uses server totals to request the next asset page without refreshing other tabs', async () => {
    const user = userEvent.setup();
    getAssetsMock.mockResolvedValue({ assets: [], total: 41 });
    renderPortfolio();
    await waitFor(() => expect(getAssetsMock).toHaveBeenCalledWith(42, { limit: 20, offset: 0 }));

    await user.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => expect(getAssetsMock).toHaveBeenLastCalledWith(42, { limit: 20, offset: 20 }));
    expect(getAssetsMock).toHaveBeenCalledTimes(2);
    expect(getMarginMock).not.toHaveBeenCalled();
    expect(getSecondsMock).not.toHaveBeenCalled();
  });

  it('refreshes only the seconds tab when its Chinese status filter changes', async () => {
    const user = userEvent.setup();
    renderPortfolio();
    await waitFor(() => expect(getAssetsMock).toHaveBeenCalledTimes(1));

    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    await waitFor(() => expect(getSecondsMock).toHaveBeenCalledTimes(1));
    await selectSemiOption('人工复核');

    await waitFor(() => {
      expect(getSecondsMock).toHaveBeenLastCalledWith(42, { limit: 20, offset: 0, status: 'manual_review' });
    });
    expect(getAssetsMock).toHaveBeenCalledTimes(1);
    expect(getMarginMock).not.toHaveBeenCalled();
  });

  it('renders empty and error states through the shared table contract', async () => {
    getAssetsMock.mockResolvedValueOnce({ assets: [], total: 0 });
    const first = renderPortfolio();
    expect(await screen.findByText('暂无数据')).toBeInTheDocument();
    first.unmount();

    getAssetsMock.mockRejectedValueOnce(new Error('资产接口异常'));
    renderPortfolio();
    expect(await screen.findByRole('alert')).toHaveTextContent('加载失败：资产接口异常');
  });
});
