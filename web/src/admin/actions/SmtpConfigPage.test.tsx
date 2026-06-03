import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SmtpConfigPage } from './SmtpConfigPage';
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

const savedConfig = {
  enabled: true,
  from_email: 'noreply@example.test',
  from_name: 'Exchange',
  host: 'smtp.example.test',
  id: 3,
  name: 'default',
  password_set: true,
  port: 587,
  security: 'starttls',
  username_mask: 'mail****user'
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

describe('SmtpConfigPage', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/smtp/config' && !init?.method) {
        return Promise.resolve(savedConfig);
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('loads the saved config with Semi controls and never renders plaintext SMTP password', async () => {
    render(<SmtpConfigPage />);

    expect(await screen.findByDisplayValue('smtp.example.test')).toBeInTheDocument();
    expect(screen.getByLabelText('SMTP host').closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('加密方式');
    expect(screen.getByLabelText('SMTP 用户名').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('SMTP 密码').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('SMTP 密码')).toHaveValue('');
    expect(screen.getByRole('checkbox', { name: '启用 SMTP' })).toBeChecked();
    expect(screen.getByText(/当前用户名：mail\*\*\*\*user/)).toBeInTheDocument();
    expect(screen.getByText(/SMTP 密码：已设置/)).toBeInTheDocument();
    expect(screen.queryByText(/smtp-password/)).not.toBeInTheDocument();
  });

  it('saves SMTP config with operation reason and omits blank password to preserve existing secret', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/smtp/config' && !init?.method) {
        return Promise.resolve(null);
      }
      if (path === '/admin/api/v1/smtp/config' && init?.method === 'PATCH') {
        return Promise.resolve({ ...savedConfig, host: 'smtp.new.test', port: 465, security: 'tls', password_set: false });
      }
      return Promise.resolve({});
    });

    render(<SmtpConfigPage />);
    await user.type(await screen.findByLabelText('SMTP host'), 'smtp.new.test');
    await user.clear(screen.getByLabelText('SMTP port'));
    await user.type(screen.getByLabelText('SMTP port'), '465');
    await selectSemiOption(user, '加密方式', 'TLS');
    await user.type(screen.getByLabelText('发件邮箱'), 'noreply@example.test');
    await user.click(screen.getByRole('checkbox', { name: '启用 SMTP' }));
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'configure smtp');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/smtp/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/smtp/config' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      enabled: true,
      from_email: 'noreply@example.test',
      host: 'smtp.new.test',
      port: 465,
      reason: 'configure smtp',
      security: 'tls'
    });
  });

  it('sends a test email with recipient and operation reason', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/smtp/config' && !init?.method) {
        return Promise.resolve(savedConfig);
      }
      if (path === '/admin/api/v1/smtp/test') {
        return Promise.resolve({ recipient: 'ops@example.test', sent: true });
      }
      return Promise.resolve({});
    });

    render(<SmtpConfigPage />);
    await user.type(await screen.findByLabelText('测试收件邮箱'), 'ops@example.test');
    await user.click(screen.getByRole('button', { name: '测试发送' }));
    await user.type(screen.getByLabelText('操作原因'), 'verify smtp');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/smtp/test',
        expect.objectContaining({ method: 'POST', body: JSON.stringify({ recipient: 'ops@example.test', reason: 'verify smtp' }) })
      );
    });
  });
});
