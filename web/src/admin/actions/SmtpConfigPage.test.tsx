import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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

const savedConfig = {
  enabled: true,
  from_email: 'noreply@example.test',
  from_name: 'Exchange',
  host: 'smtp.example.test',
  id: 3,
  name: '主发信配置',
  password_set: true,
  port: 587,
  priority: 10,
  security: 'starttls',
  username_mask: 'mail****user',
  verification_code_template_html: '<p>{{subject}}：<strong>{{code}}</strong></p>',
  verification_code_templates: [
    {
      key: 'default',
      name: '通用验证码模板',
      purpose: null,
      html: '<p>{{subject}}：<strong>{{code}}</strong></p>',
      enabled: true
    },
    {
      key: 'fund_password_reset',
      name: '资金密码验证码',
      purpose: 'fund_password_reset',
      html: '<p>资金密码：<strong>{{code}}</strong></p>',
      enabled: true
    }
  ]
};

const listResponse = {
  configs: [savedConfig],
  delivery_settings: {
    strategy: 'priority'
  }
};

function semiSelectByLabel(label: string, root: HTMLElement | Document = document.body): HTMLElement {
  const labelNode = [...root.querySelectorAll('label')].find((item) => item.textContent?.trim().startsWith(label) && item.querySelector('.semi-select')) as HTMLElement | undefined;
  expect(labelNode).toBeDefined();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

async function selectSemiOption(user: ReturnType<typeof userEvent.setup>, label: string, optionLabel: string, root: HTMLElement | Document = document.body) {
  await user.click(semiSelectByLabel(label, root));
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

async function findSideSheet(title: string): Promise<HTMLElement> {
  let sheet: HTMLElement | null = null;
  await waitFor(() => {
    const titleNode = [...document.querySelectorAll('.semi-sidesheet-title')].find((item) => item.textContent?.trim() === title) as HTMLElement | undefined;
    expect(titleNode).toBeDefined();
    sheet = titleNode?.closest('.semi-sidesheet-inner') as HTMLElement | null;
    expect(sheet).toBeInTheDocument();
  });
  if (!sheet) {
    throw new Error(`SideSheet with title "${title}" was not found.`);
  }
  return sheet;
}

describe('SmtpConfigPage', () => {
  beforeEach(() => {
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/smtp/configs' && !init?.method) {
        return Promise.resolve(listResponse);
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    apiRequestMock.mockReset();
  });

  it('loads the saved config with Semi controls and never renders plaintext SMTP password', async () => {
    render(<SmtpConfigPage />);

    expect(await screen.findByDisplayValue('smtp.example.test')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '发信配置' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '验证码模板' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '发信策略' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '测试发送' })).toBeInTheDocument();
    expect(screen.getByText('主发信配置')).toBeInTheDocument();
    expect(screen.getByLabelText('SMTP host').closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('加密方式');
    expect(screen.getByLabelText('SMTP 用户名').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('SMTP 密码').closest('.semi-input-wrapper')).toBeInTheDocument();
    expect(screen.getByLabelText('SMTP 密码')).toHaveValue('');
    expect(screen.getByRole('checkbox', { name: '启用 SMTP' })).toBeChecked();
    await userEvent.click(screen.getByText('验证码模板'));
    expect(screen.getByLabelText('验证码 HTML 模板 1')).toHaveAttribute('contenteditable', 'true');
    expect(screen.getByLabelText('验证码 HTML 模板 1')).toHaveTextContent('{{subject}}：{{code}}');
    expect(screen.getByLabelText('验证码 HTML 模板 2')).toHaveTextContent('资金密码：{{code}}');
    expect(screen.queryByLabelText('验证码 HTML 模板')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('tab', { name: '发信策略' }));
    expect(screen.getByText('按优先级发送')).toBeInTheDocument();
    expect(screen.queryByText(/smtp-password/)).not.toBeInTheDocument();
  });

  it('saves SMTP config with operation reason and omits blank password to preserve existing secret', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/smtp/configs' && !init?.method) {
        return Promise.resolve({ configs: [], delivery_settings: { strategy: 'priority' } });
      }
      if (path === '/admin/api/v1/smtp/configs' && init?.method === 'POST') {
        return Promise.resolve({ ...savedConfig, host: 'smtp.new.test', port: 465, security: 'tls', password_set: false });
      }
      return Promise.resolve({});
    });

    render(<SmtpConfigPage />);
    await user.click(await screen.findByRole('button', { name: '新增配置' }));
    const sheet = await findSideSheet('新增发信配置');
    await user.type(within(sheet).getByLabelText('SMTP host'), 'smtp.new.test');
    await user.clear(within(sheet).getByLabelText('SMTP port'));
    await user.type(within(sheet).getByLabelText('SMTP port'), '465');
    await selectSemiOption(user, '加密方式', 'TLS/SSL 加密', sheet);
    await user.type(within(sheet).getByLabelText('发件邮箱'), 'noreply@example.test');
    await user.click(within(sheet).getByRole('checkbox', { name: '启用 SMTP' }));
    fireEvent.input(within(sheet).getByLabelText('新增验证码 HTML 模板 1'), { target: { innerText: '绑定邮箱 {{code}}' } });
    await user.click(within(sheet).getByRole('button', { name: '新增模板' }));
    fireEvent.input(within(sheet).getByLabelText('新增验证码 HTML 模板 2'), { target: { innerText: '资金密码 {{code}}' } });
    await user.click(within(sheet).getByRole('button', { name: '新增配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'configure smtp');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/smtp/configs',
        expect.objectContaining({ method: 'POST' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/smtp/configs' && init?.method === 'POST')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      enabled: true,
      from_email: 'noreply@example.test',
      host: 'smtp.new.test',
      name: '发信配置 1',
      port: 465,
      priority: 100,
      reason: 'configure smtp',
      security: 'tls',
      verification_code_template_html: '<p>绑定邮箱 {{code}}</p>',
      verification_code_templates: [
        {
          enabled: true,
          html: '<p>绑定邮箱 {{code}}</p>',
          key: 'default',
          name: '通用验证码模板',
          purpose: null
        },
        {
          enabled: true,
          html: '<p>资金密码 {{code}}</p>',
          key: 'bind',
          name: '绑定邮箱模板',
          purpose: 'bind'
        }
      ]
    });
    await waitFor(() => {
      expect(document.querySelector('.semi-sidesheet-title')?.textContent).not.toBe('新增发信配置');
    });
  });

  it('saves the SMTP delivery strategy', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/smtp/configs' && !init?.method) {
        return Promise.resolve(listResponse);
      }
      if (path === '/admin/api/v1/smtp/delivery-settings' && init?.method === 'PATCH') {
        return Promise.resolve({ strategy: 'round_robin' });
      }
      return Promise.resolve({});
    });

    render(<SmtpConfigPage />);
    await screen.findByDisplayValue('smtp.example.test');
    await user.click(screen.getByRole('tab', { name: '发信策略' }));
    await selectSemiOption(user, '发送策略', '轮询发送');
    await user.click(screen.getByRole('button', { name: '保存策略' }));
    await user.type(screen.getByLabelText('操作原因'), 'switch smtp strategy');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/smtp/delivery-settings',
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({ strategy: 'round_robin', reason: 'switch smtp strategy' })
        })
      );
    });
  });

  it('sends a test email with recipient and operation reason', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/smtp/configs' && !init?.method) {
        return Promise.resolve(listResponse);
      }
      if (path === '/admin/api/v1/smtp/test') {
        return Promise.resolve({ config_id: 3, config_name: '主发信配置', recipient: 'ops@example.test', sent: true });
      }
      return Promise.resolve({});
    });

    render(<SmtpConfigPage />);
    await screen.findByDisplayValue('smtp.example.test');
    await user.click(screen.getByRole('tab', { name: '测试发送' }));
    await user.type(screen.getByLabelText('测试收件邮箱'), 'ops@example.test');
    await selectSemiOption(user, '发信方式', '主发信配置');
    await user.click(screen.getByRole('button', { name: '测试发送' }));
    await user.type(screen.getByLabelText('操作原因'), 'verify smtp');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/smtp/test',
        expect.objectContaining({ method: 'POST', body: JSON.stringify({ recipient: 'ops@example.test', reason: 'verify smtp', config_id: 3 }) })
      );
    });
  });
});
