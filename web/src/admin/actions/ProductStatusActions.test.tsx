import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ProductStatusActions } from './ProductStatusActions';
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

describe('ProductStatusActions', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue({});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('creates a spot trading pair through the Admin market pair endpoint', async () => {
    const user = userEvent.setup();
    render(<ProductStatusActions />);

    await user.type(screen.getByLabelText('基础资产ID'), '11');
    await user.type(screen.getByLabelText('计价资产ID'), '12');
    await user.type(screen.getByLabelText('交易对符号'), 'btc_usdt');
    await user.type(screen.getByLabelText('价格精度'), '8');
    await user.type(screen.getByLabelText('数量精度'), '6');
    await user.type(screen.getByLabelText('最小下单额'), '10.000000000000000000');
    await user.click(screen.getByRole('button', { name: '创建现货交易对' }));
    await user.type(screen.getByLabelText('操作原因'), 'create spot pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/market-pairs',
        expect.objectContaining({ method: 'POST' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/market-pairs')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      base_asset_id: 11,
      quote_asset_id: 12,
      symbol: 'btc_usdt',
      price_precision: 8,
      qty_precision: 6,
      min_order_value: '10.000000000000000000',
      status: 'active',
      market_type: 'external',
      reason: 'create spot pair'
    });
  });

  it('keeps spot pair creation disabled until precision fields are filled', async () => {
    const user = userEvent.setup();
    render(<ProductStatusActions />);

    await user.type(screen.getByLabelText('基础资产ID'), '11');
    await user.type(screen.getByLabelText('计价资产ID'), '12');
    await user.type(screen.getByLabelText('交易对符号'), 'btc-usdt');
    await user.type(screen.getByLabelText('最小下单额'), '10.000000000000000000');

    expect(screen.getByRole('button', { name: '创建现货交易对' })).toHaveAttribute('aria-disabled', 'true');
    expect(apiRequestMock).not.toHaveBeenCalled();
  });

  it('creates a margin product through the Admin margin product endpoint', async () => {
    const user = userEvent.setup();
    render(<ProductStatusActions />);

    await user.type(screen.getByLabelText('杠杆交易对ID'), '21');
    await user.type(screen.getByLabelText('保证金资产ID'), '22');
    await user.type(screen.getByLabelText('最大杠杆'), '5');
    await user.type(screen.getByLabelText('最小保证金'), '100');
    await user.type(screen.getByLabelText('最大保证金'), '10000');
    await user.type(screen.getByLabelText('维持保证金率'), '0.1');
    await user.type(screen.getByLabelText('小时利率'), '0.0001');
    await user.click(screen.getByRole('button', { name: '创建杠杆产品' }));
    await user.type(screen.getByLabelText('操作原因'), 'create margin product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/margin/products',
        expect.objectContaining({ method: 'POST' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/margin/products')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      pair_id: 21,
      margin_asset: 22,
      max_leverage: '5',
      min_margin: '100',
      max_margin: '10000',
      maintenance_margin_rate: '0.1',
      hourly_interest_rate: '0.0001',
      status: 'active',
      reason: 'create margin product'
    });
  });

  it('creates a seconds contract product through the Admin seconds product endpoint', async () => {
    const user = userEvent.setup();
    render(<ProductStatusActions />);

    await user.type(screen.getByLabelText('秒合约交易对ID'), '31');
    await user.type(screen.getByLabelText('押注资产ID'), '32');
    await user.type(screen.getByLabelText('周期秒数'), '60');
    await user.type(screen.getByLabelText('赔率'), '0.85');
    await user.type(screen.getByLabelText('最小押注'), '10');
    await user.type(screen.getByLabelText('最大押注'), '1000');
    await user.click(screen.getByRole('button', { name: '创建秒合约产品' }));
    await user.type(screen.getByLabelText('操作原因'), 'create seconds product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/seconds-contracts/products',
        expect.objectContaining({ method: 'POST' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/seconds-contracts/products')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      pair_id: 31,
      stake_asset: 32,
      duration_seconds: 60,
      payout_rate: '0.85',
      min_stake: '10',
      max_stake: '1000',
      status: 'active',
      reason: 'create seconds product'
    });
  });
});
