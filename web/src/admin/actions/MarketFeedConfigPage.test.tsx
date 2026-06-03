import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
      providers: ['bitget', 'htx'],
      symbols: ['BTCUSDT', 'ETHUSDT'],
      version: 1,
      ...overrides
    },
    runtime: {
      applied_version: 1,
      intervals: ['1m', '5m'],
      last_reload_error: null,
      last_reload_status: 'success',
      providers: ['bitget', 'htx'],
      symbols: ['BTCUSDT', 'ETHUSDT']
    }
  };
}

const credentialsResponse = {
  credentials: [{ api_key_mask: 'abcd****wxyz', auth_type: 'api_key', enabled: true, provider: 'bitget' }]
};

function semiSelectByLabel(label: string): HTMLElement {
  const labelNode = screen.getByText(label).closest('label') as HTMLElement | null;
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
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
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
    vi.unstubAllGlobals();
  });

  it('loads saved config, runtime status, masked credentials, and selectable feed options without static helper copy', async () => {
    render(<MarketFeedConfigPage />);

    const symbolsInput = await screen.findByDisplayValue('BTCUSDT,ETHUSDT');
    expect(symbolsInput).toBeInTheDocument();
    expect(symbolsInput.closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('启用状态');
    semiSelectByLabel('Provider');
    semiSelectByLabel('Auth Type');
    expect(screen.getByLabelText('API Key').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('API Secret').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('Passphrase').closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('凭证状态');
    expect(screen.queryByText('配置第三方行情 symbols、intervals、providers 和 API Key；保存后需手动重载才会生效。')).not.toBeInTheDocument();
    expect(screen.queryByText('交易对支持逗号分隔输入；K 线周期和行情源可多选，保存后需手动重载。')).not.toBeInTheDocument();
    expect(screen.queryByText('保存配置不会立即影响 worker，只有手动重载会更新运行态。')).not.toBeInTheDocument();
    expect(screen.queryByText('API Key、Secret、Passphrase 只会加密提交；页面和接口仅展示 Key 掩码。')).not.toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: '1m' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '5m' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '15m' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'bitget' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'htx' })).toBeChecked();
    expect(screen.getByTestId('runtime-providers')).toHaveTextContent('当前启动 providers');
    expect(screen.getByTestId('runtime-providers')).toHaveTextContent('bitget');
    expect(screen.getByTestId('runtime-providers')).toHaveTextContent('htx');
    expect(screen.getByText('成功')).toBeInTheDocument();
    expect(screen.getByText(/运行 symbols：BTCUSDT,ETHUSDT/)).toBeInTheDocument();
    expect(screen.getByText(/bitget:abcd\*\*\*\*wxyz/)).toBeInTheDocument();
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
    await user.click(screen.getByRole('checkbox', { name: 'htx' }));
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
      providers: ['bitget'],
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
    await user.type(await screen.findByLabelText('API Key'), 'abcd1234wxyz');
    await user.type(screen.getByLabelText('API Secret'), 'secret-value');
    await user.click(screen.getByRole('button', { name: '保存凭证' }));
    await user.type(screen.getByLabelText('操作原因'), 'store credential');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => expect(screen.getByText(/bitget:abcd\*\*\*\*wxyz/)).toBeInTheDocument());
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
    expect(await screen.findByText('待重载')).toBeInTheDocument();
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
