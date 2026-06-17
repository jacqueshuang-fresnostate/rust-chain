import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listAdminResource } from '../../api/adminResources';
import { apiRequest } from '../../api/client';
import { ResourcePage, resourceConfigs, type ResourceConfig } from './resourceConfigs';

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

async function findActionSheet(title: string): Promise<HTMLElement> {
  let sheet: HTMLElement | null = null;
  await waitFor(() => {
    const titleNode = [...document.querySelectorAll('.semi-sidesheet-title')].find((item) => item.textContent?.trim() === title) as HTMLElement | undefined;
    expect(titleNode).toBeDefined();
    sheet = titleNode?.closest('.semi-sidesheet-inner') as HTMLElement | null;
    expect(sheet).toBeInTheDocument();
  });
  const actionSheet = sheet;
  if (!actionSheet) {
    throw new Error(`SideSheet with title "${title}" was not found.`);
  }
  return actionSheet;
}

async function openFiltersTab(user: ReturnType<typeof userEvent.setup>) {
  void user;
  await waitFor(() => {
    expect(screen.getByRole('button', { name: '查询' })).toBeInTheDocument();
  });
}

function semiSelectByLabel(root: HTMLElement, label: string, index = 0): HTMLElement {
  const labelNode = [...root.querySelectorAll('label')].filter((item) => item.textContent?.trim().startsWith(label) && item.querySelector('.semi-select'))[index] as HTMLElement | undefined;
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

function expectFilter(config: ResourceConfig, key: string, expected: Partial<NonNullable<ResourceConfig['filters']>[number]>) {
  expect(config.filters?.find((filter) => filter.key === key)).toMatchObject(expected);
}

async function selectSemiOption(user: ReturnType<typeof userEvent.setup>, dialog: HTMLElement, label: string, optionLabel: string, index = 0) {
  await user.click(semiSelectByLabel(dialog, label, index));
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
    expect(semiSelectByLabel(dialog, label, index)).toHaveTextContent(optionLabel);
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

function stubResizeObserver() {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver');
  if (descriptor?.configurable === false) {
    if ('writable' in descriptor && descriptor.writable) {
      (globalThis as typeof globalThis & { ResizeObserver: typeof ResizeObserverMock }).ResizeObserver = ResizeObserverMock;
    }
    return;
  }
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
}

function stubMatchMedia() {
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      addEventListener: vi.fn(),
      addListener: vi.fn(),
      dispatchEvent: vi.fn(),
      matches: false,
      media: query,
      onchange: null,
      removeEventListener: vi.fn(),
      removeListener: vi.fn()
    }))
  });
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

  it('shows wallet ledger user email without user and asset ID columns', () => {
    const config = resourceConfigs.walletLedger;
    const columnKeys = resourceConfigs.walletLedger.columns.map((column) => column.key);

    expect(columnKeys).toContain('user_email');
    expect(columnKeys).toContain('asset_symbol');
    expect(columnKeys).not.toContain('user_id');
    expect(columnKeys).not.toContain('asset_id');
    expect(columnKeys).not.toContain('ref_id');
    expect(resourceConfigs.walletLedger.columns.find((column) => column.key === 'user_email')).toMatchObject({
      title: '用户邮箱'
    });
    expectFilter(config, 'asset_id', { label: '资产', optionLabelKey: 'asset_symbol', type: 'select', optionsFromRows: true });
    expectFilter(config, 'change_type', {
      label: '变动类型',
      type: 'select',
      options: expect.arrayContaining([
        { label: '快速充值', value: 'quick_recharge' },
        { label: '后台充值', value: 'admin_recharge' },
        { label: '现货成交结算', value: 'spot_trade_settlement' },
        { label: '贷款放款', value: 'loan_disbursement' },
        { label: '贷款还款', value: 'loan_repayment' }
      ])
    });
    expectFilter(config, 'ref_type', {
      label: '来源类型',
      type: 'select',
      options: expect.arrayContaining([
        { label: '快速充值', value: 'quick_recharge' },
        { label: '后台充值', value: 'admin_recharge' },
        { label: '现货成交', value: 'spot_trade' },
        { label: '贷款订单', value: 'loan_order' }
      ])
    });
    expect(config.toolbarFilters).toEqual([{ key: 'include_internal', label: '显示机器人数据', type: 'switch' }]);
    expect(config.columns.find((column) => column.key === 'change_type')).toMatchObject({
      valueMap: expect.objectContaining({ quick_recharge: '快速充值', admin_recharge: '后台充值', loan_disbursement: '贷款放款' })
    });
    expect(config.columns.find((column) => column.key === 'balance_type')).toMatchObject({
      valueMap: expect.objectContaining({ available: '可用余额', frozen: '冻结余额', locked: '锁定余额' })
    });
    expect(config.columns.find((column) => column.key === 'ref_type')).toMatchObject({
      valueMap: expect.objectContaining({ quick_recharge: '快速充值', admin_recharge: '后台充值', loan_order: '贷款订单' })
    });
  });

  it('configures loan product and order resources with Chinese business fields', () => {
    const productColumns = resourceConfigs.loanProducts.columns.map((column) => column.key);
    const orderColumns = resourceConfigs.loanOrders.columns.map((column) => column.key);

    expect(resourceConfigs.loanProducts.endpoint).toBe('/admin/api/v1/loan/products');
    expect(resourceConfigs.loanOrders.endpoint).toBe('/admin/api/v1/loan/orders');
    expect(productColumns).toContain('asset_symbol');
    expect(productColumns).toContain('loan_type');
    expect(productColumns).toContain('name_json');
    expect(productColumns).not.toContain('asset_id');
    expect(orderColumns).toContain('user_email');
    expect(orderColumns).toContain('product_name');
    expect(orderColumns).not.toContain('user_id');
    expect(orderColumns).not.toContain('product_id');
    expect(resourceConfigs.loanProducts.columns.find((column) => column.key === 'loan_type')).toMatchObject({
      valueMap: expect.objectContaining({ credit: '信用贷', collateralized: '抵押贷' })
    });
    expect(resourceConfigs.loanOrders.columns.find((column) => column.key === 'status')).toMatchObject({
      valueMap: expect.objectContaining({ pending: '待审核', disbursed: '已放款', repaid: '已还款' })
    });
    expectFilter(resourceConfigs.loanOrders, 'loan_type', {
      label: '贷款类型',
      type: 'select',
      options: expect.arrayContaining([{ label: '抵押贷', value: 'collateralized' }])
    });
  });

  it('shows wallet account user email without internal ID columns', () => {
    const columnKeys = resourceConfigs.walletAccounts.columns.map((column) => column.key);

    expect(columnKeys).toContain('user_email');
    expect(columnKeys).toContain('asset_symbol');
    expect(columnKeys).not.toContain('id');
    expect(columnKeys).not.toContain('user_id');
    expect(columnKeys).not.toContain('asset_id');
    expect(resourceConfigs.walletAccounts.columns.find((column) => column.key === 'user_email')).toMatchObject({
      title: '用户邮箱'
    });
    expect(resourceConfigs.walletAccounts.toolbarFilters).toEqual([{ key: 'include_internal', label: '显示机器人数据', type: 'switch' }]);
  });

  it('shows asset deposit support as a status column', () => {
    expect(resourceConfigs.assets.columns.find((column) => column.key === 'deposit_enabled')).toMatchObject({
      title: '支持充值',
      type: 'status'
    });
    expect(resourceConfigs.assets.columns.find((column) => column.key === 'withdraw_enabled')).toMatchObject({
      title: '支持提现',
      type: 'status'
    });
    expect(resourceConfigs.assets.columns.find((column) => column.key === 'min_deposit_amount')).toMatchObject({
      title: '最小充值数量',
      type: 'amount'
    });
    expect(resourceConfigs.assets.columns.find((column) => column.key === 'deposit_fee')).toMatchObject({
      title: '充值手续费',
      type: 'amount'
    });
    expect(resourceConfigs.assets.columns.find((column) => column.key === 'withdraw_fee')).toMatchObject({
      title: '提现手续费',
      type: 'amount'
    });
  });

  it('shows spot order user email and localized order fields without internal ID columns', () => {
    const config = resourceConfigs.spotOrders;
    const columnKeys = config.columns.map((column) => column.key);

    expect(columnKeys).toContain('user_email');
    expect(columnKeys).toContain('average_price');
    expect(columnKeys).not.toContain('id');
    expect(columnKeys).not.toContain('user_id');
    expect(config.columns.find((column) => column.key === 'user_email')).toMatchObject({
      title: '用户邮箱'
    });
    expect(config.columns.find((column) => column.key === 'pair_id')).toMatchObject({
      title: '交易对'
    });
    expect(config.columns.find((column) => column.key === 'average_price')).toMatchObject({
      title: '成交价',
      type: 'amount'
    });
    expect(config.columns.find((column) => column.key === 'side')).toMatchObject({
      title: '方向',
      valueMap: expect.objectContaining({ buy: '买入', sell: '卖出' })
    });
    expect(config.columns.find((column) => column.key === 'order_type')).toMatchObject({
      title: '订单类型',
      valueMap: expect.objectContaining({ limit: '限价单', market: '市价单' })
    });
    expect(config.columns.find((column) => column.key === 'status')).toMatchObject({
      title: '状态',
      valueMap: expect.objectContaining({ open: '当前委托', filled: '已成交', cancelled: '已撤销' })
    });
    expectFilter(config, 'pair_id', { label: '交易对', type: 'select', optionsFromRows: true });
    expectFilter(config, 'status', {
      label: '状态',
      type: 'select',
      options: expect.arrayContaining([
        { label: '当前委托', value: 'open' },
        { label: '已成交', value: 'filled' },
        { label: '已撤销', value: 'cancelled' }
      ])
    });
    expect(config.filters?.some((filter) => filter.key === 'include_internal')).toBe(false);
    expect(config.toolbarFilters).toEqual([{ key: 'include_internal', label: '显示机器人订单', type: 'switch' }]);
  });

  it('shows convert order user email and asset symbols without internal reference columns', () => {
    const config = resourceConfigs.convertOrders;
    const columnKeys = config.columns.map((column) => column.key);

    expect(columnKeys).toContain('user_email');
    expect(columnKeys).toContain('from_asset_symbol');
    expect(columnKeys).toContain('to_asset_symbol');
    expect(columnKeys).not.toContain('quote_id');
    expect(columnKeys).not.toContain('user_id');
    expect(columnKeys).not.toContain('convert_pair_id');
    expect(config.columns.find((column) => column.key === 'user_email')).toMatchObject({
      title: '用户邮箱'
    });
    expect(config.columns.find((column) => column.key === 'from_asset_symbol')).toMatchObject({
      title: '源资产'
    });
    expect(config.columns.find((column) => column.key === 'to_asset_symbol')).toMatchObject({
      title: '目标资产'
    });
  });

  it('configures the deposit address pool without internal ID columns', () => {
    const config = resourceConfigs.depositAddressPool;
    const columnKeys = config.columns.map((column) => column.key);

    expect(config.title).toBe('充值地址池');
    expect(config.endpoint).toBe('/admin/api/v1/deposit-address-pool');
    expect(config.responseKey).toBe('addresses');
    expect(config.showJsonAction).toBe(false);
    expectFilter(config, 'network', {
      label: '网络',
      type: 'select',
      options: [
        { label: 'ETH', value: 'eth' },
        { label: 'Base', value: 'base' },
        { label: 'Tron', value: 'tron' },
        { label: 'BTC', value: 'btc' },
        { label: 'Solana', value: 'solana' }
      ]
    });
    expectFilter(config, 'status', {
      label: '状态',
      type: 'select',
      options: [
        { label: '可用', value: 'available' },
        { label: '已分配', value: 'assigned' },
        { label: '禁用', value: 'disabled' }
      ]
    });
    expect(columnKeys).toEqual([
      'network',
      'address',
      'asset_symbols',
      'status',
      'assigned_user_email',
      'assigned_asset_symbol',
      'assigned_at',
      'memo',
      'remark',
      'updated_at'
    ]);
    expect(columnKeys).not.toContain('id');
    expect(columnKeys).not.toContain('assigned_user_id');
  });

  it('configures the Admin country list with filters, columns, and hidden JSON action', () => {
    const config = resourceConfigs.countries;

    expect(config.title).toBe('国家配置');
    expect(config.endpoint).toBe('/admin/api/v1/countries');
    expect(config.responseKey).toBe('countries');
    expect(config.showJsonAction).toBe(false);
    expectFilter(config, 'country_code', { label: '国家代码' });
    expectFilter(config, 'status', {
      label: '状态',
      type: 'select',
      options: [
        { label: '启用', value: 'active' },
        { label: '停用', value: 'disabled' }
      ]
    });
    expectFilter(config, 'registration_enabled', {
      label: '开放注册',
      type: 'select',
      options: [
        { label: '启用', value: 'true' },
        { label: '停用', value: 'false' }
      ]
    });
    expectFilter(config, 'limit', { label: '数量限制' });
    expect(config.columns).toEqual([
      { key: 'id', title: '国家配置ID' },
      { key: 'country_code', title: '国家代码' },
      { key: 'country_name', title: '国家名称' },
      { key: 'remark', title: '备注（中文名称）' },
      { key: 'default_locale', title: '默认语言' },
      expect.objectContaining({ key: 'supported_locales', title: '支持语言' }),
      expect.objectContaining({ key: 'registration_enabled', title: '开放注册', type: 'status' }),
      expect.objectContaining({ key: 'status', title: '状态', type: 'status' }),
      { key: 'sort_order', title: '排序' },
      expect.objectContaining({ key: 'updated_at', title: '更新时间', type: 'timestamp' })
    ]);
  });

  it('lists Admin countries from the countries endpoint and renders locale arrays', async () => {
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey, filters) => {
      if (endpoint === '/admin/api/v1/countries') {
        const rows = [
          {
            id: 8,
            country_code: 'US',
            country_name: 'United States',
            remark: '美国',
            default_locale: 'en',
            supported_locales: ['en'],
            registration_enabled: true,
            status: 'active',
            sort_order: 10,
            updated_at: 1_700_000_100_000
          }
        ];
        return { rows, raw: { [responseKey]: rows, filters } };
      }

      return { rows: [], raw: {} };
    });

    render(<ResourcePage config={resourceConfigs.countries} />);

    expect(await screen.findByText('United States')).toBeInTheDocument();
    expect(screen.getByText('美国')).toBeInTheDocument();
    expect(screen.getAllByText('en').length).toBeGreaterThan(0);
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
    expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/api/v1/countries', 'countries', {});
  });

  it('creates edits and updates Admin country status from row actions', async () => {
    const user = userEvent.setup();
    const confirmWithReason = async (reason: string) => {
      const reasonInputs = await screen.findAllByLabelText('操作原因');
      await user.type(reasonInputs.at(-1)!, reason);
      const confirmButtons = screen.getAllByRole('button', { name: '确认' });
      await user.click(confirmButtons.at(-1)!);
    };

    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/countries') {
        const rows = [
          {
            id: 8,
            country_code: 'US',
            country_name: 'United States',
            remark: '美国',
            default_locale: 'en',
            supported_locales: ['en'],
            registration_enabled: true,
            status: 'active',
            sort_order: 10,
            updated_at: 1_700_000_100_000
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/countries/8') {
        return { id: 8, detail: 'country-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.countries} />);

    expect(await screen.findByText('United States')).toBeInTheDocument();
    expect(screen.getByText('美国')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '添加国家' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '停用' })).toBeInTheDocument();
    const initialCountryLoadCount = listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/countries').length;

    await user.click(screen.getByRole('button', { name: '添加国家' }));
    let dialog = await findActionSheet('添加国家');
    expectCreateModalSize(dialog, 'medium');
    semiInputByLabel(dialog, '国家代码');
    semiInputByLabel(dialog, '国家名称');
    semiInputByLabel(dialog, '备注（中文名称）');
    semiSelectByLabel(dialog, '默认语言');
    semiInputByLabel(dialog, '支持语言');
    semiSelectByLabel(dialog, '开放注册');
    semiSelectByLabel(dialog, '初始状态');
    semiInputByLabel(dialog, '排序');
    await user.type(within(dialog).getByLabelText('国家代码'), ' jp ');
    await user.type(within(dialog).getByLabelText('国家名称'), '日本');
    await user.type(within(dialog).getByLabelText('备注（中文名称）'), '日本');
    await selectSemiOption(user, dialog, '默认语言', '英文');
    await user.clear(within(dialog).getByLabelText('支持语言'));
    await user.type(within(dialog).getByLabelText('支持语言'), 'en, zh');
    await selectSemiOption(user, dialog, '开放注册', '启用');
    await selectSemiOption(user, dialog, '初始状态', '启用');
    await user.clear(within(dialog).getByLabelText('排序'));
    await user.type(within(dialog).getByLabelText('排序'), '30');
    await user.click(within(dialog).getByRole('button', { name: '提交添加国家' }));
    await confirmWithReason('create country');

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/countries', expect.objectContaining({ method: 'POST' }));
    });
    let request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/countries' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      country_code: 'JP',
      country_name: '日本',
      remark: '日本',
      default_locale: 'en',
      supported_locales: ['en', 'zh'],
      registration_enabled: true,
      status: 'active',
      sort_order: 30,
      reason: 'create country'
    });

    await user.click(screen.getByRole('button', { name: '查看详情' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/countries/8');
    });
    await expectFormattedDetail('country-detail', /"detail": "country-detail"/);

    await user.click(screen.getByRole('button', { name: '修改' }));
    dialog = await findActionSheet('修改国家配置');
    semiInputByLabel(dialog, '国家代码');
    semiInputByLabel(dialog, '国家名称');
    semiInputByLabel(dialog, '备注（中文名称）');
    semiSelectByLabel(dialog, '默认语言');
    semiInputByLabel(dialog, '支持语言');
    semiSelectByLabel(dialog, '开放注册');
    semiInputByLabel(dialog, '排序');
    await user.clear(within(dialog).getByLabelText('国家名称'));
    await user.type(within(dialog).getByLabelText('国家名称'), '台灣');
    await user.clear(within(dialog).getByLabelText('备注（中文名称）'));
    await user.type(within(dialog).getByLabelText('备注（中文名称）'), '台湾');
    await selectSemiOption(user, dialog, '默认语言', '中文');
    await user.clear(within(dialog).getByLabelText('支持语言'));
    await user.type(within(dialog).getByLabelText('支持语言'), 'zh,en');
    await selectSemiOption(user, dialog, '开放注册', '停用');
    await user.clear(within(dialog).getByLabelText('排序'));
    await user.type(within(dialog).getByLabelText('排序'), '5');
    await user.click(within(dialog).getByRole('button', { name: '提交修改' }));
    await confirmWithReason('edit country');

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/countries/8', expect.objectContaining({ method: 'PATCH' }));
    });
    request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/countries/8' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      country_name: '台灣',
      remark: '台湾',
      default_locale: 'zh',
      supported_locales: ['zh', 'en'],
      registration_enabled: false,
      sort_order: 5,
      reason: 'edit country'
    });
    expect(JSON.parse(String(request?.body))).not.toHaveProperty('country_code');
    expect(JSON.parse(String(request?.body))).not.toHaveProperty('status');

    await user.click(screen.getByRole('button', { name: '停用' }));
    await confirmWithReason('disable country');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/countries/8/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable country' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/countries').length).toBeGreaterThanOrEqual(initialCountryLoadCount + 3);
    });
  }, 20_000);

  it('configures the Admin news center list with filters and columns', () => {
    expect(resourceConfigs.news.title).toBe('新闻中心');
    expect(resourceConfigs.news.endpoint).toBe('/admin/api/v1/news');
    expect(resourceConfigs.news.responseKey).toBe('news');
    expect(resourceConfigs.news.showJsonAction).toBe(false);
    expectFilter(resourceConfigs.news, 'q', { label: '关键词' });
    expectFilter(resourceConfigs.news, 'status', { label: '状态', type: 'select' });
    expectFilter(resourceConfigs.news, 'category', { label: '分类', type: 'select' });
    expectFilter(resourceConfigs.news, 'country_code', { label: '国家' });
    expectFilter(resourceConfigs.news, 'locale', { label: '语言' });
    expectFilter(resourceConfigs.news, 'limit', { label: '数量限制' });
    expect(resourceConfigs.news.columns).toEqual([
      { key: 'id', title: '新闻ID' },
      { key: 'title', title: '标题' },
      expect.objectContaining({ key: 'banner_url', title: 'Banner' }),
      expect.objectContaining({ key: 'small_logo_url', title: '小 Logo' }),
      expect.objectContaining({ key: 'category', title: '分类' }),
      { key: 'country_code', title: '国家' },
      { key: 'default_locale', title: '默认语言' },
      expect.objectContaining({ key: 'status', title: '状态', type: 'status' }),
      expect.objectContaining({ key: 'published_at', title: '发布时间', type: 'timestamp' }),
      expect.objectContaining({ key: 'updated_at', title: '更新时间', type: 'timestamp' })
    ]);
  });

  it('lists Admin news from the news endpoint', async () => {
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey, filters) => ({
      rows:
        endpoint === '/admin/api/v1/news'
          ? [
              {
                id: 7,
                title: '平台公告',
                category: 'system',
                country_code: 'CN',
                default_locale: 'zh-CN',
                status: 'published',
                published_at: 1_700_000_000_000,
                updated_at: 1_700_000_100_000
              }
            ]
          : [],
      raw: { [responseKey]: [], filters }
    }));

    render(<ResourcePage config={resourceConfigs.news} />);

    expect(await screen.findByText('平台公告')).toBeInTheDocument();
    expect(screen.getByText('系统公告')).toBeInTheDocument();
    expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/api/v1/news', 'news', {});
  });

  it('creates edits publishes and archives Admin news', async () => {
    const user = userEvent.setup();
    const confirmWithReason = async (reason: string) => {
      const reasonInputs = await screen.findAllByLabelText('操作原因');
      await user.type(reasonInputs.at(-1)!, reason);
      const confirmButtons = screen.getAllByRole('button', { name: '确认' });
      await user.click(confirmButtons.at(-1)!);
    };
    const newsContent = {
      version: 1,
      default_locale: 'zh-CN',
      items: [
        {
          locale: 'zh-CN',
          country_code: 'CN',
          title: '平台公告',
          summary: '旧摘要',
          content: [{ type: 'p', children: [{ text: '旧内容' }] }]
        }
      ]
    };

    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/news') {
        const rows = [
          {
            id: 7,
            title: '平台公告',
            category: 'system',
            country_code: 'CN',
            default_locale: 'zh-CN',
            status: 'draft',
            content_json: newsContent,
            updated_at: 1_700_000_100_000
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }
      if (endpoint === '/admin/api/v1/countries') {
        const rows = [
          {
            id: 1,
            country_code: 'US',
            country_name: 'United States',
            remark: '美国',
            default_locale: 'en',
            supported_locales: ['en'],
            status: 'active'
          },
          {
            id: 2,
            country_code: 'CN',
            country_name: '中国',
            remark: '中国',
            default_locale: 'zh',
            supported_locales: ['zh', 'en'],
            status: 'active'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/uploads/images') {
        return {
          delete_url: null,
          download_url: 'https://cdn.example.test/news-inline.png',
          mime_type: 'image/png',
          object_key: 'news-inline.png',
          provider: 'image_bed',
          share_url: null,
          size_bytes: 8
        };
      }
      if (path === '/admin/api/v1/news/7') {
        return { id: 7, detail: 'news-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.news} />);

    expect(await screen.findByText('平台公告')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '添加新闻' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '编辑' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '发布' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '归档' })).toBeInTheDocument();
    const initialNewsLoadCount = listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/news').length;

    await user.click(screen.getByRole('button', { name: '添加新闻' }));
    let dialog = await findActionSheet('添加新闻');
    expectCreateModalSize(dialog, 'extra-wide');
    expect(dialog.querySelector('.admin-news-create-layout')).toBeInTheDocument();
    expect(within(dialog).getByText('发布设置')).toBeInTheDocument();
    expect(within(dialog).getByText('视觉素材')).toBeInTheDocument();
    expect(within(dialog).getByText('内容编辑')).toBeInTheDocument();
    semiInputByLabel(dialog, '新闻标题');
    semiSelectByLabel(dialog, '分类');
    semiSelectByLabel(dialog, '国家');
    expect(within(dialog).queryByLabelText('默认语言')).not.toBeInTheDocument();
    semiSelectByLabel(dialog, '初始状态');
    expect(within(dialog).queryByLabelText('语言')).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('翻译国家')).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('翻译标题')).not.toBeInTheDocument();
    const createSummaryEditor = within(dialog).getByLabelText('摘要');
    expect(createSummaryEditor).toHaveAttribute('contenteditable', 'true');
    expect(createSummaryEditor.closest('.ql-editor')).toHaveAttribute('data-placeholder', '请输入新闻摘要');
    const createEditor = within(dialog).getByLabelText('富文本内容');
    expect(createEditor).toHaveAttribute('contenteditable', 'true');
    expect(createEditor.closest('.ql-editor')).toHaveAttribute('data-placeholder', '请输入新闻内容');
    expect(within(dialog).getAllByRole('button', { name: '插入图片' }).length).toBeGreaterThan(0);
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/api/v1/countries', 'countries', expect.objectContaining({ status: 'active' }));
      expect(semiSelectByLabel(dialog, '国家')).not.toHaveClass('semi-select-disabled');
    });
    await user.type(within(dialog).getByLabelText('新闻标题'), '平台新公告');
    await selectSemiOption(user, dialog, '分类', '市场资讯');
    await selectSemiOption(user, dialog, '国家', 'United States (US)');
    await selectSemiOption(user, dialog, '初始状态', '已发布');
    fireEvent.input(createSummaryEditor, { target: { innerText: '公告摘要' } });
    expect(within(dialog).getByRole('button', { name: '提交添加新闻' })).toBeDisabled();
    fireEvent.input(createEditor, { target: { innerText: '公告正文' } });
    const inlineImageInput = dialog.querySelector('.quill-rich-text-upload input[type="file"]') as HTMLInputElement | null;
    expect(inlineImageInput).toBeInTheDocument();
    const inlineImage = new File(['png-data'], 'news-inline.png', { type: 'image/png' });
    await user.upload(inlineImageInput as HTMLInputElement, inlineImage);
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/uploads/images', expect.objectContaining({ method: 'POST', body: expect.any(FormData) }));
    });
    expect(within(dialog).queryByRole('button', { name: '新增语言内容' })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '提交添加新闻' }));
    await confirmWithReason('create news');

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/news', expect.objectContaining({ method: 'POST' }));
    });
    let request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/news' && init && 'method' in init)?.[1];
    let body = JSON.parse(String(request?.body));
    expect(body).toMatchObject({
      title: '平台新公告',
      category: 'market',
      status: 'published',
      country_code: 'US',
      default_locale: 'en',
      reason: 'create news'
    });
    expect(body.content_json).toMatchObject({
      version: 1,
      default_locale: 'en',
      items: [
        { locale: 'en', country_code: 'US', title: '平台新公告' }
      ]
    });
    expect(body.content_json.items[0].summary).toEqual([{ type: 'p', children: [{ text: '公告摘要' }] }]);
    expect(body.content_json.items[0].content).toEqual([
      { type: 'p', children: [{ text: '公告正文' }] },
      { type: 'image', url: 'https://cdn.example.test/news-inline.png' }
    ]);

    await user.click(screen.getByRole('button', { name: '查看详情' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/news/7');
    });
    await expectFormattedDetail('news-detail', /"detail": "news-detail"/);

    await user.click(screen.getByRole('button', { name: '编辑' }));
    dialog = await findActionSheet('编辑新闻');
    semiInputByLabel(dialog, '新闻标题');
    semiSelectByLabel(dialog, '分类');
    semiInputByLabel(dialog, '国家');
    semiInputByLabel(dialog, '默认语言');
    await user.clear(within(dialog).getByLabelText('新闻标题'));
    await user.type(within(dialog).getByLabelText('新闻标题'), '平台公告更新');
    await selectSemiOption(user, dialog, '分类', '产品资讯');
    await user.clear(within(dialog).getByLabelText('国家'));
    await user.type(within(dialog).getByLabelText('国家'), 'JP');
    await user.click(within(dialog).getByRole('button', { name: '提交编辑新闻' }));
    await confirmWithReason('edit news');

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/news/7', expect.objectContaining({ method: 'PATCH' }));
    });
    request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/news/7' && init && 'method' in init)?.[1];
    body = JSON.parse(String(request?.body));
    expect(body).toMatchObject({
      title: '平台公告更新',
      category: 'product',
      country_code: 'JP',
      default_locale: 'zh-CN',
      reason: 'edit news'
    });
    expect(body.content_json.items[0]).toMatchObject({ locale: 'zh-CN', country_code: 'CN', title: '平台公告' });
    expect(body.content_json.items[0].summary).toEqual([{ type: 'p', children: [{ text: '旧摘要' }] }]);
    expect(body).not.toHaveProperty('status');

    await user.click(screen.getByRole('button', { name: '发布' }));
    await confirmWithReason('publish news');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/news/7/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'published', reason: 'publish news' })
      });
    });

    await user.click(screen.getByRole('button', { name: '归档' }));
    await confirmWithReason('archive news');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/news/7/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'archived', reason: 'archive news' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/news').length).toBeGreaterThanOrEqual(initialNewsLoadCount + 4);
    });
  }, 20_000);

  it('keeps the user ID column visible on user management', () => {
    expect(resourceConfigs.users.columns).toContainEqual({ key: 'id', title: '用户ID' });
  });

  it('shows user invite codes on user management', () => {
    expect(resourceConfigs.users.columns).toContainEqual({ key: 'invite_code', title: '邀请码' });
  });

  it('defaults robot-backed admin resources to a toolbar visibility switch', () => {
    expect(resourceConfigs.users.toolbarFilters).toEqual([{ key: 'include_internal', label: '显示机器人数据', type: 'switch' }]);
    expect(resourceConfigs.spotTrades.toolbarFilters).toEqual([{ key: 'include_internal', label: '显示机器人数据', type: 'switch' }]);
  });

  beforeEach(() => {
    stubResizeObserver();
    stubMatchMedia();
    vi.stubGlobal('WebSocket', undefined);
    Object.defineProperty(window, 'WebSocket', { configurable: true, value: undefined });
    listAdminResourceMock.mockReset();
    apiRequestMock.mockReset();
    mockEmptyResource();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('deletes unpaid quick recharge orders from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/quick-recharge/orders') {
        const rows = [
          {
            order_id: 'QRDELETE001',
            user_email: 'user@example.test',
            asset_symbol: 'USDT',
            currency: 'CNY',
            token: 'USDT',
            network: 'tron',
            fiat_amount: '100.000000000000000000',
            actual_amount: '13.000000000000000000',
            provider_trade_id: 'GMQRDELETE001',
            status: 'pending',
            created_at: 1_700_000_000_000
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockResolvedValue({});

    render(<ResourcePage config={resourceConfigs.quickRechargeOrders} />);

    expect(await screen.findByText('QRDELETE001')).toBeInTheDocument();
    expect(resourceConfigs.quickRechargeOrders.showJsonAction).toBe(false);
    await user.click(screen.getByRole('button', { name: '删除' }));
    await user.type(screen.getByLabelText('操作原因'), 'delete quick recharge order');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/quick-recharge/orders/QRDELETE001', {
        method: 'DELETE',
        body: JSON.stringify({ reason: 'delete quick recharge order' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/quick-recharge/orders')).toHaveLength(2);
    });
  });

  it('uses generated business order numbers for visible order columns', () => {
    expect(resourceConfigs.loanOrders.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '订单号' }));
    expect(resourceConfigs.spotOrders.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '订单号' }));
    expect(resourceConfigs.secondsOrders.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '订单号' }));
    expect(resourceConfigs.convertOrders.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '订单号' }));
    expect(resourceConfigs.predictionOrders.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '订单号' }));
    expect(resourceConfigs.earnSubscriptions.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '申购号' }));
    expect(resourceConfigs.newCoinSubscriptions.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '申购号' }));
    expect(resourceConfigs.newCoinPurchases.columns[0]).toEqual(expect.objectContaining({ key: 'order_no', title: '订单号' }));
    expect(resourceConfigs.spotTrades.columns).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ key: 'buy_order_id', title: '买单号' }),
        expect.objectContaining({ key: 'sell_order_id', title: '卖单号' })
      ])
    );
  });

  it('registers prediction resources with localized filters and market row actions', () => {
    expect(resourceConfigs.predictionAssetConfigs.endpoint).toBe('/admin/api/v1/prediction/asset-configs');
    expect(resourceConfigs.predictionMarkets.endpoint).toBe('/admin/api/v1/prediction/markets');
    expect(resourceConfigs.predictionOrders.endpoint).toBe('/admin/api/v1/prediction/orders');
    expect(resourceConfigs.predictionSyncLogs.endpoint).toBe('/admin/api/v1/prediction/sync/logs');
    expect(resourceConfigs.predictionMarkets.showJsonAction).toBe(false);
    expect(resourceConfigs.predictionMarkets.rowActions).toBeDefined();
    expect(resourceConfigs.predictionMarkets.columns).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ key: 'title', title: '市场标题' }),
        expect.objectContaining({ key: 'settlement_status', title: '结算状态' }),
        expect.objectContaining({ key: 'external_resolution', title: '外部结果' })
      ])
    );
    expect(resourceConfigs.predictionOrders.columns).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ key: 'user_email', title: '用户邮箱' }),
        expect.objectContaining({ key: 'fee_amount', title: '手续费' }),
        expect.objectContaining({ key: 'accepted_price', title: '成交概率' })
      ])
    );
  });

  it('creates deposit address pool entries with Semi form controls', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.depositAddressPool} />);

    await user.click(await screen.findByRole('button', { name: '添加充值地址' }));
    const dialog = await findActionSheet('添加充值地址');
    expectCreateModalSize(dialog, 'extra-wide');
    expect(within(dialog).getByText('地址规则')).toBeInTheDocument();
    expect(within(dialog).getByText('地址明细')).toBeInTheDocument();
    expect(within(dialog).getByText('地址 1')).toBeInTheDocument();
    await selectSemiOption(user, dialog, '网络', 'Base');
    await selectSemiOption(user, dialog, '支持币种', 'USDT - Tether（ID: 12）');
    await selectSemiOption(user, dialog, '支持币种', 'BTC - Bitcoin（ID: 11）');
    await user.type(within(dialog).getAllByLabelText('充值地址')[0], '0x1234567890abcdef1234567890abcdef12345678');
    await user.type(within(dialog).getAllByLabelText('Memo / Tag')[0], 'memo-1');
    await user.type(within(dialog).getAllByLabelText('备注')[0], 'pool address');
    await user.click(within(dialog).getByRole('button', { name: '新增一行' }));
    expect(within(dialog).getByText('地址 2')).toBeInTheDocument();
    await user.type(within(dialog).getAllByLabelText('充值地址')[1], '0xabcdefabcdefabcdefabcdefabcdefabcdefabcd');
    await user.type(within(dialog).getAllByLabelText('Memo / Tag')[1], 'memo-2');
    await user.type(within(dialog).getAllByLabelText('备注')[1], 'backup pool address');
    await user.click(within(dialog).getByRole('button', { name: '提交添加' }));
    await user.type(screen.getByLabelText('操作原因'), 'create deposit address');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/deposit-address-pool/batch', expect.objectContaining({ method: 'POST' }));
    });
    const request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/deposit-address-pool/batch' && init && 'method' in init)?.[1];
    expect(request).toBeDefined();
    expect(JSON.parse(String(request?.body))).toEqual({
      network: 'base',
      asset_symbols: ['USDT', 'BTC'],
      status: 'available',
      entries: [
        {
          address: '0x1234567890abcdef1234567890abcdef12345678',
          memo: 'memo-1',
          remark: 'pool address'
        },
        {
          address: '0xabcdefabcdefabcdefabcdefabcdefabcdefabcd',
          memo: 'memo-2',
          remark: 'backup pool address'
        }
      ],
      reason: 'create deposit address'
    });
  });

  it('imports deposit address pool entries from a file before submitting', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.depositAddressPool} />);

    await user.click(await screen.findByRole('button', { name: '添加充值地址' }));
    const dialog = await findActionSheet('添加充值地址');
    expect(within(dialog).getByText('导入文件')).toBeInTheDocument();
    await selectSemiOption(user, dialog, '网络', 'Tron');
    await selectSemiOption(user, dialog, '支持币种', 'USDT - Tether（ID: 12）');
    const fileInput = dialog.querySelector('input[type="file"]') as HTMLInputElement | null;
    expect(fileInput).toBeInTheDocument();
    expect(fileInput).toHaveAttribute('accept', '.csv,.txt');

    const importContent = ['充值地址,Memo / Tag,备注', '0x1111111111111111111111111111111111111111,memo-a,主地址', 'TImportedAddress\tmemo-b\t备用地址'].join('\n');
    const importFile = new File([importContent], 'deposit-addresses.csv', { type: 'text/csv' });
    Object.defineProperty(importFile, 'text', { value: async () => importContent });

    fireEvent.change(fileInput as HTMLInputElement, { target: { files: [importFile] } });

    await waitFor(() => {
      expect(within(dialog).getByText('地址 2')).toBeInTheDocument();
    });
    expect(within(dialog).getAllByLabelText('充值地址')[0]).toHaveValue('0x1111111111111111111111111111111111111111');
    expect(within(dialog).getAllByLabelText('Memo / Tag')[0]).toHaveValue('memo-a');
    expect(within(dialog).getAllByLabelText('备注')[0]).toHaveValue('主地址');
    expect(within(dialog).getAllByLabelText('充值地址')[1]).toHaveValue('TImportedAddress');
    expect(within(dialog).getAllByLabelText('Memo / Tag')[1]).toHaveValue('memo-b');
    expect(within(dialog).getAllByLabelText('备注')[1]).toHaveValue('备用地址');

    await user.click(within(dialog).getByRole('button', { name: '提交添加' }));
    await user.type(screen.getByLabelText('操作原因'), 'import deposit addresses');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/deposit-address-pool/batch', expect.objectContaining({ method: 'POST' }));
    });
    const request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/deposit-address-pool/batch' && init && 'method' in init)?.[1];
    expect(request).toBeDefined();
    expect(JSON.parse(String(request?.body))).toEqual({
      network: 'tron',
      asset_symbols: ['USDT'],
      status: 'available',
      entries: [
        {
          address: '0x1111111111111111111111111111111111111111',
          memo: 'memo-a',
          remark: '主地址'
        },
        {
          address: 'TImportedAddress',
          memo: 'memo-b',
          remark: '备用地址'
        }
      ],
      reason: 'import deposit addresses'
    });
  });

  it('shows deposit address details and reclaims assigned rows', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/deposit-address-pool') {
        const rows = [
          {
            id: 101,
            network: 'tron',
            address: 'TAssignedAddress',
            asset_symbol: null,
            asset_symbols: [],
            status: 'assigned',
            assigned_user_email: 'user@example.test',
            assigned_asset_symbol: 'USDT',
            assigned_at: 1_700_000_000_000,
            memo: 'memo',
            remark: 'remark',
            updated_at: 1_700_000_100_000
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/deposit-address-pool/101') {
        return { id: 101, status: 'assigned', address: 'TAssignedAddress' };
      }
      return {};
    });

    render(<ResourcePage config={resourceConfigs.depositAddressPool} />);

    expect(await screen.findByText('TAssignedAddress')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '修改' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: '查看详情' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/deposit-address-pool/101');
    });
    await user.click(screen.getByRole('button', { name: '回收' }));
    await user.type(screen.getByLabelText('操作原因'), 'reclaim address');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/deposit-address-pool/101/reclaim', {
        method: 'POST',
        body: JSON.stringify({ reason: 'reclaim address' })
      });
    });
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
            status: 'active',
            deposit_enabled: true,
            withdraw_enabled: true,
            min_deposit_amount: '1.000000000000000000',
            deposit_fee: '0.010000000000000000',
            withdraw_fee: '0.100000000000000000'
          },
          {
            id: 12,
            symbol: 'USDT',
            name: 'Tether',
            precision_scale: 6,
            asset_type: 'stablecoin',
            status: 'disabled',
            deposit_enabled: false,
            withdraw_enabled: false,
            min_deposit_amount: '2.000000000000000000',
            deposit_fee: '0.020000000000000000',
            withdraw_fee: '0.200000000000000000'
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
    expect(screen.getByText('数字货币', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('稳定币', { selector: 'span' })).toBeInTheDocument();
    expect(screen.queryByText('coin')).not.toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '查看详情' })).toHaveLength(2);
    expect(screen.getAllByRole('button', { name: '修改' })).toHaveLength(2);
    expect(screen.getAllByRole('button', { name: '删除' })).toHaveLength(1);

    await openFiltersTab(user);
    semiSelectByLabel(document.body, '资产类型');
    semiSelectByLabel(document.body, '状态');
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
    const editDialog = await findActionSheet('修改资产配置');
    await user.clear(within(editDialog).getByLabelText('资产名称'));
    await user.type(within(editDialog).getByLabelText('资产名称'), 'Bitcoin Updated');
    await user.clear(within(editDialog).getByLabelText('资产精度'));
    await user.type(within(editDialog).getByLabelText('资产精度'), '6');
    await user.clear(within(editDialog).getByLabelText('最小充值数量'));
    await user.type(within(editDialog).getByLabelText('最小充值数量'), '3');
    await user.clear(within(editDialog).getByLabelText('充值手续费'));
    await user.type(within(editDialog).getByLabelText('充值手续费'), '0.03');
    await user.clear(within(editDialog).getByLabelText('提现手续费'));
    await user.type(within(editDialog).getByLabelText('提现手续费'), '0.3');
    await selectSemiOption(user, editDialog, '资产类型', '稳定币');
    await selectSemiOption(user, editDialog, '状态', '禁用');
    await user.click(within(editDialog).getByLabelText('支持充值'));
    await user.click(within(editDialog).getByLabelText('支持提现'));
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
      deposit_enabled: false,
      withdraw_enabled: false,
      min_deposit_amount: '3',
      deposit_fee: '0.03',
      withdraw_fee: '0.3',
      reason: 'update asset config'
    });
    expect(body).not.toHaveProperty('symbol');
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/assets')).toHaveLength(3);

    await user.click(screen.getByRole('button', { name: '删除' }));
    await user.type(screen.getByLabelText('操作原因'), 'delete disabled asset');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/assets/12', {
        method: 'DELETE',
        body: JSON.stringify({ reason: 'delete disabled asset' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/assets')).toHaveLength(4);
    });
  });

  it('opens an asset creation modal from the asset management page without static helper copy', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.assets} />);

    await user.click(await screen.findByRole('button', { name: '添加资产' }));
    const dialog = await findActionSheet('添加资产');
    expectCreateModalSize(dialog, 'medium');
    expect(within(dialog).queryByText('资产创建后可作为交易对、钱包账户、闪兑和产品配置的基础资产。')).not.toBeInTheDocument();
    semiInputByLabel(dialog, '资产符号');
    semiInputByLabel(dialog, '资产名称');
    semiInputByLabel(dialog, '资产精度');
    semiInputByLabel(dialog, '最小充值数量');
    semiInputByLabel(dialog, '充值手续费');
    semiInputByLabel(dialog, '提现手续费');
    semiSelectByLabel(dialog, '资产类型');
    semiSelectByLabel(dialog, '初始状态');
    expect(within(dialog).getByLabelText('支持充值')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('支持提现')).toBeInTheDocument();
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
    const request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/assets' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(request?.body))).toMatchObject({
      symbol: 'btc',
      deposit_enabled: true,
      withdraw_enabled: true,
      min_deposit_amount: '0',
      deposit_fee: '0',
      withdraw_fee: '0',
      reason: 'add asset'
    });
  });

  it('opens a spot trading pair creation modal from the trading pair config page without static helper copy', async () => {
    const user = userEvent.setup();
    render(<ResourcePage config={resourceConfigs.marketPairs} />);

    await user.click(await screen.findByRole('button', { name: '添加交易对' }));
    const dialog = await findActionSheet('添加现货交易对');
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
            margin_modes: ['isolated'],
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
            margin_mode: 'isolated',
            margin_modes: ['isolated', 'cross'],
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
    expect(screen.getByText('逐仓 / 全仓', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('2.00x / 5.00x / 10.00x', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByText('3.00x / 7.00x', { selector: 'span' })).toBeInTheDocument();

    await user.click(await screen.findByRole('button', { name: '添加杠杆交易对' }));
    const dialog = await findActionSheet('添加杠杆交易对');
    expectCreateModalSize(dialog, 'extra-wide');
    expect(within(dialog).getByRole('tab', { name: '基础配置' })).toBeInTheDocument();
    expect(within(dialog).getByRole('tab', { name: '杠杆档位' })).toBeInTheDocument();
    expect(within(dialog).getByRole('tab', { name: '风控参数' })).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '提交添加杠杆交易对' })).toBeDisabled();
    expect(within(dialog).queryByLabelText('杠杆交易对ID')).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('最大杠杆')).not.toBeInTheDocument();
    semiSelectByLabel(dialog, '杠杆交易对');
    semiSelectByLabel(dialog, '保证金资产');
    semiSelectByLabel(dialog, '支持保证金模式');
    semiSelectByLabel(dialog, '初始状态');
    expect(within(dialog).queryByLabelText('自定义杠杆档位')).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('最小保证金')).not.toBeInTheDocument();
    await selectSemiOption(user, dialog, '杠杆交易对', 'BTC-USDT（ID: 21）');
    await selectSemiOption(user, dialog, '保证金资产', 'ETH - Ethereum（ID: 22）');
    await selectSemiOption(user, dialog, '支持保证金模式', '全仓');
    await user.click(within(dialog).getByRole('tab', { name: '杠杆档位' }));
    expect(within(dialog).getByText('杠杆档位', { selector: 'legend' })).toBeInTheDocument();
    expect(within(dialog).getByLabelText('2x')).not.toBeChecked();
    await user.click(within(dialog).getByLabelText('2x'));
    expect(within(dialog).getByLabelText('2x')).toBeChecked();
    await user.click(within(dialog).getByLabelText('5x'));
    expect(within(dialog).getByLabelText('5x')).toBeChecked();
    await user.click(within(dialog).getByLabelText('10x'));
    expect(within(dialog).getByLabelText('10x')).toBeChecked();
    semiInputByLabel(dialog, '自定义杠杆档位');
    await user.type(within(dialog).getByLabelText('自定义杠杆档位'), '25');
    expect(within(dialog).getByText('已选杠杆：2x / 5x / 10x / 25x')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('tab', { name: '风控参数' }));
    semiInputByLabel(dialog, '最小保证金');
    semiInputByLabel(dialog, '最大保证金');
    semiInputByLabel(dialog, '维持保证金率');
    semiInputByLabel(dialog, '小时利率');
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
      margin_modes: ['isolated', 'cross'],
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
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/market-pairs') {
        const rows = [
          { id: 31, symbol: 'BTC-USDT', status: 'active' },
          { id: 33, symbol: 'ETH-USDT', status: 'active' }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    render(<ResourcePage config={resourceConfigs.secondsProducts} />);

    await user.click(await screen.findByRole('button', { name: '添加秒合约交易对' }));
    const dialog = await findActionSheet('添加秒合约交易对');
    expectCreateModalSize(dialog, 'wide');
    expect(within(dialog).getByRole('tab', { name: '基础配置' })).toBeInTheDocument();
    expect(within(dialog).getByRole('tab', { name: '交易参数' })).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '提交添加秒合约交易对' })).toBeDisabled();
    expect(within(dialog).queryByLabelText('秒合约交易对ID')).not.toBeInTheDocument();
    semiSelectByLabel(dialog, '秒合约交易对');
    semiSelectByLabel(dialog, '押注资产');
    semiSelectByLabel(dialog, '初始状态');
    expect(within(dialog).queryByLabelText('周期秒数')).not.toBeInTheDocument();
    await selectSemiOption(user, dialog, '秒合约交易对', 'BTC-USDT（ID: 31）');
    await selectSemiOption(user, dialog, '押注资产', 'BNB - BNB（ID: 32）');
    await user.click(within(dialog).getByRole('tab', { name: '交易参数' }));
    semiInputByLabel(dialog, '周期秒数');
    semiInputByLabel(dialog, '赔率');
    semiInputByLabel(dialog, '最小押注');
    semiInputByLabel(dialog, '最大押注');
    expect(within(dialog).getByLabelText('最大押注')).toHaveAttribute('placeholder', '留空表示无上限');
    await user.type(within(dialog).getByLabelText('周期秒数'), '60');
    await user.type(within(dialog).getByLabelText('赔率'), '0.85');
    await user.type(within(dialog).getByLabelText('最小押注'), '10');
    await user.click(within(dialog).getByRole('button', { name: '新增周期' }));
    expect(within(dialog).getByRole('button', { name: '提交添加秒合约交易对' })).toBeDisabled();
    const durationInputs = within(dialog).getAllByLabelText('周期秒数');
    const payoutInputs = within(dialog).getAllByLabelText('赔率');
    const minStakeInputs = within(dialog).getAllByLabelText('最小押注');
    const maxStakeInputs = within(dialog).getAllByLabelText('最大押注');
    await user.type(durationInputs[1], '120');
    await user.type(payoutInputs[1], '0.9');
    await user.type(minStakeInputs[1], '20');
    await user.type(maxStakeInputs[1], '2000');
    expect(within(dialog).getByRole('button', { name: '提交添加秒合约交易对' })).not.toBeDisabled();
    await user.click(within(dialog).getByRole('button', { name: '提交添加秒合约交易对' }));
    await user.type(screen.getByLabelText('操作原因'), 'add seconds pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock.mock.calls.filter(([path]) => path === '/admin/api/v1/seconds-contracts/products')).toHaveLength(1);
    });
    const requests = apiRequestMock.mock.calls.filter(([path, init]) => path === '/admin/api/v1/seconds-contracts/products' && init && 'method' in init).map(([, init]) => JSON.parse(String(init?.body)));
    expect(requests[0]).toEqual({
      pair_id: 31,
      stake_asset: 32,
      cycles: [
        {
          duration_seconds: 60,
          payout_rate: '0.85',
          min_stake: '10'
        },
        {
          duration_seconds: 120,
          payout_rate: '0.9',
          min_stake: '20',
          max_stake: '2000'
        }
      ],
      status: 'active',
      reason: 'add seconds pair'
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
            from_asset_symbol: 'BTC',
            to_asset_id: 12,
            to_asset_symbol: 'USDT',
            pricing_mode: 'fixed',
            spread_rate: '0.01000000',
            fee_rate: '0.00100000',
            min_amount: '1.000000000000000000',
            max_amount: '100.000000000000000000',
            target_min_amount: '10.000000000000000000',
            target_max_amount: '1000.000000000000000000',
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
            invite_code: 'A1B2C3',
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
    let dialog = await findActionSheet('添加闪兑交易对');
    expectCreateModalSize(dialog, 'wide');
    semiSelectByLabel(dialog, '源资产');
    semiSelectByLabel(dialog, '目标资产');
    semiSelectByLabel(dialog, '定价模式');
    semiInputByLabel(dialog, '价差率');
    semiInputByLabel(dialog, '手续费率');
    semiInputByLabel(dialog, '源资产最小金额');
    semiInputByLabel(dialog, '源资产最大金额');
    semiInputByLabel(dialog, '目标资产最小金额');
    semiInputByLabel(dialog, '目标资产最大金额');
    semiSelectByLabel(dialog, '启用');
    expect(within(dialog).queryByRole('checkbox', { name: '同时创建反向交易对' })).not.toBeInTheDocument();
    expect(await screen.findByText('BTC')).toBeInTheDocument();
    expect(await screen.findByText('USDT')).toBeInTheDocument();
    await selectSemiOption(user, dialog, '源资产', 'BTC - Bitcoin（ID: 11）');
    await selectSemiOption(user, dialog, '目标资产', 'USDT - Tether（ID: 12）');
    await user.type(within(dialog).getByLabelText('价差率'), '0.01');
    await user.clear(within(dialog).getByLabelText('手续费率'));
    await user.type(within(dialog).getByLabelText('手续费率'), '0.001');
    await user.type(within(dialog).getByLabelText('源资产最小金额'), '1');
    await user.type(within(dialog).getByLabelText('源资产最大金额'), '100');
    await user.type(within(dialog).getByLabelText('目标资产最小金额'), '10');
    await user.type(within(dialog).getByLabelText('目标资产最大金额'), '1000');
    await user.click(within(dialog).getByRole('button', { name: '提交添加闪兑交易对' }));
    await confirmWithReason('create convert pair');

    await waitFor(() => {
      expect(apiRequestMock.mock.calls.filter(([path]) => path === '/admin/api/v1/convert/pairs')).toHaveLength(1);
    });
    const convertRequests = apiRequestMock.mock.calls.filter(([path, init]) => path === '/admin/api/v1/convert/pairs' && init && 'method' in init).map(([, init]) => JSON.parse(String(init?.body)));
    expect(convertRequests).toEqual([{
      from_asset_id: 11,
      to_asset_id: 12,
      pricing_mode: 'fixed',
      spread_rate: '0.01',
      fee_rate: '0.001',
      min_amount: '1',
      max_amount: '100',
      target_min_amount: '10',
      target_max_amount: '1000',
      enabled: true,
      reason: 'create convert pair'
    }]);
    unmount();

    const riskPage = render(<ResourcePage config={resourceConfigs.riskRules} />);
    await user.click(await screen.findByRole('button', { name: '添加风控规则' }));
    dialog = await findActionSheet('添加风控规则');
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
    let request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/risk/rules' && init && 'method' in init)?.[1];
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
    dialog = await findActionSheet('添加新币项目');
    expectCreateModalSize(dialog, 'extra-wide');
    semiSelectByLabel(dialog, '项目资产');
    semiSelectByLabel(dialog, '项目符号');
    expect(semiSelectByLabel(dialog, '生命周期')).toHaveTextContent('预热');
    semiInputByLabel(dialog, '发行总量');
    semiInputByLabel(dialog, '发行价');
    expect(semiSelectByLabel(dialog, '解禁类型')).toHaveTextContent('固定时间解禁');
    semiSelectByLabel(dialog, '启用解禁矿工费');
    await selectSemiOption(user, dialog, '生命周期', '申购中');
    await selectSemiOption(user, dialog, '生命周期', '预热');
    await selectSemiOption(user, dialog, '解禁类型', '上市即解禁');
    await selectSemiOption(user, dialog, '解禁类型', '固定时间解禁');
    await selectSemiOption(user, dialog, '项目资产', 'BTC - Bitcoin（ID: 11）');
    await selectSemiOption(user, dialog, '项目符号', 'BTC - Bitcoin（ID: 11）');
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
      symbol: 'BTC',
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
    expect(screen.getByText('A1B2C3')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
    await user.click(await screen.findByRole('button', { name: '添加用户' }));
    dialog = await findActionSheet('添加用户');
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
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/users')).toHaveLength(2);
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
    await openFiltersTab(user);
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
    const dialog = await findActionSheet('用户充值');
    expect(within(dialog).queryByLabelText('用户ID')).not.toBeInTheDocument();
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

  it('assigns an agent from user row actions with a required reason', async () => {
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
    await user.click(screen.getByRole('button', { name: '分配代理' }));
    const dialog = await findActionSheet('分配代理');
    semiInputByLabel(dialog, '用户ID');
    semiInputByLabel(dialog, '代理ID');
    await user.type(within(dialog).getByLabelText('代理ID'), '42');
    await user.click(within(dialog).getByRole('button', { name: '提交分配代理' }));
    await user.type(screen.getByLabelText('操作原因'), 'assign user agent');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/users/123/agent', {
        method: 'PATCH',
        body: JSON.stringify({ agent_id: 42, reason: 'assign user agent' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/users')).toHaveLength(2);
    });
  });

  it('resets user 2FA from user row actions with a required reason', async () => {
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
    await user.click(screen.getByRole('button', { name: '重置2FA' }));
    await user.type(screen.getByLabelText('操作原因'), 'reset user 2fa');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/users/123/2fa/reset', {
        method: 'POST',
        body: JSON.stringify({ reason: 'reset user 2fa' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/users')).toHaveLength(2);
    });
  });

  it('creates and updates convert-only agent commission rules with required reasons', async () => {
    const user = userEvent.setup();
    const config = resourceConfigs.agentCommissionRules;
    expect(config).toMatchObject({
      title: '佣金规则',
      endpoint: '/admin/api/v1/agent-commission-rules',
      responseKey: 'rules'
    });
    expectFilter(config, 'agent_id', { key: 'agent_id', label: '代理ID' });
    expectFilter(config, 'product_type', {
      key: 'product_type',
      label: '产品类型',
      type: 'select',
      options: [{ label: '闪兑', value: 'convert' }]
    });
    expectFilter(config, 'status', {
      key: 'status',
      label: '状态',
      type: 'select',
      options: [
        { label: '启用', value: 'active' },
        { label: '禁用', value: 'disabled' }
      ]
    });
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/agent-commission-rules') {
        const rows = [
          {
            id: 77,
            agent_id: 42,
            product_type: 'convert',
            commission_rate: '0.05000000',
            status: 'active',
            created_at: 1_775_027_600_000,
            updated_at: 1_775_027_700_000
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    const { unmount } = render(<ResourcePage config={config} />);

    expect(await screen.findByText('0.05')).toBeInTheDocument();
    expect(screen.getByText('更新时间')).toBeInTheDocument();
    await openFiltersTab(user);
    semiInputByLabel(document.body, '代理ID');
    semiSelectByLabel(document.body, '产品类型');
    semiSelectByLabel(document.body, '状态');
    await selectSemiOption(user, document.body, '产品类型', '闪兑');
    await selectSemiOption(user, document.body, '状态', '禁用');
    await user.click(screen.getByRole('button', { name: '查询' }));
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/api/v1/agent-commission-rules', 'rules', {
        product_type: 'convert',
        status: 'disabled'
      });
    });

    await user.click(screen.getByRole('button', { name: '添加佣金规则' }));
    const createDialog = await findActionSheet('添加佣金规则');
    semiInputByLabel(createDialog, '代理ID');
    semiSelectByLabel(createDialog, '产品类型');
    semiInputByLabel(createDialog, '佣金比例');
    semiSelectByLabel(createDialog, '初始状态');
    expect(semiSelectByLabel(createDialog, '产品类型')).toHaveTextContent('闪兑');
    await user.type(within(createDialog).getByLabelText('代理ID'), '42');
    await user.type(within(createDialog).getByLabelText('佣金比例'), '0.05');
    await user.click(within(createDialog).getByRole('button', { name: '提交添加佣金规则' }));
    await user.type(screen.getByLabelText('操作原因'), 'create commission rule');
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agent-commission-rules', expect.objectContaining({ method: 'POST' }));
    });
    const createRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/agent-commission-rules' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(createRequest?.body))).toEqual({
      agent_id: 42,
      product_type: 'convert',
      commission_rate: '0.05',
      status: 'active',
      reason: 'create commission rule'
    });

    unmount();
    render(<ResourcePage config={config} />);
    expect(await screen.findByText('0.05')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '修改' }));
    const editDialog = await findActionSheet('修改佣金规则');
    semiInputByLabel(editDialog, '代理ID');
    semiSelectByLabel(editDialog, '产品类型');
    semiInputByLabel(editDialog, '佣金比例');
    semiSelectByLabel(editDialog, '状态');
    await user.clear(within(editDialog).getByLabelText('佣金比例'));
    await user.type(within(editDialog).getByLabelText('佣金比例'), '0.08');
    await selectSemiOption(user, editDialog, '状态', '禁用');
    await user.click(within(editDialog).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'update commission rule');
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agent-commission-rules/77', expect.objectContaining({ method: 'PATCH' }));
    });
    const updateRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/agent-commission-rules/77' && init && 'method' in init)?.[1];
    expect(JSON.parse(String(updateRequest?.body))).toEqual({
      commission_rate: '0.08',
      status: 'disabled',
      reason: 'update commission rule'
    });
    expect(JSON.parse(String(updateRequest?.body))).not.toHaveProperty('agent_id');
    expect(JSON.parse(String(updateRequest?.body))).not.toHaveProperty('product_type');
  });

  it('settles and rejects agent commissions from row actions with a dropdown status filter', async () => {
    const user = userEvent.setup();
    const statusFilterConfig = resourceConfigs.agentCommissions.filters?.find((filter) => filter.key === 'status');
    expect(statusFilterConfig).toMatchObject({
      key: 'status',
      label: '状态',
      type: 'select',
      options: [
        { label: '待结算', value: 'pending' },
        { label: '已结算', value: 'settled' },
        { label: '已拒绝', value: 'rejected' }
      ]
    });
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/agent-commissions') {
        const rows = [
          {
            id: 88,
            agent_id: 42,
            user_id: 123,
            source_type: 'convert_order',
            source_id: 'quote-88',
            source_amount: '10.000000000000000000',
            commission_amount: '0.500000000000000000',
            status: 'pending',
            created_at: 1_775_027_600_000
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });

    render(<ResourcePage config={resourceConfigs.agentCommissions} />);

    expect(await screen.findByText('quote-88')).toBeInTheDocument();
    await openFiltersTab(user);
    semiSelectByLabel(document.body, '状态');
    await selectSemiOption(user, document.body, '状态', '待结算');
    await user.click(screen.getByRole('button', { name: '查询' }));
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/api/v1/agent-commissions', 'commissions', { status: 'pending' });
    });

    await user.click(screen.getByRole('button', { name: '结算' }));
    await user.type(screen.getByLabelText('操作原因'), 'settle commission');
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agent-commissions/88/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'settled', reason: 'settle commission' })
      });
    });

    await user.click(screen.getByRole('button', { name: '拒绝' }));
    await user.type(screen.getByLabelText('操作原因'), 'reject commission');
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/agent-commissions/88/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'rejected', reason: 'reject commission' })
      });
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

    await openFiltersTab(user);
    semiSelectByLabel(document.body, '交易对');
    semiSelectByLabel(document.body, '状态');
    semiSelectByLabel(document.body, '市场类型');
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
    const dialog = await findActionSheet('修改交易对配置');
    expect(within(dialog).queryByText('仅允许修改运营配置字段；交易对、基础资产、计价资产和状态保持只读。')).not.toBeInTheDocument();
    semiInputByLabel(dialog, '交易对');
    semiInputByLabel(dialog, '基础资产');
    semiInputByLabel(dialog, '计价资产');
    expect(within(dialog).getByLabelText('交易对')).toBeDisabled();
    expect(within(dialog).getByLabelText('基础资产')).toBeDisabled();
    expect(within(dialog).getByLabelText('计价资产')).toBeDisabled();
    semiSelectByLabel(dialog, '当前状态');
    semiInputByLabel(dialog, '价格精度');
    semiInputByLabel(dialog, '数量精度');
    semiInputByLabel(dialog, '最小下单额');
    semiSelectByLabel(dialog, '市场类型');
    expect(semiSelectByLabel(dialog, '当前状态')).toHaveTextContent('启用');
    await selectSemiOption(user, dialog, '当前状态', '禁用');
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
      status: 'disabled',
      market_type: 'strategy',
      reason: 'adjust pair config'
    });
    expect(body).not.toHaveProperty('symbol');
    expect(body).not.toHaveProperty('base_asset_id');
    expect(body).not.toHaveProperty('quote_asset_id');
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
            user_email: 'spot-user@example.test',
            pair_id: 'BTC-USDT',
            side: 'buy',
            order_type: 'limit',
            price: '100.0000',
            average_price: '99.5000',
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
    expect(screen.getByText('99.50')).toBeInTheDocument();
    expect(screen.getByText('spot-user@example.test')).toBeInTheDocument();
    expect(screen.getByText('买入')).toBeInTheDocument();
    expect(screen.getByText('限价单')).toBeInTheDocument();
    expect(screen.getByText('当前委托')).toBeInTheDocument();
    expect(screen.getByText('成交价')).toBeInTheDocument();
    expect(screen.getByText('已成交数量')).toBeInTheDocument();

    await openFiltersTab(user);
    semiSelectByLabel(document.body, '交易对');
    semiSelectByLabel(document.body, '状态');
    await selectSemiOption(user, document.body, '交易对', 'BTC-USDT');
    await selectSemiOption(user, document.body, '状态', '当前委托');
    expect(screen.getByLabelText('显示机器人订单').closest('.admin-resource-head-switch')).toBeInTheDocument();
    await user.click(screen.getByLabelText('显示机器人订单'));
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/api/v1/spot/orders', 'orders', {
        include_internal: 'true'
      });
    });
    await waitFor(() => {
      expect(screen.getByLabelText('显示机器人订单')).not.toBeDisabled();
    });
    await user.click(screen.getByRole('button', { name: '查询' }));

    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/api/v1/spot/orders', 'orders', {
        pair_id: 'BTC-USDT',
        status: 'open',
        include_internal: 'true'
      });
    });

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
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/spot/orders')).toHaveLength(4);
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
            margin_asset: 12,
            margin_asset_symbol: 'USDT',
            logo_url: 'https://cdn.example.test/margin/btc-usdt.png',
            margin_mode: 'isolated',
            margin_modes: ['isolated'],
            leverage_levels: ['2', '5'],
            max_leverage: '5.00000000',
            min_margin: '10.0000',
            max_margin: '100.0000',
            maintenance_margin_rate: '0.05000000',
            hourly_interest_rate: '0.00010000',
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
    expect(screen.queryByRole('columnheader', { name: '产品ID' })).not.toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: '交易对ID' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/products/14');
    });
    await expectFormattedDetail('margin-product-detail', /"detail": "margin-product-detail"/);

    await user.click(screen.getByRole('button', { name: '修改' }));
    const editSheet = await findActionSheet('修改杠杆产品');
    expectCreateModalSize(editSheet, 'extra-wide');
    expect(within(editSheet).getByRole('tab', { name: '基础配置' })).toBeInTheDocument();
    semiSelectByLabel(editSheet, '杠杆交易对');
    semiSelectByLabel(editSheet, '保证金资产');
    semiSelectByLabel(editSheet, '支持保证金模式');
    semiSelectByLabel(editSheet, '状态');
    await selectSemiOption(user, editSheet, '支持保证金模式', '全仓');
    await selectSemiOption(user, editSheet, '状态', '禁用');
    await user.click(within(editSheet).getByRole('tab', { name: '杠杆档位' }));
    expect(within(editSheet).getByLabelText('2x')).toBeChecked();
    expect(within(editSheet).getByLabelText('5x')).toBeChecked();
    await user.click(within(editSheet).getByLabelText('10x'));
    await user.type(within(editSheet).getByLabelText('自定义杠杆档位'), '25');
    expect(within(editSheet).getByText('已选杠杆：2x / 5x / 10x / 25x')).toBeInTheDocument();
    await user.click(within(editSheet).getByRole('tab', { name: '风控参数' }));
    await user.clear(within(editSheet).getByLabelText('最小保证金'));
    await user.type(within(editSheet).getByLabelText('最小保证金'), '20');
    await user.clear(within(editSheet).getByLabelText('最大保证金'));
    await user.type(within(editSheet).getByLabelText('最大保证金'), '200');
    await user.clear(within(editSheet).getByLabelText('维持保证金率'));
    await user.type(within(editSheet).getByLabelText('维持保证金率'), '0.04');
    await user.clear(within(editSheet).getByLabelText('小时利率'));
    await user.type(within(editSheet).getByLabelText('小时利率'), '0.0002');
    await user.click(within(editSheet).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'update margin product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/products/14', expect.objectContaining({ method: 'PATCH' }));
    });
    const editRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/margin/products/14' && init?.method === 'PATCH')?.[1];
    expect(JSON.parse(String(editRequest?.body))).toEqual({
      pair_id: 1,
      margin_asset: 12,
      logo_url: 'https://cdn.example.test/margin/btc-usdt.png',
      margin_modes: ['isolated', 'cross'],
      leverage_levels: ['2', '5', '10', '25'],
      max_leverage: '25',
      min_margin: '20',
      max_margin: '200',
      maintenance_margin_rate: '0.04',
      hourly_interest_rate: '0.0002',
      status: 'disabled',
      reason: 'update margin product'
    });

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable margin product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/margin/products/14/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable margin product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/margin/products')).toHaveLength(3);
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

  it('opens seconds contract product details, edits products, and updates product status from row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/market-pairs') {
        const rows = [
          { id: 1, symbol: 'ETH-USDT', status: 'active' },
          { id: 31, symbol: 'BTC-USDT', status: 'active' }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      if (endpoint === '/admin/api/v1/seconds-contracts/products') {
        const rows = [
          {
            id: 41,
            pair_id: 1,
            symbol: 'ETH-USDT',
            stake_asset: 12,
            stake_asset_symbol: 'USDT',
            duration_seconds: 60,
            payout_rate: '0.85000000',
            min_stake: '10.0000',
            max_stake: '1000.0000',
            cycles: [
              {
                id: 4101,
                product_id: 41,
                duration_seconds: 60,
                payout_rate: '0.85000000',
                min_stake: '10.0000',
                max_stake: '1000.0000',
                sort_order: 0
              },
              {
                id: 4102,
                product_id: 41,
                duration_seconds: 120,
                payout_rate: '0.90000000',
                min_stake: '20.0000',
                max_stake: null,
                sort_order: 1
              }
            ],
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
    expect(screen.queryByRole('columnheader', { name: '产品ID' })).not.toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: '交易对ID' })).not.toBeInTheDocument();
    expect(screen.getByText(/1,000\.00/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/products/41');
    });
    await expectFormattedDetail('seconds-product-detail', /"detail": "seconds-product-detail"/);

    await user.click(screen.getByRole('button', { name: '修改' }));
    const editDialog = await findActionSheet('修改秒合约产品');
    expectCreateModalSize(editDialog, 'wide');
    expect(within(editDialog).getByRole('tab', { name: '基础配置' })).toBeInTheDocument();
    expect(within(editDialog).getByRole('tab', { name: '交易参数' })).toBeInTheDocument();
    semiInputByLabel(editDialog, '产品ID');
    semiSelectByLabel(editDialog, '秒合约交易对');
    semiSelectByLabel(editDialog, '押注资产');
    semiSelectByLabel(editDialog, '状态');
    await user.click(within(editDialog).getByRole('tab', { name: '交易参数' }));
    semiInputByLabel(editDialog, '周期秒数');
    semiInputByLabel(editDialog, '赔率');
    semiInputByLabel(editDialog, '最小押注');
    semiInputByLabel(editDialog, '最大押注');
    const editDurationInputs = within(editDialog).getAllByLabelText('周期秒数');
    const editPayoutInputs = within(editDialog).getAllByLabelText('赔率');
    const editMinStakeInputs = within(editDialog).getAllByLabelText('最小押注');
    const editMaxStakeInputs = within(editDialog).getAllByLabelText('最大押注');
    expect(editDurationInputs).toHaveLength(2);
    expect(editMaxStakeInputs[0]).toHaveAttribute('placeholder', '留空表示无上限');
    await user.clear(editDurationInputs[0]);
    await user.type(editDurationInputs[0], '180');
    await user.clear(editPayoutInputs[0]);
    await user.type(editPayoutInputs[0], '0.95');
    await user.clear(editMinStakeInputs[0]);
    await user.type(editMinStakeInputs[0], '30');
    await user.clear(editMaxStakeInputs[0]);
    await user.click(within(editDialog).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'edit seconds product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/products/41', expect.objectContaining({ method: 'PATCH' }));
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/seconds-contracts/products')).toHaveLength(2);
    });
    const editRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/seconds-contracts/products/41' && init && 'method' in init)?.[1];
    expect(editRequest).toBeDefined();
    const editBody = JSON.parse(String(editRequest?.body));
    expect(editBody).toEqual({
      pair_id: 1,
      stake_asset: 12,
      cycles: [
        {
          duration_seconds: 180,
          payout_rate: '0.95',
          min_stake: '30'
        },
        {
          duration_seconds: 120,
          payout_rate: '0.90000000',
          min_stake: '20.0000'
        }
      ],
      status: 'disabled',
      reason: 'edit seconds product'
    });

    await user.click(screen.getByRole('button', { name: '启用' }));
    await user.type(screen.getByLabelText('操作原因'), 'enable seconds product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/products/41/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'active', reason: 'enable seconds product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/seconds-contracts/products')).toHaveLength(3);
    });

    await user.click(screen.getByRole('button', { name: '删除' }));
    await user.type(screen.getByLabelText('操作原因'), 'delete disabled seconds product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/seconds-contracts/products/41', {
        method: 'DELETE',
        body: JSON.stringify({ reason: 'delete disabled seconds product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/seconds-contracts/products')).toHaveLength(4);
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
            email: 'seconds-user@example.test',
            symbol: 'BTC-USDT',
            direction: 'up',
            stake_amount: '10.0000',
            entry_price: '100.0000',
            settlement_price: null,
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

    expect(await screen.findByRole('columnheader', { name: '用户邮箱' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: '交易对' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: '结算价格' })).toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: '订单ID' })).not.toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: '用户ID' })).not.toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: '产品ID' })).not.toBeInTheDocument();
    expect(screen.getByText('seconds-user@example.test')).toBeInTheDocument();
    expect(screen.getByText('BTC-USDT')).toBeInTheDocument();
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
            email: 'seconds-user@example.test',
            symbol: 'ETH-USDT',
            direction: 'down',
            stake_amount: '12.0000',
            entry_price: '100.0000',
            settlement_price: '98.5000',
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

  it('creates earn category columns with multilingual names, then supports row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/countries') {
        const rows = [
          { id: 1, country_code: 'CN', country_name: '中国', default_locale: 'zh-CN', status: 'active' },
          { id: 2, country_code: 'US', country_name: 'United States', default_locale: 'en-US', status: 'active' }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      if (endpoint === '/admin/api/v1/earn/categories') {
        const rows = [
          {
            id: 501,
            code: 'stable',
            default_name: '稳健栏目',
            name_json: {
              version: 1,
              default_locale: 'zh-CN',
              items: [
                { locale: 'zh-CN', country: 'CN', title: '稳健栏目' },
                { locale: 'en-US', country: 'US', title: 'Stable Earn' }
              ]
            },
            sort_order: 7,
            status: 'active'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/earn/categories/501') {
        return { id: 501, detail: 'earn-category-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.earnCategories} />);

    expect(await screen.findByText('稳健栏目')).toBeInTheDocument();
    expect(screen.getByText('zh-CN: 稳健栏目 / en-US: Stable Earn')).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: '默认栏目名' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '添加理财分类' })).toBeInTheDocument();
    const initialCategoryLoadCount = listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/categories').length;

    await user.click(screen.getByRole('button', { name: '添加理财分类' }));
    const dialog = await findActionSheet('添加理财分类');
    expectCreateModalSize(dialog, 'wide');
    semiInputByLabel(dialog, '分类代码');
    semiInputByLabel(dialog, '排序值');
    semiSelectByLabel(dialog, '状态');
    semiSelectByLabel(dialog, '国家');
    semiInputByLabel(dialog, '栏目名称');
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/api/v1/countries', 'countries', expect.objectContaining({ status: 'active' }));
      expect(semiSelectByLabel(dialog, '国家')).toHaveTextContent('中国 (CN / zh-CN)');
    });
    await user.type(within(dialog).getByLabelText('分类代码'), 'premium');
    await user.clear(within(dialog).getByLabelText('排序值'));
    await user.type(within(dialog).getByLabelText('排序值'), '3');
    await user.type(within(dialog).getByLabelText('栏目名称'), '精品理财');
    await user.click(within(dialog).getByRole('button', { name: '新增国家名称' }));
    await selectSemiOption(user, dialog, '国家', 'United States (US / en-US)', 1);
    const categoryNameInputs = within(dialog).getAllByLabelText('栏目名称');
    await user.type(categoryNameInputs[1], 'Premium Earn');
    const submitCategoryButton = within(dialog).getByRole('button', { name: '提交添加理财分类' });
    await waitFor(() => {
      expect(submitCategoryButton).not.toBeDisabled();
    });
    await user.click(submitCategoryButton);
    await user.type(screen.getByLabelText('操作原因'), 'add earn category');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/categories', expect.objectContaining({ method: 'POST' }));
    });
    const createRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/earn/categories' && init && 'method' in init)?.[1];
    const createBody = JSON.parse(String(createRequest?.body));
    expect(createBody).toMatchObject({
      code: 'premium',
      sort_order: 3,
      status: 'active',
      reason: 'add earn category'
    });
    expect(createBody.name_json).toMatchObject({
      version: 1,
      default_locale: 'zh-CN',
      items: [
        { locale: 'zh-CN', country: 'CN', title: '精品理财' },
        { locale: 'en-US', country: 'US', title: 'Premium Earn' }
      ]
    });
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/categories')).toHaveLength(initialCategoryLoadCount + 1);

    await user.click(screen.getByRole('button', { name: '查看详情' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/categories/501');
    });
    await expectFormattedDetail('earn-category-detail', /"detail": "earn-category-detail"/);

    await user.click(screen.getByRole('button', { name: '修改' }));
    const editDialog = await findActionSheet('修改理财分类');
    const editCodeInput = within(editDialog).getByLabelText('分类代码');
    expect(editCodeInput).toBeDisabled();
    await user.clear(within(editDialog).getAllByLabelText('栏目名称')[0]);
    await user.type(within(editDialog).getAllByLabelText('栏目名称')[0], '稳健精选');
    await user.click(within(editDialog).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'update earn category');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/categories/501', expect.objectContaining({ method: 'PATCH' }));
    });
    const updateRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/earn/categories/501' && init && 'method' in init && init.method === 'PATCH')?.[1];
    const updateBody = JSON.parse(String(updateRequest?.body));
    expect(updateBody).toMatchObject({
      sort_order: 7,
      status: 'active',
      reason: 'update earn category'
    });
    expect(updateBody.name_json.items[0]).toMatchObject({ locale: 'zh-CN', country: 'CN', title: '稳健精选' });

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable earn category');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/categories/501/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable earn category' })
      });
    });
  });

  it('creates earn products with category and multilingual rich text, then supports row actions', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: assetRows, raw: { [responseKey]: assetRows } };
      }

      if (endpoint === '/admin/api/v1/countries') {
        const rows = [
          { id: 1, country_code: 'CN', country_name: '中国', default_locale: 'zh-CN', status: 'active' },
          { id: 2, country_code: 'US', country_name: 'United States', default_locale: 'en-US', status: 'active' }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      if (endpoint === '/admin/api/v1/earn/categories') {
        const rows = [
          {
            id: 501,
            code: 'fixed_term',
            default_name: '定期',
            name_json: { version: 1, default_locale: 'zh-CN', items: [{ locale: 'zh-CN', country: 'CN', title: '定期' }] },
            sort_order: 10,
            status: 'active'
          },
          {
            id: 502,
            code: 'structured',
            default_name: '结构化',
            name_json: { version: 1, default_locale: 'zh-CN', items: [{ locale: 'zh-CN', country: 'CN', title: '结构化' }] },
            sort_order: 20,
            status: 'active'
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      if (endpoint === '/admin/api/v1/earn/products') {
        const rows = [
          {
            id: 61,
            asset_id: 12,
            asset_symbol: 'USDT',
            name: 'USDT 30D',
            banner_url: 'https://static.example.com/earn-banner.png',
            small_logo_url: 'https://static.example.com/earn-logo.png',
            category: 'fixed_term',
            category_name: '定期',
            category_name_json: {
              version: 1,
              default_locale: 'zh-CN',
              items: [{ locale: 'zh-CN', country: 'CN', title: '定期' }]
            },
            introduction_json: {
              version: 1,
              default_locale: 'zh-CN',
              items: [
                {
                  locale: 'zh-CN',
                  country: 'CN',
                  title: 'USDT 定期',
                  content: [{ type: 'p', children: [{ text: '定期理财说明。' }] }]
                }
              ]
            },
            term_days: 30,
            apr_rate: '0.12000000',
            redemption_fee_rate: '0.01000000',
            maturity_profit_fee_rate: '0.10000000',
            early_redeem_fee_basis: 'principal',
            early_redeem_fee_rate: '0.02000000',
            min_subscribe: '10.0000',
            max_subscribe: '1000.0000',
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
    const dialog = await findActionSheet('添加理财产品');
    expectCreateModalSize(dialog, 'extra-wide');
    const earnProductLayout = dialog.querySelector('.admin-earn-product-layout') as HTMLElement | null;
    expect(earnProductLayout).toBeInTheDocument();
    expect(getComputedStyle(earnProductLayout as HTMLElement).display).toBe('grid');
    expect(dialog.querySelector('.admin-earn-product-basic-grid')).toBeInTheDocument();
    expect(dialog.querySelector('.admin-earn-introduction-card')).toBeInTheDocument();
    expect(dialog.querySelector('.admin-earn-introduction-meta')).toBeInTheDocument();
    expect(dialog.querySelector('.admin-earn-product-footer')).toBeInTheDocument();
    expect(dialog.querySelector('.admin-earn-category-descriptions')).not.toBeInTheDocument();
    expect(within((earnProductLayout as HTMLElement).children[0] as HTMLElement).getByText('基础信息')).toBeInTheDocument();
    expect(within((earnProductLayout as HTMLElement).children[1] as HTMLElement).getByText('收益与申购参数')).toBeInTheDocument();
    expect(within((earnProductLayout as HTMLElement).children[2] as HTMLElement).getByText('手续费配置')).toBeInTheDocument();
    semiSelectByLabel(dialog, '理财资产');
    semiSelectByLabel(dialog, '产品分类');
    semiSelectByLabel(dialog, '初始状态');
    semiInputByLabel(dialog, '产品名称');
    semiInputByLabel(dialog, '期限天数');
    semiInputByLabel(dialog, '年化利率');
    semiInputByLabel(dialog, '提现赎回手续费率');
    semiInputByLabel(dialog, '到期获利手续费率');
    semiSelectByLabel(dialog, '提前赎回扣费基准');
    semiInputByLabel(dialog, '提前赎回扣费率');
    semiInputByLabel(dialog, '最小申购');
    semiInputByLabel(dialog, '最大申购');
    semiSelectByLabel(dialog, '国家');
    expect(within(dialog).queryByLabelText('语言')).not.toBeInTheDocument();
    semiInputByLabel(dialog, '介绍标题');
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/api/v1/countries', 'countries', expect.objectContaining({ status: 'active' }));
      expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/api/v1/earn/categories', 'categories', expect.objectContaining({ status: 'active' }));
      expect(semiSelectByLabel(dialog, '国家')).toHaveTextContent('中国 (CN / zh-CN)');
    });
    await selectSemiOption(user, dialog, '理财资产', 'USDT - Tether（ID: 12）');
    expect(semiSelectByLabel(dialog, '理财资产')).toHaveTextContent('USDT - Tether（ID: 12）');
    await user.type(within(dialog).getByLabelText('产品名称'), 'USDT 稳健理财');
    await selectSemiOption(user, dialog, '产品分类', '结构化（structured）');
    expect(semiSelectByLabel(dialog, '产品分类')).toHaveTextContent('结构化（structured）');
    await user.type(within(dialog).getByLabelText('期限天数'), '30');
    await user.type(within(dialog).getByLabelText('年化利率'), '0.12');
    await user.clear(within(dialog).getByLabelText('提现赎回手续费率'));
    await user.type(within(dialog).getByLabelText('提现赎回手续费率'), '0.01');
    await user.clear(within(dialog).getByLabelText('到期获利手续费率'));
    await user.type(within(dialog).getByLabelText('到期获利手续费率'), '0.1');
    await selectSemiOption(user, dialog, '提前赎回扣费基准', '按本金比例扣除');
    await user.clear(within(dialog).getByLabelText('提前赎回扣费率'));
    await user.type(within(dialog).getByLabelText('提前赎回扣费率'), '0.02');
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
    await user.click(within(dialog).getByRole('button', { name: '新增国家介绍' }));
    await selectSemiOption(user, dialog, '国家', 'United States (US / en-US)', 1);
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
      redemption_fee_rate: '0.01',
      maturity_profit_fee_rate: '0.1',
      early_redeem_fee_basis: 'principal',
      early_redeem_fee_rate: '0.02',
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

    await user.click(screen.getByRole('button', { name: '修改' }));
    const editDialog = await findActionSheet('修改理财产品');
    semiSelectByLabel(editDialog, '理财资产');
    semiSelectByLabel(editDialog, '产品分类');
    semiSelectByLabel(editDialog, '状态');
    semiSelectByLabel(editDialog, '提前赎回扣费基准');
    semiSelectByLabel(editDialog, '国家');
    expect(within(editDialog).queryByLabelText('语言')).not.toBeInTheDocument();
    await waitFor(() => {
      expect(semiSelectByLabel(editDialog, '国家')).toHaveTextContent('中国 (CN / zh-CN)');
    });
    await user.clear(within(editDialog).getByLabelText('产品名称'));
    await user.type(within(editDialog).getByLabelText('产品名称'), 'USDT 90D');
    await user.clear(within(editDialog).getByLabelText('期限天数'));
    await user.type(within(editDialog).getByLabelText('期限天数'), '90');
    await selectSemiOption(user, editDialog, '提前赎回扣费基准', '按收益比例扣除');
    await user.clear(within(editDialog).getByLabelText('提前赎回扣费率'));
    await user.type(within(editDialog).getByLabelText('提前赎回扣费率'), '0.03');
    await selectSemiOption(user, editDialog, '国家', 'United States (US / en-US)');
    await user.clear(within(editDialog).getByLabelText('介绍标题'));
    await user.type(within(editDialog).getByLabelText('介绍标题'), 'USDT Earn Updated');
    await user.click(within(editDialog).getByRole('button', { name: '提交修改' }));
    await user.type(screen.getByLabelText('操作原因'), 'update earn product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products/61', expect.objectContaining({ method: 'PATCH' }));
    });
    const updateRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/earn/products/61' && init && 'method' in init)?.[1];
    const updateBody = JSON.parse(String(updateRequest?.body));
    expect(updateBody).toMatchObject({
      asset_id: 12,
      name: 'USDT 90D',
      category: 'fixed_term',
      term_days: 90,
      apr_rate: '0.12000000',
      redemption_fee_rate: '0.01000000',
      maturity_profit_fee_rate: '0.10000000',
      early_redeem_fee_basis: 'profit',
      early_redeem_fee_rate: '0.03',
      min_subscribe: '10.0000',
      max_subscribe: '1000.0000',
      status: 'active',
      reason: 'update earn product'
    });
    expect(updateBody.introduction_json).toMatchObject({
      version: 1,
      default_locale: 'en-US',
      items: [{ locale: 'en-US', country: 'US', title: 'USDT Earn Updated' }]
    });
    expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/products')).toHaveLength(initialEarnProductLoadCount + 2);

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable earn product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products/61/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable earn product' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/earn/products')).toHaveLength(initialEarnProductLoadCount + 3);
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

  it('opens convert pair details, updates enabled state, and deletes from row actions', async () => {
    const user = userEvent.setup();
    let convertPairEnabled = true;
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/convert/pairs') {
        const rows = [
          {
            id: 71,
            from_asset_id: 11,
            from_asset_symbol: 'BTC',
            to_asset_id: 12,
            to_asset_symbol: 'USDT',
            pricing_mode: 'fixed',
            spread_rate: '0.01000000',
            fee_rate: '0.00100000',
            min_amount: '1.0000',
            max_amount: '100.0000',
            target_min_amount: '10.0000',
            target_max_amount: '1000.0000',
            enabled: convertPairEnabled
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockImplementation(async (path, init) => {
      if (path === '/admin/api/v1/convert/pairs/71') {
        if (init && 'method' in init && init.method === 'PATCH') {
          convertPairEnabled = false;
        }
        return { id: 71, detail: 'convert-pair-detail' };
      }

      return {};
    });

    render(<ResourcePage config={resourceConfigs.convertPairs} />);

    expect(await screen.findByText('0.01')).toBeInTheDocument();
    expect(screen.getByText('BTC')).toBeInTheDocument();
    expect(screen.getByText('USDT')).toBeInTheDocument();
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

    await user.click(screen.getByRole('button', { name: '删除' }));
    await user.type(screen.getByLabelText('操作原因'), 'delete convert pair');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/convert/pairs/71', {
        method: 'DELETE',
        body: JSON.stringify({ reason: 'delete convert pair' })
      });
      expect(listAdminResourceMock.mock.calls.filter(([endpoint]) => endpoint === '/admin/api/v1/convert/pairs')).toHaveLength(3);
    });
  });

  it('edits convert pair configuration from row actions', async () => {
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
            id: 71,
            from_asset_id: 11,
            from_asset_symbol: 'BTC',
            to_asset_id: 12,
            to_asset_symbol: 'USDT',
            pricing_mode: 'fixed',
            spread_rate: '0.01000000',
            fee_rate: '0.00100000',
            min_amount: '1.0000',
            max_amount: '100.0000',
            target_min_amount: '10.0000',
            target_max_amount: '1000.0000',
            enabled: true
          }
        ];
        return { rows, raw: { [responseKey]: rows } };
      }

      return { rows: [], raw: {} };
    });
    apiRequestMock.mockResolvedValue({});

    render(<ResourcePage config={resourceConfigs.convertPairs} />);

    expect(await screen.findByText('BTC')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '修改' }));
    const dialog = await findActionSheet('修改闪兑交易对');
    expectCreateModalSize(dialog, 'wide');
    semiSelectByLabel(dialog, '源资产');
    semiSelectByLabel(dialog, '目标资产');
    semiSelectByLabel(dialog, '定价模式');
    semiInputByLabel(dialog, '价差率');
    semiInputByLabel(dialog, '手续费率');
    semiInputByLabel(dialog, '源资产最小金额');
    semiInputByLabel(dialog, '源资产最大金额');
    semiInputByLabel(dialog, '目标资产最小金额');
    semiInputByLabel(dialog, '目标资产最大金额');
    semiSelectByLabel(dialog, '启用');

    await selectSemiOption(user, dialog, '源资产', 'ETH - Ethereum（ID: 22）');
    await selectSemiOption(user, dialog, '定价模式', '市场价格');
    await user.clear(within(dialog).getByLabelText('价差率'));
    await user.type(within(dialog).getByLabelText('价差率'), '0.02');
    await user.clear(within(dialog).getByLabelText('手续费率'));
    await user.type(within(dialog).getByLabelText('手续费率'), '0.003');
    await user.clear(within(dialog).getByLabelText('源资产最小金额'));
    await user.type(within(dialog).getByLabelText('源资产最小金额'), '2');
    await user.clear(within(dialog).getByLabelText('源资产最大金额'));
    await user.clear(within(dialog).getByLabelText('目标资产最小金额'));
    await user.type(within(dialog).getByLabelText('目标资产最小金额'), '20');
    await user.clear(within(dialog).getByLabelText('目标资产最大金额'));
    await user.click(within(dialog).getByRole('button', { name: '提交修改' }));
    await confirmWithReason('edit convert pair');

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/convert/pairs/71', {
        method: 'PATCH',
        body: JSON.stringify({
          from_asset_id: 22,
          to_asset_id: 12,
          pricing_mode: 'market',
          spread_rate: '0.02',
          fee_rate: '0.003',
          min_amount: '2',
          max_amount: null,
          target_min_amount: '20',
          target_max_amount: null,
          enabled: true,
          reason: 'edit convert pair'
        })
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
    const createDialog = await findActionSheet('创建策略');
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
    const editDialog = await findActionSheet('修改行情策略');
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
            user_email: 'convert-user@example.test',
            from_asset_symbol: 'USDT',
            to_asset_symbol: 'BTC',
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

    expect(await screen.findByText('convert-user@example.test')).toBeInTheDocument();
    expect(screen.getByText('USDT')).toBeInTheDocument();
    expect(screen.getByText('BTC')).toBeInTheDocument();
    expect(screen.queryByText('quote-72')).not.toBeInTheDocument();
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
