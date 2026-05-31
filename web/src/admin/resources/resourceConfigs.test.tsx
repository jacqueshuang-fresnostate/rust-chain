import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listAdminResource } from '../../api/adminResources';
import { apiRequest } from '../../api/client';
import { ResourcePage, resourceConfigs } from './resourceConfigs';

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

const assetRows = [
  { id: 11, symbol: 'BTC', name: 'Bitcoin' },
  { id: 12, symbol: 'USDT', name: 'Tether' },
  { id: 22, symbol: 'ETH', name: 'Ethereum' },
  { id: 32, symbol: 'BNB', name: 'BNB' }
];

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function mockEmptyResource() {
  listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
    if (endpoint === '/admin/api/v1/assets') {
      return { rows: assetRows, raw: { [responseKey]: assetRows } };
    }

    return { rows: [], raw: {} };
  });
  apiRequestMock.mockResolvedValue({});
}

describe('resourceConfigs create actions', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    listAdminResourceMock.mockReset();
    apiRequestMock.mockReset();
    mockEmptyResource();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('opens an asset creation modal from the asset management page', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.assets} />);

    await user.click(await screen.findByRole('button', { name: '添加资产' }));
    const dialog = await screen.findByRole('dialog', { name: '添加资产' });
    await user.type(within(dialog).getByLabelText('资产符号'), 'btc');
    await user.type(within(dialog).getByLabelText('资产名称'), 'Bitcoin');
    await user.type(within(dialog).getByLabelText('资产精度'), '8');
    expect(within(dialog).getByRole('option', { name: '数字货币' })).toHaveValue('coin');
    expect(within(dialog).getByRole('option', { name: '稳定币' })).toHaveValue('stablecoin');
    expect(within(dialog).getByRole('option', { name: '法币' })).toHaveValue('fiat');
    expect(within(dialog).getByRole('option', { name: '平台币' })).toHaveValue('platform');
    await user.click(within(dialog).getByRole('button', { name: '提交添加资产' }));
    await user.type(screen.getByLabelText('操作原因'), 'add asset');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/assets', expect.objectContaining({ method: 'POST' }));
    });
  });

  it('opens a spot trading pair creation modal from the trading pair config page', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.marketPairs} />);

    await user.click(await screen.findByRole('button', { name: '添加交易对' }));
    const dialog = await screen.findByRole('dialog', { name: '添加现货交易对' });
    await user.selectOptions(within(dialog).getByLabelText('基础资产'), '11');
    await user.selectOptions(within(dialog).getByLabelText('计价资产'), '12');
    expect(within(dialog).getAllByRole('option', { name: 'BTC - Bitcoin（ID: 11）' })[0]).toHaveValue('11');
    expect(within(dialog).getAllByRole('option', { name: 'USDT - Tether（ID: 12）' })[0]).toHaveValue('12');
    expect(within(dialog).getByRole('option', { name: '外部行情' })).toHaveValue('external');
    expect(within(dialog).getByRole('option', { name: '内部撮合' })).toHaveValue('internal');
    expect(within(dialog).getByRole('option', { name: '策略行情' })).toHaveValue('strategy');
    await user.type(within(dialog).getByLabelText('交易对符号'), 'btc-usdt');
    await user.type(within(dialog).getByLabelText('价格精度'), '8');
    await user.type(within(dialog).getByLabelText('数量精度'), '6');
    await user.type(within(dialog).getByLabelText('最小下单额'), '10.000000000000000000');
    await user.click(within(dialog).getByRole('button', { name: '提交添加交易对' }));
    await user.type(screen.getByLabelText('操作原因'), 'add spot pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/market-pairs',
        expect.objectContaining({
          body: expect.stringContaining('"base_asset_id":11'),
          method: 'POST'
        })
      );
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/market-pairs',
        expect.objectContaining({ body: expect.stringContaining('"quote_asset_id":12') })
      );
    });
  });

  it('opens a margin trading pair creation modal from the margin product page', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.marginProducts} />);

    await user.click(await screen.findByRole('button', { name: '添加杠杆交易对' }));
    const dialog = await screen.findByRole('dialog', { name: '添加杠杆交易对' });
    await user.type(within(dialog).getByLabelText('杠杆交易对ID'), '21');
    await user.selectOptions(within(dialog).getByLabelText('保证金资产'), '22');
    expect(within(dialog).getByRole('option', { name: 'ETH - Ethereum（ID: 22）' })).toHaveValue('22');
    await user.type(within(dialog).getByLabelText('最大杠杆'), '5');
    await user.type(within(dialog).getByLabelText('最小保证金'), '100');
    await user.type(within(dialog).getByLabelText('最大保证金'), '10000');
    await user.type(within(dialog).getByLabelText('维持保证金率'), '0.1');
    await user.type(within(dialog).getByLabelText('小时利率'), '0.0001');
    await user.click(within(dialog).getByRole('button', { name: '提交添加杠杆交易对' }));
    await user.type(screen.getByLabelText('操作原因'), 'add margin pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/margin/products',
        expect.objectContaining({
          body: expect.stringContaining('"margin_asset":22'),
          method: 'POST'
        })
      );
    });
  });

  it('opens a seconds contract pair creation modal from the seconds product page', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.secondsProducts} />);

    await user.click(await screen.findByRole('button', { name: '添加秒合约交易对' }));
    const dialog = await screen.findByRole('dialog', { name: '添加秒合约交易对' });
    await user.type(within(dialog).getByLabelText('秒合约交易对ID'), '31');
    await user.selectOptions(within(dialog).getByLabelText('押注资产'), '32');
    expect(within(dialog).getByRole('option', { name: 'BNB - BNB（ID: 32）' })).toHaveValue('32');
    await user.type(within(dialog).getByLabelText('周期秒数'), '60');
    await user.type(within(dialog).getByLabelText('赔率'), '0.85');
    await user.type(within(dialog).getByLabelText('最小押注'), '10');
    await user.type(within(dialog).getByLabelText('最大押注'), '1000');
    await user.click(within(dialog).getByRole('button', { name: '提交添加秒合约交易对' }));
    await user.type(screen.getByLabelText('操作原因'), 'add seconds pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/seconds-contracts/products',
        expect.objectContaining({
          body: expect.stringContaining('"stake_asset":32'),
          method: 'POST'
        })
      );
    });
  });

  it('updates market pair status from row actions with a required reason', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/market-pairs') {
        const rows = [
          {
            id: 1,
            symbol: 'BTC-USDT',
            base_asset: 'BTC',
            quote_asset: 'USDT',
            price_precision: 8,
            qty_precision: 6,
            min_order_value: '10.0000',
            market_type: 'external',
            status: 'active'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    render(<ResourcePage config={resourceConfigs.marketPairs} />);

    expect(await screen.findByText('BTC-USDT')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable risky pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-pairs/1/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable risky pair' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/market-pairs')).toHaveLength(2);
    });
  });

  it('opens spot order details and cancels cancellable orders from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/spot/orders') {
        const rows = [
          {
            id: 7,
            user_id: 99,
            pair_id: 1,
            side: 'buy',
            order_type: 'limit',
            price: '100.0000',
            quantity: '2.0000',
            filled_quantity: '0.5000',
            status: 'open'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/spot/orders/7') {
        return { id: '7', status: 'open', detail: 'spot-order-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.spotOrders} />);

    expect(await screen.findByText('0.5000')).toBeInTheDocument();
    expect(screen.getByText('已成交数量')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/spot/orders/7');
    });
    expect(await screen.findByText(/"detail": "spot-order-detail"/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '管理员撤单' }));
    await user.type(screen.getByLabelText('操作原因'), 'risk cancel');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/spot/orders/7/cancel', {
        method: 'POST',
        body: JSON.stringify({ reason: 'risk cancel' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/spot/orders')).toHaveLength(2);
    });
  });

  it('opens margin product details and updates product status from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/margin/products') {
        const rows = [
          {
            id: 14,
            pair_id: 1,
            symbol: 'BTC-USDT',
            margin_asset_symbol: 'USDT',
            max_leverage: '5.00000000',
            min_margin: '10.0000',
            maintenance_margin_rate: '0.05000000',
            status: 'active'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/margin/products/14') {
        return { id: 14, detail: 'margin-product-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.marginProducts} />);

    expect(await screen.findByText('BTC-USDT')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/products/14');
    });
    expect(await screen.findByText(/"detail": "margin-product-detail"/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable margin product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/products/14/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable margin product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/margin/products')).toHaveLength(2);
    });
  });

  it('opens margin position details without unsafe write actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/margin/positions') {
        const rows = [
          {
            id: 21,
            user_id: 99,
            product_id: 14,
            direction: 'long',
            margin_amount: '100.0000',
            notional_amount: '500.0000',
            borrowed_amount: '400.0000',
            interest_amount: '1.2500',
            status: 'open'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/margin/positions/21') {
        return { id: 21, detail: 'margin-position-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.marginPositions} />);

    expect(await screen.findByText('400.0000')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '强平' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '关闭仓位' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '修改状态' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/positions/21');
    });
    expect(await screen.findByText(/"detail": "margin-position-detail"/)).toBeInTheDocument();
  });

  it('opens margin liquidation record details from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/margin/liquidations') {
        const rows = [
          {
            id: 31,
            position_id: 21,
            user_id: 99,
            mark_price: '84.0000',
            equity: '2.7500',
            interest_amount: '1.2500',
            payout_amount: '2.7500',
            reason: 'maintenance_margin'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/margin/liquidations/31') {
        return { id: 31, detail: 'margin-liquidation-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.marginLiquidations} />);

    expect(await screen.findByText('maintenance_margin')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/liquidations/31');
    });
    expect(await screen.findByText(/"detail": "margin-liquidation-detail"/)).toBeInTheDocument();
  });

  it('opens seconds contract product details and updates product status from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/seconds-contracts/products') {
        const rows = [
          {
            id: 41,
            pair_id: 1,
            symbol: 'ETH-USDT',
            stake_asset_symbol: 'USDT',
            duration_seconds: 60,
            payout_rate: '0.85000000',
            min_stake: '10.0000',
            status: 'disabled'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/seconds-contracts/products/41') {
        return { id: 41, detail: 'seconds-product-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.secondsProducts} />);

    expect(await screen.findByText('ETH-USDT')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/products/41');
    });
    expect(await screen.findByText(/"detail": "seconds-product-detail"/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '启用' }));
    await user.type(screen.getByLabelText('操作原因'), 'enable seconds product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/products/41/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'active', reason: 'enable seconds product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/seconds-contracts/products')).toHaveLength(2);
    });
  });

  it('opens seconds contract order details and settles open orders with a reason', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/seconds-contracts/orders') {
        const rows = [
          {
            id: 51,
            user_id: 99,
            product_id: 41,
            direction: 'up',
            stake_amount: '10.0000',
            entry_price: '100.0000',
            result: null,
            status: 'opened'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/seconds-contracts/orders/51') {
        return { id: 51, detail: 'seconds-order-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.secondsOrders} />);

    expect(await screen.findByText('10.0000')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/orders/51');
    });
    expect(await screen.findByText(/"detail": "seconds-order-detail"/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '结算赢' }));
    await user.type(screen.getByLabelText('操作原因'), 'manual settle win');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/orders/51/settle', {
        method: 'POST',
        body: JSON.stringify({ result: 'win', reason: 'manual settle win' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/seconds-contracts/orders')).toHaveLength(2);
    });
  });

  it('does not allow settlement actions for non-open seconds contract orders', async () => {
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/seconds-contracts/orders') {
        const rows = [
          {
            id: 52,
            user_id: 99,
            product_id: 41,
            direction: 'down',
            stake_amount: '12.0000',
            entry_price: '100.0000',
            result: 'loss',
            status: 'settled'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    render(<ResourcePage config={resourceConfigs.secondsOrders} />);

    expect(await screen.findByText('12.0000')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '结算赢' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '结算输' })).toBeDisabled();
  });

  it('opens earn product details and updates product status from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/earn/products') {
        const rows = [
          {
            id: 61,
            asset_id: 12,
            asset_symbol: 'USDT',
            name: 'USDT 30D',
            term_days: 30,
            apr_rate: '0.12000000',
            min_subscribe: '10.0000',
            status: 'active'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/earn/products/61') {
        return { id: 61, detail: 'earn-product-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.earnProducts} />);

    expect(await screen.findByText('USDT 30D')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products/61');
    });
    expect(await screen.findByText(/"detail": "earn-product-detail"/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable earn product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products/61/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable earn product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/products')).toHaveLength(2);
    });
  });

  it('opens earn subscription details without unsafe write actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/earn/subscriptions') {
        const rows = [
          {
            id: 62,
            user_id: 99,
            product_id: 61,
            asset_id: 12,
            amount: '100.0000',
            apr_rate: '0.12000000',
            term_days: 30,
            status: 'subscribed'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/earn/subscriptions/62') {
        return { id: 62, detail: 'earn-subscription-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.earnSubscriptions} />);

    expect(await screen.findByText('100.0000')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '管理员赎回' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '赎回' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '修改状态' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/subscriptions/62');
    });
    expect(await screen.findByText(/"detail": "earn-subscription-detail"/)).toBeInTheDocument();
  });

  it('opens convert pair details and updates pair enabled state from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/convert/pairs') {
        const rows = [
          {
            id: 71,
            from_asset_id: 11,
            to_asset_id: 12,
            pricing_mode: 'fixed',
            spread_rate: '0.01000000',
            min_amount: '1.0000',
            max_amount: '100.0000',
            enabled: true
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/convert/pairs/71') {
        return { id: 71, detail: 'convert-pair-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.convertPairs} />);

    expect(await screen.findByText('0.01000000')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/convert/pairs/71');
    });
    expect(await screen.findByText(/"detail": "convert-pair-detail"/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable convert pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/convert/pairs/71', {
        method: 'PATCH',
        body: JSON.stringify({ enabled: false, reason: 'disable convert pair' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/convert/pairs')).toHaveLength(2);
    });
  });

  it('opens convert order details without unsafe write actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/convert/orders') {
        const rows = [
          {
            id: 72,
            quote_id: 'quote-72',
            user_id: 99,
            convert_pair_id: 71,
            from_amount: '10.0000',
            to_amount: '20.0000',
            rate: '2.0000',
            status: 'pending'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/convert/orders/72') {
        return { id: 72, detail: 'convert-order-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.convertOrders} />);

    expect(await screen.findByText('quote-72')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '修改状态' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '取消订单' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '确认成交' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/convert/orders/72');
    });
    expect(await screen.findByText(/"detail": "convert-order-detail"/)).toBeInTheDocument();
  });

  it('exposes the spot trade fee column', () => {
    expect(resourceConfigs.spotTrades.columns).toEqual(
      expect.arrayContaining([expect.objectContaining({ key: 'fee', title: '手续费', type: 'amount' })])
    );
  });
});
