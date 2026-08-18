import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  confirmAdminTwoFactor,
  disableAdminTwoFactor,
  getAdminTwoFactorStatus,
  setupAdminTwoFactor
} from '../../api/adminAuth';
import { AdminTwoFactorPage } from './AdminTwoFactorPage';

vi.mock('../../api/adminAuth', () => ({
  confirmAdminTwoFactor: vi.fn(),
  disableAdminTwoFactor: vi.fn(),
  getAdminTwoFactorStatus: vi.fn(),
  setupAdminTwoFactor: vi.fn()
}));

const getStatusMock = vi.mocked(getAdminTwoFactorStatus);
const setupMock = vi.mocked(setupAdminTwoFactor);
const confirmMock = vi.mocked(confirmAdminTwoFactor);
const disableMock = vi.mocked(disableAdminTwoFactor);

describe('AdminTwoFactorPage', () => {
  beforeEach(() => {
    getStatusMock.mockReset();
    setupMock.mockReset();
    confirmMock.mockReset();
    disableMock.mockReset();
    getStatusMock.mockResolvedValue({ totp_enabled: false });
    setupMock.mockResolvedValue({
      otpauth_uri: 'otpauth://totp/HIPPO:admin?secret=SECRET',
      secret: 'SECRET'
    });
    confirmMock.mockResolvedValue({ totp_enabled: true });
  });

  it('presents two-factor authentication as current-account security and preserves the API flow', async () => {
    const user = userEvent.setup();

    render(<AdminTwoFactorPage />);

    expect(await screen.findByText('账号安全')).toBeInTheDocument();
    expect(screen.getByText('两步验证')).toBeInTheDocument();
    expect(screen.getByText(/此设置不会改变全局安全策略/)).toBeInTheDocument();

    await user.click(await screen.findByRole('button', { name: '开始绑定' }));
    expect(await screen.findByText(/otpauth:\/\/totp\/HIPPO:admin/)).toBeInTheDocument();
    await user.type(screen.getByLabelText('验证码'), ' 123456 ');
    await user.click(screen.getByRole('button', { name: '确认开启' }));

    await waitFor(() => {
      expect(confirmMock).toHaveBeenCalledWith('123456');
    });
    expect(await screen.findByText('当前账号已开启两步验证。')).toBeInTheDocument();
  });
});
