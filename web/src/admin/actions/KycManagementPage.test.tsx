import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { KycManagementPage } from './KycManagementPage';
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

let installedResizeObserverMock = false;

const kycConfig = {
  allowed_countries: ['China', 'Japan'],
  country_document_types: [
    { country: 'China', document_types: ['identity_card', 'passport'], handheld_document_types: ['passport'] }
  ],
  created_at: 1_735_732_700_000,
  enabled: true,
  id: 1,
  max_document_size_bytes: 5_242_880,
  name: 'default',
  required_documents: ['identity_front', 'identity_back'],
  target_kyc_level: 1,
  updated_at: 1_735_732_800_000,
  updated_by: 9
};

const pendingSubmission = {
  country: 'China',
  submission_type: 'enterprise',
  enterprise_name: 'Acme Holdings Ltd',
  business_registration_number: '91310000712345678A',
  document_type: 'identity_card',
  email: 'kyc-user@example.test',
  id: 501,
  id_number: 'CN1234567890',
  real_name: 'Zhang San',
  status: 'pending',
  submitted_at: 1_735_732_800_000,
  target_kyc_level: 1,
  user_id: 42
};

const detailSubmission = {
  ...pendingSubmission,
  document_back_image: 'data:image/png;base64,back-image',
  document_front_image: 'data:image/png;base64,front-image',
  document_handheld_image: 'data:image/png;base64,handheld-image',
  document_type: 'identity_card',
  phone: '18800000000',
  review_reason: null,
  reviewed_at: null,
  reviewed_by: null,
  updated_at: 1_735_732_800_000
};

