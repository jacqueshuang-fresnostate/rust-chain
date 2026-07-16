import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import { QuickRechargeConfigPage } from './QuickRechargeConfigPage';

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

const quickRechargeConfig = {
  api_base_url: 'https://pay.example.test',
  currency: 'cny',
  enabled: true,
  id: 8,
  max_amount: '5000',
  merchant_pid: '10001',
  merchant_secret_mask: 'sec****7890',
  merchant_secret_set: true,
  min_amount: '10',
  network: 'tron',
  notify_url: 'https://api.example.test/api/v1/payments/gmpay/notify',
  redirect_url: 'https://www.example.test/user/recharge',
  pc_app_redirect_url: 'rustchain://quick-recharge/return',
  mac_app_redirect_url: 'rustchain-mac://quick-recharge/return',
  ios_app_redirect_url: 'rustchain-ios://quick-recharge/return',
  android_app_redirect_url: 'rustchain-android://quick-recharge/return',
  mobile_web_redirect_url: 'https://m.example.test/user/recharge',
  desktop_web_redirect_url: 'https://www.example.test/user/recharge',
  token: 'usdt',
  updated_at: 1_775_027_600_000,
  updated_by: 9
};

const disabledQuickRechargeConfig = {
  ...quickRechargeConfig,
  enabled: false
};

const incompleteQuickRechargeConfig = {
  ...quickRechargeConfig,
  api_base_url: null,
  enabled: false,
  merchant_pid: null,
  merchant_secret_mask: null,
  merchant_secret_set: false,
  notify_url: null
};

