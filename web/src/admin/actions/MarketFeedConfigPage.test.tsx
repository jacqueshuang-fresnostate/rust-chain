import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { MarketFeedConfigPage } from './MarketFeedConfigPage';
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

const originalResizeObserver = globalThis.ResizeObserver;

function statusResponse(overrides = {}) {
  return {
    saved_config: {
      applied_version: 1,
      enabled: true,
      id: 7,
      intervals: ['1m', '5m'],
      last_reloaded_at: 1_735_732_800_000,
      last_reload_error: null,
      last_reload_status: 'success',
      name: 'default',
      needs_reload: false,
      providers: ['bitget', 'htx', 'coinbase'],
      symbols: ['BTCUSDT', 'ETHUSDT'],
      version: 1,
      ...overrides
    },
    runtime: {
      applied_version: 1,
      intervals: ['1m', '5m'],
      last_reload_error: null,
      last_reload_status: 'success',
      providers: ['bitget', 'htx', 'coinbase'],
      symbols: ['BTCUSDT', 'ETHUSDT']
    }
  };
}

const credentialsResponse = {
  credentials: [{ api_key_mask: 'abcd****wxyz', auth_type: 'api_key', enabled: true, provider: 'bitget' }]
};

function semiSelectByLabel(label: string): HTMLElement {
  const labelNode = screen
    .getAllByText(label)
    .map((node) => node.closest('label') as HTMLElement | null)
    .find((node) => node?.querySelector('.semi-select'));
  expect(labelNode).toBeInTheDocument();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

async function selectSemiOption(user: ReturnType<typeof userEvent.setup>, label: string, optionLabel: string) {
  await user.click(semiSelectByLabel(label));
  await waitFor(() => {
    expect([...document.querySelectorAll('.semi-select-option')].some((option) => option.textContent === optionLabel)).toBe(true);
  });
  const option = [...document.querySelectorAll('.semi-select-option')].find((item) => item.textContent === optionLabel) as HTMLElement | undefined;
  expect(option).toBeDefined();
  fireEvent.mouseEnter(option as HTMLElement);
  fireEvent.mouseDown(option as HTMLElement);
  fireEvent.mouseUp(option as HTMLElement);
  fireEvent.click(option as HTMLElement);
}

describe('MarketFeedConfigPage', () => {
  beforeEach(() => {
    if (!globalThis.ResizeObserver) {
      Object.defineProperty(globalThis, 'ResizeObserver', {
        configurable: true,
        value: ResizeObserverMock
      });
    }
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/market-feed/status' && !init?.method) {
        return Promise.resolve(statusResponse());
      }
      if (path === '/admin/api/v1/market-feed/credentials' && !init?.method) {
        return Promise.resolve(credentialsResponse);
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    if (!originalResizeObserver) {
      Reflect.deleteProperty(globalThis, 'ResizeObserver');
    }
  });

  it('loads saved config, runtime status, masked credentials, and selectable feed options without static helper copy', async () => {
    const user = userEvent.setup();

    render(<MarketFeedConfigPage />);

    const symbolsInput = await screen.findByDisplayValue('BTCUSDT,ETHUSDT');
    expect(symbolsInput).toBeInTheDocument();
    expect(symbolsInput.closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('启用状态');
    expect(screen.getByRole('tab', { name: '订阅配置' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '运行状态' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Provider 凭证' })).toBeInTheDocument();
    expect(screen.queryByText('配置第三方行情 symbols、intervals、providers 和 API Key；保存后需手动重载才会生效。')).not.toBeInTheDocument();
    expect(screen.queryByText('交易对支持逗号分隔输入；K 线周期和行情源可多选，保存后需手动重载。')).not.toBeInTheDocument();
    expect(screen.queryByText('保存配置不会立即影响 worker，只有手动重载会更新运行态。')).not.toBeInTheDocument();
    expect(screen.queryByText('API Key、Secret、Passphrase 只会加密提交；页面和接口仅展示 Key 掩码。')).not.toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: '1m' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '5m' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '15m' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'bitget' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'htx' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'coinbase' })).not.toBeChecked();

    await user.click(screen.getByRole('tab', { name: '运行状态' }));

    expect(screen.getByTestId('runtime-providers')).toHaveTextContent('当前启动 providers');
    expect(screen.getByTestId('runtime-providers')).toHaveTextContent('Bitget 行情');
    expect(screen.getByTestId('runtime-providers')).toHaveTextContent('HTX 行情');
    expect(screen.getByTestId('runtime-providers')).toHaveTextContent('Coinbase 行情');
    expect(screen.getAllByText('成功').length).toBeGreaterThan(0);
    expect(screen.getByText(/运行交易对/)).toBeInTheDocument();
    expect(screen.getByText('BTCUSDT,ETHUSDT')).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: 'Provider 凭证' }));

    expect(semiSelectByLabel('行情源')).toHaveTextContent('Bitget 行情');
    expect(semiSelectByLabel('鉴权方式')).toHaveTextContent('API Key 鉴权');
    expect(screen.getByLabelText('API Key').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('API Secret').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('Passphrase').closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('凭证状态');
    const credentialList = screen.getByRole('grid', { name: '行情源凭证列表' });
    expect(within(credentialList).getByRole('columnheader', { name: '行情源' })).toBeInTheDocument();
    expect(within(credentialList).getByRole('columnheader', { name: 'Key 掩码' })).toBeInTheDocument();
    expect(within(credentialList).getByText('Bitget 行情')).toBeInTheDocument();
    expect(within(credentialList).getByText('abcd****wxyz')).toBeInTheDocument();
  });

  it('renders market feed subscriptions as a toggleable list', async () => {
    const user = userEvent.setup();

    render(<MarketFeedConfigPage />);

    const list = await screen.findByRole('grid', { name: '行情订阅列表' });
    const tableWrapper = list.closest('.semi-table-wrapper');
    expect(list.closest('.semi-table-bordered')).toBeInTheDocument();
    expect(tableWrapper).not.toHaveClass('admin-action-subscription-list');
    expect(tableWrapper).toHaveStyle({ maxWidth: '100%', width: '100%' });
    expect(list.querySelector('.react-resizable-handle')).toBeInTheDocument();
    expect(document.querySelector('.admin-action-subscription-list')).not.toBeInTheDocument();
    expect(within(list).getByRole('columnheader', { name: '类型' })).toBeInTheDocument();
    expect(within(list).getByRole('columnheader', { name: '订阅项' })).toBeInTheDocument();
    expect(within(list).getByRole('columnheader', { name: '配置态' })).toBeInTheDocument();
    expect(within(list).getByRole('columnheader', { name: '运行态' })).toBeInTheDocument();
    expect(within(list).getAllByRole('gridcell', { name: '行情源' })).toHaveLength(3);
    expect(within(list).getAllByRole('gridcell', { name: '交易对' })).toHaveLength(2);
    expect(within(list).getAllByRole('gridcell', { name: 'K线周期' })).toHaveLength(5);
    expect(within(list).getByText('htx')).toBeInTheDocument();
    expect(within(list).getByText('HTX 行情')).toBeInTheDocument();
    expect(within(list).getByText('coinbase')).toBeInTheDocument();
    expect(within(list).getByText('Coinbase 行情')).toBeInTheDocument();
    expect(within(list).getByRole('gridcell', { name: 'BTCUSDT' })).toBeInTheDocument();
    expect(within(list).getByRole('gridcell', { name: '1m' })).toBeInTheDocument();

    await user.click(within(list).getByRole('button', { name: '启用 行情源 coinbase' }));

    expect(screen.getByRole('checkbox', { name: 'coinbase' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'bitget' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'htx' })).not.toBeChecked();
    expect(within(list).getByRole('button', { name: '禁用 行情源 coinbase' })).toBeInTheDocument();
    expect(within(list).getByRole('button', { name: '启用 行情源 bitget' })).toBeInTheDocument();
  });

  it('saves config with selected intervals, providers, and operation reason', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/market-feed/status' && !init?.method) {
        return Promise.resolve(statusResponse());
      }
      if (path === '/admin/api/v1/market-feed/credentials' && !init?.method) {
        return Promise.resolve(credentialsResponse);
      }
      if (path === '/admin/api/v1/market-feed/config') {
        return Promise.resolve(statusResponse({ needs_reload: true, version: 2 }).saved_config);
      }
      return Promise.resolve({});
    });

    render(<MarketFeedConfigPage />);
    await user.clear(await screen.findByLabelText('交易对 symbols'));
    await user.type(screen.getByLabelText('交易对 symbols'), 'BTC-USDT, ETH-USDT');
    expect(screen.getByLabelText('交易对 symbols').closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('启用状态');
    await selectSemiOption(user, '启用状态', '禁用');
    await selectSemiOption(user, '启用状态', '启用');
    await user.click(screen.getByRole('checkbox', { name: '5m' }));
    await user.click(screen.getByRole('checkbox', { name: '15m' }));
    await user.click(screen.getByRole('checkbox', { name: 'coinbase' }));
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'update feed');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/market-feed/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/market-feed/config')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      enabled: true,
      intervals: ['1m', '15m'],
      providers: ['coinbase'],
      reason: 'update feed',
      symbols: ['BTC-USDT', 'ETH-USDT']
    });
  });

  it('submits credentials without rendering plaintext secret after save', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/market-feed/status' && !init?.method) {
        return Promise.resolve(statusResponse());
      }
      if (path === '/admin/api/v1/market-feed/credentials' && !init?.method) {
        return Promise.resolve({ credentials: [] });
      }
      if (path === '/admin/api/v1/market-feed/credentials/bitget') {
        return Promise.resolve({ api_key_mask: 'abcd****wxyz', auth_type: 'api_key', enabled: true, provider: 'bitget' });
      }
      return Promise.resolve({});
    });

    render(<MarketFeedConfigPage />);
    await user.click(await screen.findByRole('tab', { name: 'Provider 凭证' }));
    await user.type(await screen.findByLabelText('API Key'), 'abcd1234wxyz');
    await user.type(screen.getByLabelText('API Secret'), 'secret-value');
    await user.click(screen.getByRole('button', { name: '保存凭证' }));
    await user.type(screen.getByLabelText('操作原因'), 'store credential');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/market-feed/credentials/bitget',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    await waitFor(() => expect(screen.getByText('abcd****wxyz')).toBeInTheDocument());
    expect(screen.getAllByText('Bitget 行情').length).toBeGreaterThan(0);
    expect(screen.queryByText(/secret-value/)).not.toBeInTheDocument();
  });

  it('manually reloads market feed subscriptions', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/market-feed/status' && !init?.method) {
        return Promise.resolve(statusResponse({ needs_reload: true }));
      }
      if (path === '/admin/api/v1/market-feed/credentials' && !init?.method) {
        return Promise.resolve(credentialsResponse);
      }
      if (path === '/admin/api/v1/market-feed/reload') {
        return Promise.resolve(statusResponse({ needs_reload: false }).saved_config ? {
          config: statusResponse({ needs_reload: false }).saved_config,
          runtime: statusResponse().runtime
        } : null);
      }
      return Promise.resolve({});
    });

    render(<MarketFeedConfigPage />);
    await user.click(await screen.findByRole('tab', { name: '运行状态' }));
    expect(await screen.findByText('待重载')).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: '订阅配置' }));
    await user.click(screen.getByRole('button', { name: '重载行情订阅' }));
    await user.type(screen.getByLabelText('操作原因'), 'apply feed');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/market-feed/reload',
        expect.objectContaining({ method: 'POST', body: JSON.stringify({ reason: 'apply feed' }) })
      );
    });
  });
});