function semiSelectByLabel(label: string): HTMLElement {
  const labelNode = screen.getByText(label).closest('label') as HTMLElement | null;
  expect(labelNode).toBeInTheDocument();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

function semiSelectByAriaLabel(label: string): HTMLElement {
  const node = document.querySelector(`[aria-label="${label}"]`) as HTMLElement | null;
  expect(node).toBeInTheDocument();
  const select = (node?.matches('.semi-select') ? node : node?.querySelector('.semi-select') ?? node?.closest('.semi-select')) as HTMLElement | null;
  expect(select ?? node).toBeInTheDocument();
  return (select ?? node) as HTMLElement;
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

async function selectSemiOptionByAriaLabel(user: ReturnType<typeof userEvent.setup>, label: string, optionLabel: string) {
  await user.click(semiSelectByAriaLabel(label));
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

describe('KycManagementPage', () => {
  beforeEach(() => {
    installedResizeObserverMock = !('ResizeObserver' in globalThis);
    if (installedResizeObserverMock) {
      Object.defineProperty(globalThis, 'ResizeObserver', {
        configurable: true,
        value: ResizeObserverMock
      });
    }
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/kyc/config' && !init?.method) {
        return Promise.resolve(kycConfig);
      }
      if (path === '/admin/api/v1/kyc/config' && init?.method === 'PATCH') {
        return Promise.resolve({
          ...kycConfig,
          allowed_countries: ['Singapore'],
          country_document_types: [{ country: 'Singapore', document_types: ['identity_card', 'passport'], handheld_document_types: ['passport'] }],
          target_kyc_level: 2
        });
      }
      if (path === '/admin/api/v1/countries?status=active&limit=200' && !init?.method) {
        return Promise.resolve({
          countries: [
            { country_code: 'CN', country_name: '中国', remark: '中国', registration_enabled: true, status: 'active' },
            { country_code: 'SG', country_name: 'Singapore', remark: '新加坡', registration_enabled: true, status: 'active' }
          ]
        });
      }
      if (String(path).startsWith('/admin/api/v1/kyc/submissions?') && !init?.method) {
        return Promise.resolve({ submissions: [pendingSubmission] });
      }
      if (path === '/admin/api/v1/kyc/submissions/501' && !init?.method) {
        return Promise.resolve(detailSubmission);
      }
      if (path === '/admin/api/v1/kyc/submissions/501/review' && init?.method === 'PATCH') {
        return Promise.resolve({ ...detailSubmission, status: 'approved', reviewed_by: 9, review_reason: 'identity checked' });
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    if (installedResizeObserverMock) {
      Reflect.deleteProperty(globalThis, 'ResizeObserver');
    }
  });

  it('renders the review table at full container width and opens detail SideSheet', async () => {
    const user = userEvent.setup();

    render(<KycManagementPage />);

    const emailCell = await screen.findByText('kyc-user@example.test');
    const tableWrapper = emailCell.closest('.semi-table-wrapper');
    expect(tableWrapper).toHaveStyle({ maxWidth: '100%', width: '100%' });
    expect(within(tableWrapper as HTMLElement).getByText('kyc-user@example.test')).toBeInTheDocument();
    expect(within(tableWrapper as HTMLElement).getByText('Zhang San')).toBeInTheDocument();
    expect(within(tableWrapper as HTMLElement).getByText('CN12****7890')).toBeInTheDocument();
    expect(within(tableWrapper as HTMLElement).getByText('身份证')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '查看' }));

    expect(await screen.findByText('KYC 审核详情')).toBeInTheDocument();
    expect(screen.getByText('18800000000')).toBeInTheDocument();
    expect(screen.getByRole('img', { name: '证件正面' })).toBeInTheDocument();
    expect(screen.getByRole('img', { name: '证件反面' })).toBeInTheDocument();
    expect(screen.getByRole('img', { name: '本人手持证件照' })).toBeInTheDocument();
    expect(screen.getAllByText('Acme Holdings Ltd').length).toBeGreaterThan(0);
    expect(screen.getAllByText('91310000712345678A').length).toBeGreaterThan(0);
  });

  it('approves a pending submission, closes the SideSheet, and refreshes the list', async () => {
    const user = userEvent.setup();

    render(<KycManagementPage />);

    await user.click(await screen.findByRole('button', { name: '查看' }));
    await user.clear(await screen.findByLabelText('通过后 KYC 等级'));
    await user.type(screen.getByLabelText('通过后 KYC 等级'), '2');
    await user.type(screen.getByLabelText('审核原因'), 'identity checked');
    await user.click(screen.getByRole('button', { name: '审核通过' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/kyc/submissions/501/review',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/kyc/submissions/501/review')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      kyc_level: 2,
      reason: 'identity checked',
      status: 'approved'
    });
    await waitFor(() => {
      expect(screen.queryByText('审核处理')).not.toBeInTheDocument();
    });
    expect(apiRequestMock.mock.calls.filter(([path]) => String(path).startsWith('/admin/api/v1/kyc/submissions?status=pending'))).toHaveLength(2);
  });

  it('saves KYC config with Semi controls and operation reason', async () => {
    const user = userEvent.setup();

    render(<KycManagementPage />);

    await user.click(await screen.findByRole('tab', { name: 'KYC 配置' }));
    expect(screen.getByText('身份证件正面')).toBeInTheDocument();
    expect(screen.getByText('身份证件反面')).toBeInTheDocument();
    expect(screen.getByText('本人手持证件照：1 个证件类型')).toBeInTheDocument();
    expect(screen.queryByRole('checkbox', { name: '身份证件正面' })).not.toBeInTheDocument();
    semiSelectByLabel('配置状态');
    await selectSemiOption(user, '配置状态', '禁用');
    await user.clear(screen.getByLabelText('目标 KYC 等级'));
    await user.type(screen.getByLabelText('目标 KYC 等级'), '2');
    await user.clear(screen.getByLabelText('允许国家'));
    await user.type(screen.getByLabelText('允许国家'), 'Singapore');
    await selectSemiOptionByAriaLabel(user, '规则国家 1', 'Singapore (SG)');
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'update kyc config');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/kyc/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/kyc/config' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      allowed_countries: ['Singapore'],
      country_document_types: [{ country: 'Singapore', document_types: ['identity_card', 'passport'], handheld_document_types: ['passport'] }],
      enabled: false,
      max_document_size_bytes: 5_242_880,
      reason: 'update kyc config',
      required_documents: ['identity_front', 'identity_back'],
      target_kyc_level: 2
    });
  });
});
