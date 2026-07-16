import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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

    render(<SecurityPolicyPage />);

    expect(await screen.findByText('安全策略')).toBeInTheDocument();
    expect(semiSelectByLabel('登录 2FA 策略')).toHaveTextContent('用户自选');
    expect(screen.getByRole('checkbox', { name: '启用提现校验' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '启用闪兑校验' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: '注册时必须填写邀请码' })).not.toBeChecked();
    expect(screen.getByRole('switch', { name: '允许用户名登录' })).not.toBeChecked();
    expect(screen.getByRole('switch', { name: '允许绑定Coinbase 钱包' })).not.toBeChecked();
    expect(screen.getByRole('switch', { name: '允许绑定TG 账号' })).not.toBeChecked();
    expect(semiSelectByLabel('提现校验方式')).toHaveTextContent('资金密码');
    expect(screen.getByText('登录策略：用户自选')).toBeInTheDocument();
    expect(screen.getByText('注册策略：邀请码选填')).toBeInTheDocument();
    expect(screen.getByText('用户名登录未开启')).toBeInTheDocument();
    expect(screen.getByText('Coinbase 钱包：未开启，TG 账号：未开启')).toBeInTheDocument();

    await selectSemiOption(user, '登录 2FA 策略', '强制要求');
    await user.click(screen.getByRole('checkbox', { name: '注册时必须填写邀请码' }));
    await user.click(screen.getByRole('switch', { name: '允许用户名登录' }));
    await user.click(screen.getByRole('checkbox', { name: '启用闪兑校验' }));
    await user.click(screen.getByRole('switch', { name: '允许绑定Coinbase 钱包' }));
    await user.click(screen.getByRole('switch', { name: '允许绑定TG 账号' }));
    await selectSemiOption(user, '闪兑校验方式', '双因素认证');
    await user.click(screen.getByRole('button', { name: '保存安全策略' }));
    await user.type(screen.getByLabelText('操作原因'), 'tighten policy');
    await user.click(screen.getByRole('button', { name: '确认' }));

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
  });
});
