import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  getAgentUserAssets,
  getAgentUserMarginPositions,
  getAgentUserMarginOrders,
  getAgentUserSpotOrders,
  getAgentUserSecondsContractOrders
} from '../api/agent';
import { AgentUserPortfolioPage } from './UserPortfolioPage';
import { authStore } from '../auth/authStore';

vi.mock('../api/agent', () => ({
  getAgentUserAssets: vi.fn(),
  getAgentUserMarginPositions: vi.fn(),
  getAgentUserMarginOrders: vi.fn(),
  getAgentUserSpotOrders: vi.fn(),
  getAgentUserSecondsContractOrders: vi.fn()
}));

const getAssetsMock = vi.mocked(getAgentUserAssets);
const getMarginMock = vi.mocked(getAgentUserMarginPositions);
const getMarginOrdersMock = vi.mocked(getAgentUserMarginOrders);
const getSpotMock = vi.mocked(getAgentUserSpotOrders);
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

async function selectSemiOption(optionLabel: string, panel = 'seconds') {
  const user = userEvent.setup();
  const select = document.querySelector(`#semiTabPanel${panel} .semi-select`) as HTMLElement | null;
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
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    getMarginOrdersMock.mockImplementation(async () => ({
      orders: (await getMarginMock(42)).positions.map((row) => ({ ...row, order_type: 'limit', entry_price: null, limit_price: '90' })),
      total: 1
    }));
    getSpotMock.mockResolvedValue({
      orders: [{
        id: 8, user_id: 42, pair_id: 5, symbol: 'ETH-USDT', side: 'buy', order_type: 'stop_limit',
        price: '100.123456789012345678', trigger_price: '101', quantity: '2', filled_quantity: '1',
        status: 'partially_filled', created_at: now, updated_at: now
      }],
      total: 1
    });
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

    await user.click(screen.getByRole('tab', { name: '杠杆持仓' }));
    await waitFor(() => expect(getMarginMock).toHaveBeenCalledWith(42, { limit: 20, offset: 0, status: undefined }));
    expect(await screen.findByText('做多')).toBeInTheDocument();
    expect(screen.getByText('持仓中')).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: '钱包账户' }));
    await user.click(screen.getByRole('tab', { name: '杠杆持仓' }));
    await waitFor(() => expect(getMarginMock).toHaveBeenCalledTimes(1));

    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    await waitFor(() => expect(getSecondsMock).toHaveBeenCalledWith(42, { limit: 20, offset: 0, status: undefined }));
    expect(await screen.findByText('进行中')).toBeInTheDocument();
    expect(document.querySelector('#semiTabPanelseconds')?.textContent).not.toContain('持仓中');
    expect(getMarginOrdersMock).not.toHaveBeenCalled();
    expect(getSpotMock).not.toHaveBeenCalled();
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

  it('shows pending margin orders separately and filters them without querying positions', async () => {
    const row = (await getMarginMock(42)).positions[0];
    getMarginMock.mockClear();
    getMarginOrdersMock.mockResolvedValue({
      orders: [{ ...row, entry_price: null, order_type: 'limit', limit_price: '90' }], total: 1
    });
    const user = userEvent.setup();
    renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '杠杆订单' }));
    expect(await screen.findByText('待成交')).toBeInTheDocument();
    expect(screen.queryByText('持仓中')).not.toBeInTheDocument();
    await selectSemiOption('待成交', 'margin-orders');
    await waitFor(() => expect(getMarginOrdersMock).toHaveBeenLastCalledWith(42, { limit: 20, offset: 0, status: 'pending' }));
    expect(getMarginMock).not.toHaveBeenCalled();
  });

  it('paginates and filters spot history then refreshes only the active query', async () => {
    const user = userEvent.setup();
    getSpotMock.mockResolvedValue({ orders: [], total: 41 });
    renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '现货订单' }));
    await waitFor(() => expect(getSpotMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(getSpotMock).toHaveBeenLastCalledWith(42, { limit: 20, offset: 20, status: undefined }));
    await selectSemiOption('已成交', 'spot');
    await waitFor(() => expect(getSpotMock).toHaveBeenLastCalledWith(42, { limit: 20, offset: 0, status: 'filled' }));
    await user.click(screen.getByRole('button', { name: '刷新当前列表' }));
    await waitFor(() => expect(getSpotMock).toHaveBeenCalledTimes(4));
    expect(getAssetsMock).toHaveBeenCalledTimes(1);
    expect(getMarginMock).not.toHaveBeenCalled();
    expect(getMarginOrdersMock).not.toHaveBeenCalled();
    expect(getSecondsMock).not.toHaveBeenCalled();
  });

  it('renders partial spot fills and can retry failed queries', async () => {
    const user = userEvent.setup();
    getSpotMock.mockRejectedValueOnce(new Error('现货接口异常'));
    renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '现货订单' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('现货接口异常');
    await user.click(screen.getByRole('button', { name: '刷新当前列表' }));
    expect(await screen.findByText('部分成交')).toBeInTheDocument();
    expect(screen.getByText('止盈止损限价')).toBeInTheDocument();
    expect(screen.getByText('买入')).toBeInTheDocument();
    expect(screen.getByText('ETH-USDT')).toBeInTheDocument();
  });

  it('ignores a late response after switching filters', async () => {
    let resolveOld!: (value: { orders: []; total: number }) => void;
    getSpotMock.mockImplementationOnce(() => new Promise((resolve) => { resolveOld = resolve; }));
    const user = userEvent.setup();
    renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '现货订单' }));
    await waitFor(() => expect(getSpotMock).toHaveBeenCalledTimes(1));
    await selectSemiOption('部分成交', 'spot');
    expect(await screen.findByText('ETH-USDT')).toBeInTheDocument();
    await act(async () => resolveOld({ orders: [], total: 0 }));
    expect(screen.getByText('ETH-USDT')).toBeInTheDocument();
  });

  it('clears the financial cache when the agent session changes', async () => {
    renderPortfolio();
    await waitFor(() => expect(getAssetsMock).toHaveBeenCalledTimes(1));
    getAssetsMock.mockResolvedValue({ assets: [], total: 0 });
    act(() => { authStore.setSession({ scope: 'agent', subject: 'agent:other', accessToken: 'other', refreshToken: 'refresh' }); });
    await waitFor(() => expect(getAssetsMock).toHaveBeenCalledTimes(2));
    expect(await screen.findByText('暂无数据')).toBeInTheDocument();
    expect(screen.queryByRole('img', { name: 'USDT Logo' })).not.toBeInTheDocument();
  });

  it('counts down from the deadline, rounds up sub-seconds, and waits for backend settlement at expiry', async () => {
    vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
    vi.setSystemTime(now + 500);
    const user = userEvent.setup();
    const view = renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    const timer = await screen.findByRole('timer', { name: '订单 6 倒计时' });
    expect(timer).toHaveTextContent('01:00');
    await act(async () => vi.advanceTimersByTime(1000));
    expect(timer).toHaveTextContent('00:59');
    await act(async () => vi.advanceTimersByTime(59_000));
    expect(timer).toHaveTextContent('待结算');
    expect(screen.getByText('进行中')).toBeInTheDocument();
    expect(screen.queryByText('已结算')).not.toBeInTheDocument();
    expect(getSecondsMock).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
    view.unmount();
  });

  it('does not start countdowns for historical or manual-review orders', async () => {
    const fixture = await getSecondsMock(42);
    getSecondsMock.mockResolvedValue({
      orders: [
        { ...fixture.orders[0], status: 'settled', settled_at: now },
        { ...fixture.orders[0], id: 7, status: 'manual_review' }
      ],
      total: 2
    });
    vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
    vi.setSystemTime(now);
    const intervals = vi.spyOn(window, 'setInterval');
    const user = userEvent.setup();
    const view = renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    expect(await screen.findByText('已结算')).toBeInTheDocument();
    expect(screen.getByText('人工复核')).toBeInTheDocument();
    expect(screen.queryByRole('timer')).not.toBeInTheDocument();
    expect(intervals).not.toHaveBeenCalledWith(expect.any(Function), 1000);
    view.unmount();
  });

  it('clears the previous page timer and shows already-expired orders as awaiting settlement', async () => {
    const fixture = await getSecondsMock(42);
    getSecondsMock.mockClear();
    getSecondsMock
      .mockResolvedValueOnce({ ...fixture, total: 21 })
      .mockResolvedValueOnce({
        orders: [{ ...fixture.orders[0], id: 7, expires_at: now - 1000 }], total: 21
      });
    vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
    vi.setSystemTime(now);
    const user = userEvent.setup();
    const view = renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    expect(await screen.findByRole('timer', { name: '订单 6 倒计时' })).toHaveTextContent('01:00');
    await user.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByRole('timer', { name: '订单 7 倒计时' })).toHaveTextContent('待结算');
    expect(screen.queryByRole('timer', { name: '订单 6 倒计时' })).not.toBeInTheDocument();
    expect(getSecondsMock).toHaveBeenLastCalledWith(42, { limit: 20, offset: 20, status: undefined });
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('recalculates elapsed time after focus/visibility changes and cleans timers when leaving the tab', async () => {
    vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
    vi.setSystemTime(now);
    const user = userEvent.setup();
    const view = renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    expect(await screen.findByRole('timer')).toHaveTextContent('01:00');
    vi.setSystemTime(now + 15_000);
    fireEvent.focus(window);
    expect(screen.getByRole('timer')).toHaveTextContent('00:45');
    vi.setSystemTime(now + 30_000);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    fireEvent(document, new Event('visibilitychange'));
    expect(screen.getByRole('timer')).toHaveTextContent('00:30');
    await user.click(screen.getByRole('tab', { name: '钱包账户' }));
    expect(vi.getTimerCount()).toBe(0);
    vi.setSystemTime(now + 45_000);
    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    expect(await screen.findByRole('timer')).toHaveTextContent('00:15');
    expect(getSecondsMock).toHaveBeenCalledTimes(1);
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('supports long durations and removes the countdown when refreshed data is settled', async () => {
    const fixture = await getSecondsMock(42);
    getSecondsMock.mockClear();
    getSecondsMock.mockResolvedValue({ ...fixture, orders: [{ ...fixture.orders[0], expires_at: now + 3_661_000 }] });
    vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
    vi.setSystemTime(now);
    const user = userEvent.setup();
    const view = renderPortfolio();
    await user.click(screen.getByRole('tab', { name: '秒合约订单' }));
    expect(await screen.findByRole('timer')).toHaveTextContent('01:01:01');
    getSecondsMock.mockResolvedValue({ ...fixture, orders: [{ ...fixture.orders[0], status: 'settled', settled_at: now }] });
    await user.click(screen.getByRole('button', { name: '刷新当前列表' }));
    expect(await screen.findByText('已结算')).toBeInTheDocument();
    expect(screen.queryByRole('timer')).not.toBeInTheDocument();
    expect(vi.getTimerCount()).toBe(0);
    view.unmount();
  });
});
