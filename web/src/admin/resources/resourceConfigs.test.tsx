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
});
