import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StrictMode } from 'react';
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

async function expectFormattedDetail(value: string, rawJson: RegExp) {
  expect(await screen.findByText(value)).toBeInTheDocument();
  expect(screen.queryByText(rawJson)).not.toBeInTheDocument();
}

function expectCreateModalSize(dialog: HTMLElement, size: 'medium' | 'wide' | 'extra-wide') {
  const modal = dialog.closest('.admin-create-modal') as HTMLElement | null;
  expect(modal).toBeInTheDocument();
  expect(modal).toHaveClass(`admin-create-modal-${size}`);
}

function semiSelectByLabel(root: HTMLElement, label: string): HTMLElement {
  const labelNode = [...root.querySelectorAll('label')].find((item) => item.textContent?.trim().startsWith(label) && item.querySelector('.semi-select')) as HTMLElement | undefined;
  expect(labelNode).toBeDefined();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

function semiInputByLabel(dialog: HTMLElement, label: string, index = 0): HTMLElement {
  const input = within(dialog).getAllByLabelText(label)[index] as HTMLElement;
  const wrapper = input.closest('.semi-input-wrapper') as HTMLElement | null;
  expect(wrapper).toBeInTheDocument();
  return wrapper as HTMLElement;
}

async function selectSemiOption(user: ReturnType<typeof userEvent.setup>, dialog: HTMLElement, label: string, optionLabel: string) {
  await user.click(semiSelectByLabel(dialog, label));
  await waitFor(() => {
    expect([...document.querySelectorAll('.semi-select-option')].some((option) => option.textContent === optionLabel)).toBe(true);
  });
  const options = [...document.querySelectorAll('.semi-select-option')].filter((item) => item.textContent === optionLabel) as HTMLElement[];
  const option = options.at(-1);
  expect(option).toBeDefined();
  fireEvent.mouseEnter(option as HTMLElement);
  fireEvent.mouseDown(option as HTMLElement);
  fireEvent.mouseUp(option as HTMLElement);
  fireEvent.click(option as HTMLElement);
  await waitFor(() => {
    expect(semiSelectByLabel(dialog, label)).toHaveTextContent(optionLabel);
  });
}

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

class WebSocketMock {
  readonly url: string;
  closed = false;
  onmessage: ((event: MessageEvent<string>) => void) | null = null;
  private listeners: Array<(event: MessageEvent<string>) => void> = [];

  constructor(url: string) {
    this.url = url;
  }

  addEventListener(type: string, listener: (event: MessageEvent<string>) => void) {
    if (type === 'message') {
      this.listeners.push(listener);
    }
  }

  removeEventListener(type: string, listener: (event: MessageEvent<string>) => void) {
    if (type === 'message') {
      this.listeners = this.listeners.filter((item) => item !== listener);
    }
  }

  close() {
    this.closed = true;
  }

  emitMessage(payload: unknown) {
    const event = { data: JSON.stringify(payload) } as MessageEvent<string>;
    this.onmessage?.(event);
    this.listeners.forEach((listener) => listener(event));
  }
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
  it('adds an email filter beside every user ID filter', () => {
    const configsWithoutEmail = Object.entries(resourceConfigs)
      .filter(([, config]) => config.filters?.some((filter) => filter.key === 'user_id'))
      .filter(([, config]) => !config.filters?.some((filter) => filter.key === 'email'))
      .map(([key]) => key);

    expect(configsWithoutEmail).toEqual([]);
  });

  it('keeps the user ID column visible on user management', () => {
    expect(resourceConfigs.users.columns).toContainEqual({ key: 'id', title: '用户ID' });
  });

  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    vi.stubGlobal('WebSocket', undefined);
    Object.defineProperty(window, 'WebSocket', { configurable: true, value: undefined });
    listAdminResourceMock.mockReset();
    apiRequestMock.mockReset();
    mockEmptyResource();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('uses dropdown filters, localized type labels, details, and safe edits on assets', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        const rows = [
          {
            id: 11,
            symbol: 'BTC',
            name: 'Bitcoin',
            precision_scale: 8,
            asset_type: 'coin',
            status: 'active'
          },
          {
            id: 12,
            symbol: 'USDT',
            name: 'Tether',
            precision_scale: 6,
            asset_type: 'stablecoin',
            status: 'disabled'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/assets/11') {
        return { id: 11, symbol: 'BTC', detail: 'asset-detail' };
      }

      return {};
    });
    render(<ResourcePage config={resourceConfigs.assets} />);

    expect(await screen.findByText('BTC', { selector: 'span' })).toBeInTheDocument();
    semiSelectByLabel(document.body, '资产类型');
    semiSelectByLabel(document.body, '状态');
    expect(screen.getByText('数字货币', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('稳定币', { selector: 'span' })).toBeInTheDocument();
    expect(screen.queryByText('coin')).not.toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '查看详情' })).toHaveLength(2);
    expect(screen.getAllByRole('button', { name: '修改' })).toHaveLength(2);

    await selectSemiOption(user, document.body, '资产类型', '稳定币');
    await selectSemiOption(user, document.body, '状态', '禁用');
    await user.click(screen.getByRole('button', { name: '查询' }));

    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/api/v1/assets', 'assets', {
        asset_type: 'stablecoin',
        status: 'disabled'
      });
    });

    await user.click(screen.getAllByRole('button', { name: '查看详情' })[0]);
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/assets/11');
    });
    await expectFormattedDetail('asset-detail', /"detail": "asset-detail"/);

    await user.click(screen.getAllByRole('button', { name: '修改' })[0]);
    const editDialog = await screen.findByRole('dialog', { name: '修改资产配置' });
    await user.clear(within(editDialog).getByLabelText('资产名称'));
    await user.type(within(editDialog).getByLabelText('资产名称'), 'Bitcoin Updated');
    await user.clear(within(editDialog).getByLabelText('资产精度'));
    await user.type(within(editDialog).getByLabelText('资产精度'), '6');
    await selectSemiOption(user, editDialog, '资产类型', '稳定币');
    await selectSemiOption(user, editDialog, '状态', '禁用');
    await user.click(within(editDialog).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'update asset config');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/assets/11', expect.objectContaining({ method: 'PATCH' }));
    });
    const request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/assets/11' && init && 'method' in init)?.[1];
    expect(request).toBeDefined();
    const body = JSON.parse(String(request?.body));
    expect(body).toEqual({
      name: 'Bitcoin Updated',
      precision_scale: 6,
      asset_type: 'stablecoin',
      status: 'disabled',
      reason: 'update asset config'
    });
    expect(body).not.toHaveProperty('symbol');
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/assets')).toHaveLength(3);
  });

  it('opens an asset creation modal from the asset management page without static helper copy', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.assets} />);

    await user.click(await screen.findByRole('button', { name: '添加资产' }));
    const dialog = await screen.findByRole('dialog', { name: '添加资产' });
    expectCreateModalSize(dialog, 'medium');
    expect(within(dialog).queryByText('资产创建后可作为交易对、钱包账户、闪兑和产品配置的基础资产。')).not.toBeInTheDocument();
    semiInputByLabel(dialog, '资产符号');
    semiInputByLabel(dialog, '资产名称');
    semiInputByLabel(dialog, '资产精度');
    semiSelectByLabel(dialog, '资产类型');
    semiSelectByLabel(dialog, '初始状态');
    await user.type(within(dialog).getByLabelText('资产符号'), 'btc');
    await user.type(within(dialog).getByLabelText('资产名称'), 'Bitcoin');
    await user.type(within(dialog).getByLabelText('资产精度'), '8');
    await selectSemiOption(user, dialog, '资产类型', '稳定币');
    await user.click(within(dialog).getByRole('button', { name: '提交添加资产' }));
    await user.type(screen.getByLabelText('操作原因'), 'add asset');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/assets', expect.objectContaining({ method: 'POST' }));
    });
  });

  it('opens a spot trading pair creation modal from the trading pair config page without static helper copy', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.marketPairs} />);

    await user.click(await screen.findByRole('button', { name: '添加交易对' }));
    const dialog = await screen.findByRole('dialog', { name: '添加现货交易对' });
    expectCreateModalSize(dialog, 'wide');
    expect(within(dialog).queryByText('现货交易对创建后可被杠杆、秒合约产品复用。')).not.toBeInTheDocument();
    semiSelectByLabel(dialog, '基础资产');
    semiSelectByLabel(dialog, '计价资产');
    semiSelectByLabel(dialog, '初始状态');
    semiSelectByLabel(dialog, '市场类型');
    semiInputByLabel(dialog, '交易对符号');
    semiInputByLabel(dialog, '价格精度');
    semiInputByLabel(dialog, '数量精度');
    semiInputByLabel(dialog, '最小下单额');
    await selectSemiOption(user, dialog, '基础资产', 'BTC - Bitcoin（ID: 11）');
    await selectSemiOption(user, dialog, '计价资产', 'USDT - Tether（ID: 12）');
    await selectSemiOption(user, dialog, '市场类型', '策略行情');
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

  it('creates margin products with mode and leverage levels and renders configured levels', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/market-pairs') {
        const rows = [
          { id: 21, symbol: 'BTC-USDT', status: 'active' },
          { id: 22, symbol: 'ETH-USDT', status: 'active' }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      if (endpoint === '/admin/api/v1/margin/products') {
        const rows = [
          {
            id: 101,
            pair_id: 21,
            symbol: 'BTC-USDT',
            margin_asset_symbol: 'USDT',
            margin_mode: 'isolated',
            leverage_levels: ['2', '5', '10'],
            max_leverage: '10.00000000',
            min_margin: '100.000000000000000000',
            maintenance_margin_rate: '0.10000000',
            hourly_interest_rate: '0.00010000',
            status: 'active'
          },
          {
            id: 102,
            pair_id: 22,
            symbol: 'ETH-USDT',
            margin_asset_symbol: 'USDT',
            margin_mode: 'cross',
            leverage_levels: ['3', '7'],
            max_leverage: '7.00000000',
            min_margin: '50.000000000000000000',
            maintenance_margin_rate: '0.12000000',
            hourly_interest_rate: '0.00020000',
            status: 'disabled'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    render(<ResourcePage config={resourceConfigs.marginProducts} />);

    expect(await screen.findByText('逐仓', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('全仓', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('2.00x / 5.00x / 10.00x', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('3.00x / 7.00x', { selector: 'span' })).toBeInTheDocument();

    await user.click(await screen.findByRole('button', { name: '添加杠杆交易对' }));
    const dialog = await screen.findByRole('dialog', { name: '添加杠杆交易对' });
    expectCreateModalSize(dialog, 'extra-wide');
    expect(within(dialog).queryByLabelText('杠杆交易对ID')).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('最大杠杆')).not.toBeInTheDocument();
    semiSelectByLabel(dialog, '杠杆交易对');
    semiSelectByLabel(dialog, '保证金资产');
    semiSelectByLabel(dialog, '保证金模式');
    await selectSemiOption(user, dialog, '杠杆交易对', 'BTC-USDT（ID: 21）');
    await selectSemiOption(user, dialog, '保证金资产', 'ETH - Ethereum（ID: 22）');
    expect(within(dialog).getByText('杠杆档位')).toBeInTheDocument();
    await user.click(within(dialog).getByLabelText('2x'));
    await user.click(within(dialog).getByLabelText('5x'));
    await user.click(within(dialog).getByLabelText('10x'));
    semiInputByLabel(dialog, '自定义杠杆档位');
    semiInputByLabel(dialog, '最小保证金');
    semiInputByLabel(dialog, '最大保证金');
    semiInputByLabel(dialog, '维持保证金率');
    semiInputByLabel(dialog, '小时利率');
    semiSelectByLabel(dialog, '初始状态');
    await user.type(within(dialog).getByLabelText('自定义杠杆档位'), '25');
    await user.type(within(dialog).getByLabelText('最小保证金'), '100');
    await user.type(within(dialog).getByLabelText('最大保证金'), '10000');
    await user.type(within(dialog).getByLabelText('维持保证金率'), '0.1');
    await user.type(within(dialog).getByLabelText('小时利率'), '0.0001');
    await user.click(within(dialog).getByRole('button', { name: '提交添加杠杆交易对' }));
    await user.type(screen.getByLabelText('操作原因'), 'add margin pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/products', expect.objectContaining({ method: 'POST' }));
    });
    const request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/margin/products' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      pair_id: 21,
      margin_asset: 22,
      margin_mode: 'isolated',
      leverage_levels: ['2', '5', '10', '25'],
      max_leverage: '25',
      min_margin: '100',
      max_margin: '10000',
      maintenance_margin_rate: '0.1',
      hourly_interest_rate: '0.0001',
      status: 'active',
      reason: 'add margin pair'
    });
  });

  it('opens a seconds contract pair creation modal from the seconds product page', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.secondsProducts} />);

    await user.click(await screen.findByRole('button', { name: '添加秒合约交易对' }));
    const dialog = await screen.findByRole('dialog', { name: '添加秒合约交易对' });
    expectCreateModalSize(dialog, 'wide');
    semiInputByLabel(dialog, '秒合约交易对ID');
    semiSelectByLabel(dialog, '押注资产');
    semiInputByLabel(dialog, '周期秒数');
    semiInputByLabel(dialog, '赔率');
    semiInputByLabel(dialog, '最小押注');
    semiInputByLabel(dialog, '最大押注');
    semiSelectByLabel(dialog, '初始状态');
    await user.type(within(dialog).getByLabelText('秒合约交易对ID'), '31');
    await selectSemiOption(user, dialog, '押注资产', 'BNB - BNB（ID: 32）');
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

  it('creates convert pairs, risk rules, new coin projects, and user row actions', async () => {
    const user = userEvent.setup();
    const confirmWithReason = async (reason: string) => {
      const reasonInputs = await screen.findAllByLabelText('操作原因');
      await user.type(reasonInputs.at(-1)!, reason);
      const confirmButtons = screen.getAllByRole('button', { name: '确认' });
      await user.click(confirmButtons.at(-1)!);
    };

    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/convert/pairs') {
        const rows = [
          {
            id: 81,
            from_asset_id: 11,
            to_asset_id: 12,
            pricing_mode: 'fixed',
            spread_rate: '0.01000000',
            min_amount: '1.000000000000000000',
            max_amount: '100.000000000000000000',
            enabled: true
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      if (endpoint === '/admin/api/v1/risk/rules') {
        const rows = [
          {
            id: 91,
            rule_type: 'withdraw_limit',
            target_type: 'user',
            target_id: '123',
            enabled: true
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      if (endpoint === '/admin/api/v1/users') {
        const rows = [
          {
            id: 123,
            email: 'user@example.com',
            phone: '18800000000',
            status: 'active',
            kyc_level: 1
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/users/123') {
        return { id: 123, detail: 'user-detail' };
      }
      if (path === '/admin/api/v1/wallet/accounts?user_id=123&include_empty=true&limit=100') {
        return {
          accounts: [
            { asset_symbol: 'USDT', available: '10.0000', account_exists: true },
            { asset_symbol: 'BTC', available: '0.000000000000000000', frozen: '0.000000000000000000', locked: '0.000000000000000000', account_exists: false }
          ]
        };
      }
      return {};
    });

    const { unmount } = render(<ResourcePage config={resourceConfigs.convertPairs} />);
    await user.click(await screen.findByRole('button', { name: '添加闪兑交易对' }));
    let dialog = await screen.findByRole('dialog', { name: '添加闪兑交易对' });
    expectCreateModalSize(dialog, 'wide');
    semiSelectByLabel(dialog, '源资产');
    semiSelectByLabel(dialog, '目标资产');
    semiSelectByLabel(dialog, '定价模式');
    semiInputByLabel(dialog, '价差率');
    semiInputByLabel(dialog, '最小金额');
    semiInputByLabel(dialog, '最大金额');
    semiSelectByLabel(dialog, '启用');
    await selectSemiOption(user, dialog, '源资产', 'BTC - Bitcoin（ID: 11）');
    await selectSemiOption(user, dialog, '目标资产', 'USDT - Tether（ID: 12）');
    await user.type(within(dialog).getByLabelText('价差率'), '0.01');
    await user.type(within(dialog).getByLabelText('最小金额'), '1');
    await user.type(within(dialog).getByLabelText('最大金额'), '100');
    await user.click(within(dialog).getByRole('button', { name: '提交添加闪兑交易对' }));
    await confirmWithReason('create convert pair');

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/convert/pairs', expect.objectContaining({ method: 'POST' }));
    });
    let request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/convert/pairs' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      from_asset_id: 11,
      to_asset_id: 12,
      pricing_mode: 'fixed',
      spread_rate: '0.01',
      min_amount: '1',
      max_amount: '100',
      enabled: true,
      reason: 'create convert pair'
    });
    unmount();

    const riskPage = render(<ResourcePage config={resourceConfigs.riskRules} />);
    await user.click(await screen.findByRole('button', { name: '添加风控规则' }));
    dialog = await screen.findByRole('dialog', { name: '添加风控规则' });
    expectCreateModalSize(dialog, 'wide');
    semiInputByLabel(dialog, '规则类型');
    semiInputByLabel(dialog, '对象类型');
    semiInputByLabel(dialog, '对象ID');
    expect(within(dialog).getByLabelText('规则配置JSON').closest('.semi-input-textarea-wrapper')).toBeInTheDocument();
    semiSelectByLabel(dialog, '启用');
    await user.type(within(dialog).getByLabelText('规则类型'), 'withdraw_limit');
    await user.type(within(dialog).getByLabelText('对象类型'), 'user');
    await user.type(within(dialog).getByLabelText('对象ID'), '123');
    await user.clear(within(dialog).getByLabelText('规则配置JSON'));
    fireEvent.change(within(dialog).getByLabelText('规则配置JSON'), { target: { value: '{bad json' } });
    await user.click(within(dialog).getByRole('button', { name: '提交添加风控规则' }));
    await confirmWithReason('bad risk json');
    await waitFor(() => {
      expect(apiRequestMock.mock.calls.filter(([path]) => path === '/admin/api/v1/risk/rules')).toHaveLength(0);
    });
    await user.clear(within(dialog).getByLabelText('规则配置JSON'));
    fireEvent.change(within(dialog).getByLabelText('规则配置JSON'), { target: { value: '{"daily_limit":"1000"}' } });
    await user.click(within(dialog).getByRole('button', { name: '提交添加风控规则' }));
    await confirmWithReason('create risk rule');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/risk/rules', expect.objectContaining({ method: 'POST' }));
    });
    request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/risk/rules' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      rule_type: 'withdraw_limit',
      target_type: 'user',
      target_id: '123',
      config_json: { daily_limit: '1000' },
      enabled: true,
      reason: 'create risk rule'
    });
    await user.click(screen.getByRole('button', { name: '禁用' }));
    await confirmWithReason('disable risk rule');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/risk/rules/91/status', {
        method: 'PATCH',
        body: JSON.stringify({ enabled: false, reason: 'disable risk rule' })
      });
    });
    riskPage.unmount();

    const newCoinPage = render(<ResourcePage config={resourceConfigs.newCoinProjects} />);
    await user.click(await screen.findByRole('button', { name: '添加新币项目' }));
    dialog = await screen.findByRole('dialog', { name: '添加新币项目' });
    expectCreateModalSize(dialog, 'extra-wide');
    semiSelectByLabel(dialog, '项目资产');
    semiInputByLabel(dialog, '项目符号');
    semiSelectByLabel(dialog, '生命周期');
    semiInputByLabel(dialog, '发行总量');
    semiInputByLabel(dialog, '发行价');
    semiSelectByLabel(dialog, '解禁类型');
    semiSelectByLabel(dialog, '启用解禁矿工费');
    await selectSemiOption(user, dialog, '项目资产', 'BTC - Bitcoin（ID: 11）');
    await user.type(within(dialog).getByLabelText('项目符号'), 'ABC');
    await user.type(within(dialog).getByLabelText('发行总量'), '1000000');
    await user.type(within(dialog).getByLabelText('发行价'), '1');
    await user.type(within(dialog).getByLabelText('固定解禁时间'), '1794309753000');
    await user.click(within(dialog).getByRole('button', { name: '提交添加新币项目' }));
    await confirmWithReason('create new coin');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/new-coins', expect.objectContaining({ method: 'POST' }));
    });
    request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/new-coins' && init && 'method' in init)?.[1];
    const newCoinBody = JSON.parse(String(request?.body));
    expect(newCoinBody).toEqual({
      asset_id: 11,
      symbol: 'ABC',
      lifecycle_status: 'preheat',
      total_supply: '1000000',
      issue_price: '1',
      unlock_type: 'fixed_time',
      fixed_unlock_at: 1794309753000,
      unlock_fee_enabled: false,
      reason: 'create new coin'
    });
    expect(newCoinBody).not.toHaveProperty('listed_at');
    expect(newCoinBody).not.toHaveProperty('relative_unlock_seconds');
    newCoinPage.unmount();

    render(<ResourcePage config={resourceConfigs.users} />);
    expect(await screen.findByText('user@example.com')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
    await user.click(await screen.findByRole('button', { name: '添加用户' }));
    dialog = await screen.findByRole('dialog', { name: '添加用户' });
    expectCreateModalSize(dialog, 'medium');
    semiInputByLabel(dialog, '邮箱');
    semiInputByLabel(dialog, '手机号');
    semiInputByLabel(dialog, '登录密码');
    semiSelectByLabel(dialog, '状态');
    semiInputByLabel(dialog, 'KYC等级');
    await user.type(within(dialog).getByLabelText('邮箱'), 'new-user@example.com');
    await user.type(within(dialog).getByLabelText('手机号'), '18800001111');
    await user.type(within(dialog).getByLabelText('登录密码'), 'Password123!');
    await user.clear(within(dialog).getByLabelText('KYC等级'));
    await user.type(within(dialog).getByLabelText('KYC等级'), '2');
    await user.click(within(dialog).getByRole('button', { name: '提交添加用户' }));
    await confirmWithReason('create user');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/users', expect.objectContaining({ method: 'POST' }));
    });
    request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/users' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      email: 'new-user@example.com',
      phone: '18800001111',
      password: 'Password123!',
      status: 'active',
      kyc_level: 2,
      reason: 'create user'
    });

    await user.click(screen.getByRole('button', { name: '查看详情' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/users/123');
    });
    await expectFormattedDetail('user-detail', /"detail": "user-detail"/);
    await user.click(screen.getByRole('button', { name: '查看资产' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/wallet/accounts?user_id=123&include_empty=true&limit=100');
    });
    await expectFormattedDetail('USDT', /"asset_symbol": "USDT"/);
    expect(screen.getByText('BTC')).toBeInTheDocument();
    expect(screen.getByText('可用')).toBeInTheDocument();
    expect(screen.getByText('10.00')).toBeInTheDocument();
    expect(screen.getAllByText('0.00')).toHaveLength(3);
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/users')).toHaveLength(1);
  });

  it('filters users by email from user management page', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/users') {
        const rows = [{ id: 123, email: 'user@example.com', phone: '18800000000', status: 'active', kyc_level: 1 }];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    render(<ResourcePage config={resourceConfigs.users} />);

    expect(await screen.findByText('user@example.com')).toBeInTheDocument();
    await user.type(screen.getByLabelText('邮箱'), 'target@example.com');
    await user.click(screen.getByRole('button', { name: '查询' }));

    await waitFor(() => {
      expect(listAdminResourceMock.mock.calls).toContainEqual(['/admin/api/v1/users', 'users', { email: 'target@example.com' }]);
    });
  });

  it('recharges a user wallet from user row actions', async () => {
    const user = userEvent.setup();
    const confirmWithReason = async (reason: string) => {
      const reasonInputs = await screen.findAllByLabelText('操作原因');
      await user.type(reasonInputs.at(-1)!, reason);
      const confirmButtons = screen.getAllByRole('button', { name: '确认' });
      await user.click(confirmButtons.at(-1)!);
    };

    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/users') {
        const rows = [
          {
            id: 123,
            email: 'user@example.com',
            phone: '18800000000',
            status: 'active',
            kyc_level: 1
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    render(<ResourcePage config={resourceConfigs.users} />);
    expect(await screen.findByText('user@example.com')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '充值' }));
    const dialog = await screen.findByRole('dialog', { name: '用户充值' });
    semiInputByLabel(dialog, '用户ID');
    semiSelectByLabel(dialog, '充值资产');
    semiInputByLabel(dialog, '充值金额');
    await selectSemiOption(user, dialog, '充值资产', 'USDT - Tether（ID: 12）');
    await user.type(within(dialog).getByLabelText('充值金额'), '25.5');
    await user.click(within(dialog).getByRole('button', { name: '提交充值' }));
    await confirmWithReason('manual recharge');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/users/123/recharge', expect.objectContaining({ method: 'POST' }));
    });
    const request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/users/123/recharge' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      asset_id: 12,
      amount: '25.5',
      reason: 'manual recharge'
    });
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/users')).toHaveLength(2);
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

    expect(await screen.findByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
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

  it('uses dropdown filters, localized market type labels, and pushed latest prices on market pairs', async () => {
    const user = userEvent.setup();
    const sockets: WebSocketMock[] = [];
    class MarketPairWebSocketMock extends WebSocketMock {
      constructor(url: string) {
        super(url);
        sockets.push(this);
      }
    }
    vi.stubGlobal('WebSocket', MarketPairWebSocketMock);
    Object.defineProperty(window, 'WebSocket', { configurable: true, value: MarketPairWebSocketMock });
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
          },
          {
            id: 2,
            symbol: 'ETH-USDT',
            base_asset: 'ETH',
            quote_asset: 'USDT',
            price_precision: 8,
            qty_precision: 6,
            min_order_value: '5.0000',
            market_type: 'strategy',
            status: 'disabled'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    const { unmount } = render(<ResourcePage config={resourceConfigs.marketPairs} />);

    expect(await screen.findByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('最新价格')).toBeInTheDocument();
    semiSelectByLabel(document.body, '交易对');
    semiSelectByLabel(document.body, '状态');
    semiSelectByLabel(document.body, '市场类型');
    expect(screen.getByText('外部行情', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('策略行情', { selector: 'span' })).toBeInTheDocument();
    expect(screen.queryByText('external')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '查看详情' })).toHaveLength(2);
    expect(screen.getAllByRole('button', { name: '修改' })).toHaveLength(2);

    await waitFor(() => {
      expect(sockets.map((socket) => socket.url).some((url) => url.endsWith('/ws/public/ticker/BTCUSDT'))).toBe(true);
      expect(sockets.map((socket) => socket.url).some((url) => url.endsWith('/ws/public/ticker/ETHUSDT'))).toBe(true);
    });
    act(() => {
      sockets.find((socket) => socket.url.endsWith('/BTCUSDT'))?.emitMessage({ symbol: 'BTCUSDT', last_price: '67890.1200', observed_at: 1_735_732_800_000 });
      sockets.find((socket) => socket.url.endsWith('/ETHUSDT'))?.emitMessage({ symbol: 'ETHUSDT', last_price: '3200.55', observed_at: 1_735_732_800_000 });
    });

    expect(await screen.findByText('67,890.12')).toBeInTheDocument();
    expect(screen.getByText('3,200.55')).toBeInTheDocument();

    await selectSemiOption(user, document.body, '交易对', 'BTC-USDT');
    await selectSemiOption(user, document.body, '状态', '禁用');
    await selectSemiOption(user, document.body, '市场类型', '策略行情');
    await user.click(screen.getByRole('button', { name: '查询' }));

    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/api/v1/market-pairs', 'pairs', {
        symbol: 'BTC-USDT',
        status: 'disabled',
        market_type: 'strategy'
      });
    });
    unmount();
    expect(sockets.every((socket) => socket.closed)).toBe(true);
  });

  it('opens market pair details from row actions', async () => {
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
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/market-pairs/1') {
        return { id: 1, symbol: 'BTC-USDT', detail: 'market-pair-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.marketPairs} />);

    expect(await screen.findByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-pairs/1');
    });
    await expectFormattedDetail('market-pair-detail', /"detail": "market-pair-detail"/);
  });

  it('updates market pair safe config fields from row actions', async () => {
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
            base_asset_id: 11,
            quote_asset_id: 12,
            base_asset: 'BTC',
            quote_asset: 'USDT',
            price_precision: 8,
            qty_precision: 6,
            min_order_value: '10.000000000000000000',
            market_type: 'external',
            status: 'active'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    render(<ResourcePage config={resourceConfigs.marketPairs} />);

    expect(await screen.findByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '修改' }));
    const dialog = await screen.findByRole('dialog', { name: '修改交易对配置' });
    expect(within(dialog).queryByText('仅允许修改运营配置字段；交易对、基础资产、计价资产和状态保持只读。')).not.toBeInTheDocument();
    semiInputByLabel(dialog, '交易对');
    semiInputByLabel(dialog, '基础资产');
    semiInputByLabel(dialog, '计价资产');
    semiInputByLabel(dialog, '当前状态');
    semiInputByLabel(dialog, '价格精度');
    semiInputByLabel(dialog, '数量精度');
    semiInputByLabel(dialog, '最小下单额');
    semiSelectByLabel(dialog, '市场类型');
    await user.clear(within(dialog).getByLabelText('价格精度'));
    await user.type(within(dialog).getByLabelText('价格精度'), '10');
    await user.clear(within(dialog).getByLabelText('数量精度'));
    await user.type(within(dialog).getByLabelText('数量精度'), '4');
    await user.clear(within(dialog).getByLabelText('最小下单额'));
    await user.type(within(dialog).getByLabelText('最小下单额'), '25.000000000000000000');
    await selectSemiOption(user, dialog, '市场类型', '策略行情');
    await user.click(within(dialog).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'adjust pair config');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-pairs/1', expect.objectContaining({ method: 'PATCH' }));
    });
    const request = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/market-pairs/1')?.[1];
    expect(request).toBeDefined();
    const body = JSON.parse(String(request?.body));
    expect(body).toEqual({
      price_precision: 10,
      qty_precision: 4,
      min_order_value: '25.000000000000000000',
      market_type: 'strategy',
      reason: 'adjust pair config'
    });
    expect(body).not.toHaveProperty('symbol');
    expect(body).not.toHaveProperty('base_asset_id');
    expect(body).not.toHaveProperty('quote_asset_id');
    expect(body).not.toHaveProperty('status');
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/market-pairs')).toHaveLength(2);
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

    expect(await screen.findByText('0.50')).toBeInTheDocument();
    expect(screen.getByText('已成交数量')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/spot/orders/7');
    });
    await expectFormattedDetail('spot-order-detail', /"detail": "spot-order-detail"/);

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

    expect(await screen.findByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/products/14');
    });
    await expectFormattedDetail('margin-product-detail', /"detail": "margin-product-detail"/);

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

    expect(await screen.findByText('400.00')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '强平' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '关闭仓位' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '修改状态' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/positions/21');
    });
    await expectFormattedDetail('margin-position-detail', /"detail": "margin-position-detail"/);
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
    await expectFormattedDetail('margin-liquidation-detail', /"detail": "margin-liquidation-detail"/);
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
    await expectFormattedDetail('seconds-product-detail', /"detail": "seconds-product-detail"/);

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

    expect(await screen.findByText('10.00')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/orders/51');
    });
    await expectFormattedDetail('seconds-order-detail', /"detail": "seconds-order-detail"/);

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

    expect(await screen.findByText('12.00')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '结算赢' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '结算输' })).toBeDisabled();
  });

  it('creates earn products with category and multilingual rich text, then supports row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/earn/products') {
        const rows = [
          {
            id: 61,
            asset_id: 12,
            asset_symbol: 'USDT',
            name: 'USDT 30D',
            category: 'fixed_term',
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

    render(
      <StrictMode>
        <ResourcePage config={resourceConfigs.earnProducts} />
      </StrictMode>
    );

    expect(await screen.findByText('USDT 30D')).toBeInTheDocument();
    expect(screen.getByText('定期', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '添加理财产品' })).toBeInTheDocument();
    const initialEarnProductLoadCount = listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/products').length;

    await user.click(screen.getByRole('button', { name: '添加理财产品' }));
    const dialog = await screen.findByRole('dialog', { name: '添加理财产品' });
    expectCreateModalSize(dialog, 'extra-wide');
    const earnProductLayout = dialog.querySelector('.admin-earn-product-layout') as HTMLElement | null;
    expect(earnProductLayout).toBeInTheDocument();
    expect(getComputedStyle(earnProductLayout as HTMLElement).display).toBe('grid');
    expect(dialog.querySelector('.admin-earn-product-basic-grid')).toBeInTheDocument();
    expect(dialog.querySelector('.admin-earn-introduction-card')).toBeInTheDocument();
    expect(dialog.querySelector('.admin-earn-introduction-meta')).toBeInTheDocument();
    expect(dialog.querySelector('.admin-earn-product-footer')).toBeInTheDocument();
    semiSelectByLabel(dialog, '理财资产');
    semiSelectByLabel(dialog, '产品分类');
    semiSelectByLabel(dialog, '初始状态');
    semiInputByLabel(dialog, '产品名称');
    semiInputByLabel(dialog, '期限天数');
    semiInputByLabel(dialog, '年化利率');
    semiInputByLabel(dialog, '最小申购');
    semiInputByLabel(dialog, '最大申购');
    semiInputByLabel(dialog, '语言');
    semiInputByLabel(dialog, '国家');
    semiInputByLabel(dialog, '介绍标题');
    await selectSemiOption(user, dialog, '理财资产', 'USDT - Tether（ID: 12）');
    expect(semiSelectByLabel(dialog, '理财资产')).toHaveTextContent('USDT - Tether（ID: 12）');
    await user.type(within(dialog).getByLabelText('产品名称'), 'USDT 稳健理财');
    await selectSemiOption(user, dialog, '产品分类', '结构化');
    expect(semiSelectByLabel(dialog, '产品分类')).toHaveTextContent('结构化');
    await user.type(within(dialog).getByLabelText('期限天数'), '30');
    await user.type(within(dialog).getByLabelText('年化利率'), '0.12');
    await user.type(within(dialog).getByLabelText('最小申购'), '10');
    await user.type(within(dialog).getByLabelText('最大申购'), '1000');
    await user.type(within(dialog).getByLabelText('介绍标题'), 'USDT 稳健理财');
    const firstRichTextEditor = within(dialog).getByLabelText('富文本内容');
    expect(firstRichTextEditor).toHaveAttribute('contenteditable', 'true');
    expect(firstRichTextEditor.tagName).not.toBe('TEXTAREA');
    const firstEditorShell = firstRichTextEditor.closest('[data-quill-editor="true"]') as HTMLElement | null;
    expect(firstEditorShell).toBeInTheDocument();
    expect(firstEditorShell).toHaveClass('quill-rich-text-editor');
    expect(getComputedStyle(firstEditorShell as HTMLElement).width).toBe('100%');
    expect(firstRichTextEditor).toHaveClass('ql-editor');
    const firstEditorToolbar = within(firstEditorShell as HTMLElement).getByRole('toolbar', { name: '富文本工具栏' });
    expect(firstEditorToolbar).toHaveClass('ql-toolbar', 'ql-snow');
    expect(firstEditorToolbar.querySelectorAll('.ql-formats')).toHaveLength(2);
    expect(firstEditorToolbar.querySelector('.ql-header')).toBeInTheDocument();
    expect(firstEditorToolbar.querySelector('.ql-blockquote')).toBeInTheDocument();
    expect(firstEditorToolbar.querySelector('.ql-bold')).toBeInTheDocument();
    expect(firstEditorToolbar.querySelector('.ql-italic')).toBeInTheDocument();
    expect(firstEditorToolbar.querySelector('.ql-underline')).toBeInTheDocument();
    const firstEditorContainer = firstEditorShell?.querySelector('.quill-rich-text-container') as HTMLElement | null;
    expect(firstEditorContainer).toHaveClass('ql-container', 'ql-snow');
    expect(getComputedStyle(firstEditorToolbar).display).toBe('block');
    expect(getComputedStyle(firstEditorToolbar).borderTopWidth).toBe('1px');
    expect(getComputedStyle(firstEditorToolbar).backgroundImage).toBe('none');
    expect(getComputedStyle(firstEditorContainer as HTMLElement).borderLeftWidth).toBe('1px');
    expect(getComputedStyle(firstRichTextEditor).whiteSpace).toBe('pre-wrap');
    fireEvent.input(firstRichTextEditor, { target: { innerText: '适合稳健型用户。' } });
    await user.click(within(dialog).getByRole('button', { name: '新增语言介绍' }));
    const localeInputs = within(dialog).getAllByLabelText('语言');
    await user.clear(localeInputs[1]);
    await user.type(localeInputs[1], 'en-US');
    const countryInputs = within(dialog).getAllByLabelText('国家');
    await user.clear(countryInputs[1]);
    await user.type(countryInputs[1], 'US');
    const titleInputs = within(dialog).getAllByLabelText('介绍标题');
    await user.clear(titleInputs[1]);
    await user.type(titleInputs[1], 'USDT Earn');
    const contentEditors = within(dialog).getAllByLabelText('富文本内容');
    expect(contentEditors).toHaveLength(2);
    expect(contentEditors[1]).toHaveAttribute('contenteditable', 'true');
    expect(contentEditors[1].tagName).not.toBe('TEXTAREA');
    fireEvent.input(contentEditors[1], { target: { innerText: 'For stable users.' } });
    const submitEarnProductButton = within(dialog).getByRole('button', { name: '提交添加理财产品' });
    await waitFor(() => {
      expect(submitEarnProductButton).not.toBeDisabled();
    });
    await user.click(submitEarnProductButton);
    await user.type(screen.getByLabelText('操作原因'), 'add earn product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products', expect.objectContaining({ method: 'POST' }));
    });
    const createRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/earn/products' && init && 'method' in init)?.[1];
    const createBody = JSON.parse(String(createRequest?.body));
    expect(createBody).toMatchObject({
      asset_id: 12,
      name: 'USDT 稳健理财',
      category: 'structured',
      term_days: 30,
      apr_rate: '0.12',
      min_subscribe: '10',
      max_subscribe: '1000',
      status: 'active',
      reason: 'add earn product'
    });
    expect(createBody.introduction_json).toMatchObject({
      version: 1,
      default_locale: 'zh-CN',
      items: [
        { locale: 'zh-CN', country: 'CN', title: 'USDT 稳健理财' },
        { locale: 'en-US', country: 'US', title: 'USDT Earn' }
      ]
    });
    expect(createBody.introduction_json.items[0].content).toEqual([{ type: 'p', children: [{ text: '适合稳健型用户。' }] }]);
    expect(createBody.introduction_json.items[1].content).toEqual([{ type: 'p', children: [{ text: 'For stable users.' }] }]);
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/products')).toHaveLength(initialEarnProductLoadCount + 1);
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products/61');
    });
    await expectFormattedDetail('earn-product-detail', /"detail": "earn-product-detail"/);

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable earn product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products/61/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable earn product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/products')).toHaveLength(initialEarnProductLoadCount + 2);
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

    expect(await screen.findByText('100.00')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '管理员赎回' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '赎回' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '修改状态' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/subscriptions/62');
    });
    await expectFormattedDetail('earn-subscription-detail', /"detail": "earn-subscription-detail"/);
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

    expect(await screen.findByText('0.01')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/convert/pairs/71');
    });
    await expectFormattedDetail('convert-pair-detail', /"detail": "convert-pair-detail"/);

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

  it('renders market strategy actions as a table with create, detail, edit, and status actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/market-strategies') {
        const rows = [
          {
            id: 91,
            pair_id: 21,
            symbol: 'BTC-USDT',
            market_type: 'strategy',
            strategy_type: 'price_path',
            start_price: '1.000000000000000000',
            target_price: '2.000000000000000000',
            start_time: 1_775_027_600_000,
            end_time: 1_775_031_200_000,
            volatility: '0.01000000',
            volume_min: '10.000000000000000000',
            volume_max: '20.000000000000000000',
            status: 'paused',
            run_status: 'paused',
            created_at: 1_775_027_600_000
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/market-strategies/91') {
        return { id: 91, detail: 'market-strategy-detail' };
      }

      return {};
    });

    const createPage = render(<ResourcePage config={resourceConfigs.marketStrategyActions} />);

    expect(await screen.findByText('行情策略动作')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '创建策略' }));
    const createDialog = await screen.findByRole('dialog', { name: '创建策略' });
    expectCreateModalSize(createDialog, 'wide');
    createPage.unmount();

    render(<ResourcePage config={resourceConfigs.marketStrategyActions} />);

    expect(await screen.findByText('行情策略动作')).toBeInTheDocument();
    const initialMarketStrategyLoadCount = listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/market-strategies').length;
    expect(screen.getByRole('button', { name: '创建策略' })).toBeInTheDocument();
    expect(screen.getByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    expect(screen.queryByText('更新策略状态')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '启用' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '查看详情' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-strategies/91');
    });
    await expectFormattedDetail('market-strategy-detail', /"detail": "market-strategy-detail"/);

    await user.click(screen.getByRole('button', { name: '修改' }));
    const editDialog = await screen.findByRole('dialog', { name: '修改行情策略' });
    await user.clear(within(editDialog).getByLabelText('策略类型'));
    await user.type(within(editDialog).getByLabelText('策略类型'), 'price_path_v2');
    await user.clear(within(editDialog).getByLabelText('起始价'));
    await user.type(within(editDialog).getByLabelText('起始价'), '1.100000000000000000');
    await user.clear(within(editDialog).getByLabelText('目标价'));
    await user.type(within(editDialog).getByLabelText('目标价'), '2.200000000000000000');
    await user.clear(within(editDialog).getByLabelText('开始时间戳'));
    await user.type(within(editDialog).getByLabelText('开始时间戳'), '1775031200000');
    await user.clear(within(editDialog).getByLabelText('结束时间戳'));
    await user.type(within(editDialog).getByLabelText('结束时间戳'), '1775034800000');
    await user.clear(within(editDialog).getByLabelText('波动率'));
    await user.type(within(editDialog).getByLabelText('波动率'), '0.02000000');
    await user.clear(within(editDialog).getByLabelText('最小成交量'));
    await user.type(within(editDialog).getByLabelText('最小成交量'), '12.000000000000000000');
    await user.clear(within(editDialog).getByLabelText('最大成交量'));
    await user.type(within(editDialog).getByLabelText('最大成交量'), '24.000000000000000000');
    await user.click(within(editDialog).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'update strategy config');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-strategies/91', expect.objectContaining({ method: 'PATCH' }));
    });
    const editRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/market-strategies/91' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(editRequest?.body))).toEqual({
      strategy_type: 'price_path_v2',
      start_price: '1.100000000000000000',
      target_price: '2.200000000000000000',
      start_time: 1775031200000,
      end_time: 1775034800000,
      volatility: '0.02000000',
      volume_min: '12.000000000000000000',
      volume_max: '24.000000000000000000',
      reason: 'update strategy config'
    });
    expect(JSON.parse(String(editRequest?.body))).not.toHaveProperty('pair_id');
    expect(JSON.parse(String(editRequest?.body))).not.toHaveProperty('status');

    await waitFor(() => {
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/market-strategies')).toHaveLength(initialMarketStrategyLoadCount + 1);
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
    await expectFormattedDetail('convert-order-detail', /"detail": "convert-order-detail"/);
  });

  it('exposes the spot trade fee column', () => {
    expect(resourceConfigs.spotTrades.columns).toEqual(
      expect.arrayContaining([expect.objectContaining({ key: 'fee', title: '手续费', type: 'amount' })])
    );
  });
});