describe('QuickRechargeConfigPage', () => {
  beforeEach(() => {
    stubResizeObserver();
    stubMatchMedia();
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/quick-recharge/config' && !init?.method) {
        return Promise.resolve(quickRechargeConfig);
      }
      if (path === '/admin/api/v1/quick-recharge/config' && init?.method === 'PATCH') {
        return Promise.resolve({
          ...quickRechargeConfig,
          api_base_url: 'https://pay.new.test'
        });
      }
      if (path === '/admin/api/v1/quick-recharge/config/test' && init?.method === 'POST') {
        return Promise.resolve({
          actual_amount: '2.500000000000000000',
          currency: 'cny',
          expiration_time: 1_775_100_000,
          fiat_amount: '12.500000000000000000',
          network: 'tron',
          order_id: 'test-order-1',
          payment_url: 'https://cashier.example/test-order-1',
          provider_trade_id: 'GMTEST1',
          receive_address: 'TQuickRechargeTestAddress',
          tested_at: 1_775_027_700_000,
          token: 'usdt'
        });
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('uses a spacious Semi grid layout instead of the compact action form', async () => {
    const { container } = render(<QuickRechargeConfigPage />);

    const apiBaseUrl = await screen.findByLabelText('API 基础地址');
    expect(apiBaseUrl.closest('.semi-col')).toHaveClass('semi-col-lg-16');
    expect(screen.getByText('配置 ID：8')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '商户接口' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '充值限制' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '入账范围' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '单笔金额限制' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '回调跳转' })).toBeInTheDocument();
    expect(screen.getByLabelText('PC 应用端回跳地址')).toHaveValue('rustchain://quick-recharge/return');
    expect(screen.getByLabelText('手机网页端回跳地址')).toHaveValue('https://m.example.test/user/recharge');
    expect(screen.getByRole('heading', { name: '联通测试' })).toBeInTheDocument();
    expect(container.querySelector('.admin-action-form')).not.toBeInTheDocument();

    const currencyLabel = screen.getByText('法币币种');
    const tokenLabel = screen.getByText('到账资产');
    const networkLabel = screen.getByText('收款网络');
    const minAmountInput = screen.getByLabelText('单笔最小金额');
    const maxAmountInput = screen.getByLabelText('单笔最大金额');

    expect(currencyLabel.closest('.semi-col')).toHaveClass('semi-col-xl-8');
    expect(tokenLabel.closest('.semi-col')).toHaveClass('semi-col-xl-8');
    expect(networkLabel.closest('.semi-col')).toHaveClass('semi-col-xl-8');
    expect(minAmountInput?.closest('.semi-col')).toHaveClass('semi-col-md-12');
    expect(maxAmountInput?.closest('.semi-col')).toHaveClass('semi-col-md-12');
  });

  it('keeps the save action available and shows missing fields when enabling an incomplete config', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/quick-recharge/config' && !init?.method) {
        return Promise.resolve(incompleteQuickRechargeConfig);
      }
      return Promise.resolve({});
    });

    render(<QuickRechargeConfigPage />);
    await user.click(await screen.findByRole('switch'));

    expect(screen.getByText('将启用，保存后生效')).toBeInTheDocument();
    expect(screen.getByText('启用前需完善：API 基础地址、商户 PID、商户 Secret Key、异步回调地址')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '保存并启用GMPay' })).toBeEnabled();
  });

  it('enables a complete saved quick recharge config', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/quick-recharge/config' && !init?.method) {
        return Promise.resolve(disabledQuickRechargeConfig);
      }
      if (path === '/admin/api/v1/quick-recharge/config' && init?.method === 'PATCH') {
        return Promise.resolve({ ...disabledQuickRechargeConfig, enabled: true });
      }
      return Promise.resolve({});
    });

    render(<QuickRechargeConfigPage />);
    await user.click(await screen.findByRole('switch'));
    expect(screen.getByText('将启用，保存后生效')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '保存并启用GMPay' }));
    await user.type(screen.getByLabelText('操作原因'), 'enable quick recharge');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/quick-recharge/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/quick-recharge/config' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual(expect.objectContaining({
      enabled: true,
      reason: 'enable quick recharge'
    }));
  });

  it('disables a saved quick recharge config through the switch draft and save action', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/quick-recharge/config' && !init?.method) {
        return Promise.resolve(quickRechargeConfig);
      }
      if (path === '/admin/api/v1/quick-recharge/config' && init?.method === 'PATCH') {
        return Promise.resolve({ ...quickRechargeConfig, enabled: false });
      }
      return Promise.resolve({});
    });

    render(<QuickRechargeConfigPage />);
    expect(await screen.findByText('当前已启用')).toBeInTheDocument();
    await user.click(screen.getByRole('switch'));
    expect(screen.getByText('将停用，保存后生效')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '保存并停用GMPay' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable quick recharge');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/quick-recharge/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/quick-recharge/config' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual(expect.objectContaining({
      enabled: false,
      reason: 'disable quick recharge'
    }));
    expect(await screen.findByText('当前未启用')).toBeInTheDocument();
  });

  it('saves the quick recharge config with operation reason', async () => {
    const user = userEvent.setup();

    render(<QuickRechargeConfigPage />);
    await user.clear(await screen.findByLabelText('API 基础地址'));
    await user.type(screen.getByLabelText('API 基础地址'), 'https://pay.new.test');
    await user.click(screen.getByRole('button', { name: '保存快速充值配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'spread out quick recharge settings');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/quick-recharge/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/quick-recharge/config' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      api_base_url: 'https://pay.new.test',
      currency: 'cny',
      enabled: true,
      max_amount: '5000',
      android_app_redirect_url: 'rustchain-android://quick-recharge/return',
      desktop_web_redirect_url: 'https://www.example.test/user/recharge',
      ios_app_redirect_url: 'rustchain-ios://quick-recharge/return',
      mac_app_redirect_url: 'rustchain-mac://quick-recharge/return',
      merchant_pid: '10001',
      merchant_secret: null,
      min_amount: '10',
      mobile_web_redirect_url: 'https://m.example.test/user/recharge',
      network: 'tron',
      notify_url: 'https://api.example.test/api/v1/payments/gmpay/notify',
      pc_app_redirect_url: 'rustchain://quick-recharge/return',
      redirect_url: 'https://www.example.test/user/recharge',
      reason: 'spread out quick recharge settings',
      token: 'usdt'
    });
  });

  it('tests the saved quick recharge provider config and renders the provider result', async () => {
    const user = userEvent.setup();

    render(<QuickRechargeConfigPage />);
    await user.clear(await screen.findByLabelText('测试金额'));
    await user.type(screen.getByLabelText('测试金额'), '12.50');
    await user.click(screen.getByRole('button', { name: '测试快速充值' }));
    await user.type(screen.getByLabelText('操作原因'), 'verify quick recharge provider');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/quick-recharge/config/test',
        expect.objectContaining({
          method: 'POST',
          body: JSON.stringify({
            amount: '12.5',
            reason: 'verify quick recharge provider'
          })
        })
      );
    });
    expect(await screen.findByTestId('quick-recharge-test-result')).toHaveTextContent('test-order-1');
    expect(screen.getByTestId('quick-recharge-test-result')).toHaveTextContent('GMTEST1');
    expect(screen.getByRole('button', { name: '打开收银台' })).toBeInTheDocument();
  });
});
