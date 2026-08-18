import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import { SecurityPolicyPage } from './SecurityPolicyPage';

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

function policyResponse() {
  return {
    login_2fa_mode: 'user_enabled',
    registration_invite_required: false,
    username_login_enabled: false,
    payment_policies: {
      withdraw: { enabled: true, method: 'fund_password' },
      spot_order: { enabled: false, method: 'fund_password' },
      convert: { enabled: false, method: 'fund_password' },
      earn_subscribe: { enabled: false, method: 'fund_password' }
    },
    third_party_bindings: {
      coinbase_wallet_enabled: false,
      telegram_account_enabled: false
    }
  };
}

function semiSelectByLabel(label: string): HTMLElement {
  const labelNode = [...document.querySelectorAll('label')].find((item) => item.textContent?.trim().startsWith(label)) as HTMLElement | undefined;
  expect(labelNode).toBeDefined();
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

function renderSecurityPolicyPage() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { gcTime: 0, retry: false },
      mutations: { retry: false }
    }
  });
  const router = createMemoryRouter(
    [
      { path: '/security', element: <SecurityPolicyPage /> },
      { path: '/other', element: <div>其他页面</div> }
    ],
    { initialEntries: ['/security'] }
  );
  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
  return { queryClient, router, ...view };
}

describe('SecurityPolicyPage', () => {
  beforeEach(() => {
    if (!globalThis.ResizeObserver) {
      Object.defineProperty(globalThis, 'ResizeObserver', {
        configurable: true,
        value: ResizeObserverMock
      });
    }
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/security-policy' && !init?.method) {
        return Promise.resolve(policyResponse());
      }
      if (path === '/admin/api/v1/security-policy' && init?.method === 'PATCH') {
        return Promise.resolve(JSON.parse(String(init.body)));
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    if (!originalResizeObserver) {
      Reflect.deleteProperty(globalThis, 'ResizeObserver');
    }
  });

  it('loads and saves Admin login and payment verification policy', async () => {
    const user = userEvent.setup();

    renderSecurityPolicyPage();

    expect(await screen.findByText('安全策略')).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: '保存安全策略' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '保存安全策略' }).closest('[data-risk-level="high"]')).toBeInTheDocument();
    expect(semiSelectByLabel('登录 2FA 策略')).toHaveTextContent('用户自选');
    expect(screen.getByRole('checkbox', { name: '注册时必须填写邀请码' })).not.toBeChecked();
    expect(screen.getByRole('switch', { name: '允许用户名登录' })).not.toBeChecked();
    expect(screen.queryByRole('checkbox', { name: '启用提现校验' })).not.toBeInTheDocument();
    expect(screen.getByRole('tabpanel', { name: '登录策略' })).toBeInTheDocument();

    await selectSemiOption(user, '登录 2FA 策略', '强制要求');
    await user.click(screen.getByRole('checkbox', { name: '注册时必须填写邀请码' }));
    await user.click(screen.getByRole('switch', { name: '允许用户名登录' }));

    await user.click(screen.getByRole('tab', { name: '资金动作校验' }));
    expect(screen.getByRole('tabpanel', { name: '资金动作校验' })).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: '启用提现校验' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '启用闪兑校验' })).not.toBeChecked();
    expect(semiSelectByLabel('提现校验方式')).toHaveTextContent('资金密码');
    await user.click(screen.getByRole('checkbox', { name: '启用闪兑校验' }));
    await selectSemiOption(user, '闪兑校验方式', '双因素认证');

    await user.click(screen.getByRole('tab', { name: '第三方绑定' }));
    expect(screen.getByRole('switch', { name: '允许绑定Coinbase 钱包' })).not.toBeChecked();
    expect(screen.getByRole('switch', { name: '允许绑定TG 账号' })).not.toBeChecked();
    await user.click(screen.getByRole('switch', { name: '允许绑定Coinbase 钱包' }));
    await user.click(screen.getByRole('switch', { name: '允许绑定TG 账号' }));

    await user.click(screen.getByRole('tab', { name: '登录策略' }));
    expect(semiSelectByLabel('登录 2FA 策略')).toHaveTextContent('强制要求');
    expect(screen.getByRole('checkbox', { name: '注册时必须填写邀请码' })).toBeChecked();
    expect(screen.getByRole('switch', { name: '允许用户名登录' })).toBeChecked();

    await user.click(screen.getByRole('tab', { name: '策略摘要' }));
    expect(screen.getByRole('tabpanel', { name: '策略摘要' })).toBeInTheDocument();
    expect(screen.getByText('登录策略：强制要求')).toBeInTheDocument();
    expect(screen.getByText('注册策略：邀请码必填')).toBeInTheDocument();
    expect(screen.getByText('用户名登录已开启')).toBeInTheDocument();
    expect(screen.getByText('Coinbase 钱包：已开启，TG 账号：已开启')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '保存安全策略' }));

    expect(await screen.findByText('确认保存高风险安全策略')).toBeInTheDocument();
    expect(screen.getByText('字段差异（6 项）')).toBeInTheDocument();
    expect(screen.getByText('保存后会立即影响全站用户登录、资金操作验证和第三方绑定入口。')).toBeInTheDocument();
    expect(screen.getByText('这是高风险配置变更，保存前请再次核对影响范围。')).toBeInTheDocument();
    expect(screen.getByText('闪兑校验')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '确认保存' })).toBeDisabled();
    await user.type(await screen.findByLabelText('操作原因'), 'tighten policy');
    await user.click(await screen.findByRole('button', { name: '确认保存' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/security-policy', expect.objectContaining({ method: 'PATCH' }));
    });
    const request = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/security-policy' && init?.method === 'PATCH')?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      login_2fa_mode: 'mandatory',
      registration_invite_required: true,
      username_login_enabled: true,
      payment_policies: {
        withdraw: { enabled: true, method: 'fund_password' },
        spot_order: { enabled: false, method: 'fund_password' },
        convert: { enabled: true, method: 'two_factor' },
        earn_subscribe: { enabled: false, method: 'fund_password' }
      },
      third_party_bindings: {
        coinbase_wallet_enabled: true,
        telegram_account_enabled: true
      },
      reason: 'tighten policy'
    });
    expect(await screen.findByText('安全策略已保存。')).toBeInTheDocument();
    expect(screen.queryByText('有未保存的变更')).not.toBeInTheDocument();
  });
});
