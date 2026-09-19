import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Toast } from '@douyinfe/semi-ui';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactElement } from 'react';

import { listAdminResource } from '../../../api/adminResources';
import { ApiError, apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { LOAN_PRODUCT_REVISION_CONFLICT_MESSAGE, LoanProductRowActions } from './loan';

vi.mock('../../../api/adminResources', () => ({
  listAdminResource: vi.fn()
}));

vi.mock('../../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../../api/client')>('../../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const listAdminResourceMock = vi.mocked(listAdminResource);
const apiRequestMock = vi.mocked(apiRequest);

function renderWithQueryClient(element: ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={queryClient}>{element}</QueryClientProvider>);
}

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function stubBrowserLayoutApis() {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver');
  if (descriptor?.configurable === false) {
    if ('writable' in descriptor && descriptor.writable) {
      (globalThis as typeof globalThis & { ResizeObserver: typeof ResizeObserverMock }).ResizeObserver = ResizeObserverMock;
    }
  } else {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
  }
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

const productRecord: ApiRecord = {
  id: 71,
  revision: 7,
  status: 'active',
  asset_id: 11,
  asset_symbol: 'USDT',
  loan_type: 'credit',
  name: '30日信用贷',
  name_json: {
    version: 1,
    default_locale: 'zh-CN',
    items: [{ locale: 'zh-CN', country: 'CN', title: '30日信用贷' }]
  },
  term_days: 30,
  interest_rate: '0.02',
  interest_calculation_mode: 'full_term',
  min_kyc_level: 1,
  min_amount: '10',
  max_amount: '1000',
  user_principal_limit: null,
  product_principal_capacity: null,
  deny_borrowing_while_overdue: false
};

function rowHelpers() {
  return {
    loadDetail: vi.fn(), openDetail: vi.fn(),
    reload: vi.fn()
  };
}

describe('loan product revision actions', () => {
  beforeEach(() => {
    stubBrowserLayoutApis();
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue({ ...productRecord, revision: 8 });
    listAdminResourceMock.mockReset();
    listAdminResourceMock.mockImplementation(async (endpoint) => {
      if (endpoint === '/admin/api/v1/assets') {
        return { rows: [{ id: 11, symbol: 'USDT', name: 'Tether', precision_scale: 18 }], raw: {} };
      }
      if (endpoint === '/admin/api/v1/countries') {
        return {
          rows: [{ country_code: 'CN', country_name: '中国', default_locale: 'zh-CN' }],
          raw: {}
        };
      }
      return { rows: [], raw: {} };
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('sends the row revision and trimmed reason when changing product status', async () => {
    const user = userEvent.setup();
    const helpers = rowHelpers();
    renderWithQueryClient(<LoanProductRowActions helpers={helpers} record={productRecord} />);

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(await screen.findByLabelText('操作原因'), '停售旧产品');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/loan/products/71/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: '停售旧产品', revision: 7 })
      });
    });
    expect(helpers.reload).toHaveBeenCalledTimes(1);
  });

  it('keeps the list revision in the complete edit payload', async () => {
    const user = userEvent.setup();
    const helpers = rowHelpers();
    renderWithQueryClient(<LoanProductRowActions helpers={helpers} record={productRecord} />);

    await user.click(screen.getByRole('button', { name: '修改' }));
    await user.click(await screen.findByRole('button', { name: '提交修改' }));
    await user.type(await screen.findByLabelText('操作原因'), '调整产品配置');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/loan/products/71',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(
      ([path, init]) => path === '/admin/api/v1/loan/products/71' && init?.method === 'PATCH'
    )!;
    expect(JSON.parse(String(request?.body))).toEqual(
      expect.objectContaining({
        asset_id: 11,
        name: '30日信用贷',
        reason: '调整产品配置',
        revision: 7,
        status: 'active'
      })
    );
    expect(helpers.reload).toHaveBeenCalledTimes(1);
  });

  it('refreshes the list and shows a Chinese recovery instruction on HTTP 409', async () => {
    const user = userEvent.setup();
    const helpers = rowHelpers();
    const toastError = vi.spyOn(Toast, 'error');
    apiRequestMock.mockRejectedValue(
      new ApiError(409, 'CONFLICT', 'conflict: loan product revision is stale')
    );
    renderWithQueryClient(<LoanProductRowActions helpers={helpers} record={productRecord} />);

    await user.click(screen.getByRole('button', { name: '禁用' }));
    await user.type(await screen.findByLabelText('操作原因'), '尝试停售');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith(LOAN_PRODUCT_REVISION_CONFLICT_MESSAGE);
      expect(helpers.reload).toHaveBeenCalledTimes(1);
    });
  });

  it('does not offer write actions when a legacy row has no revision', () => {
    const helpers = rowHelpers();
    const legacyRecord = { ...productRecord };
    delete legacyRecord.revision;
    renderWithQueryClient(<LoanProductRowActions helpers={helpers} record={legacyRecord} />);

    expect(screen.getByRole('button', { name: '修改' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '禁用' })).toBeDisabled();
  });

  it('roundtrips exact exposure decimals and the overdue policy through the existing edit form', async () => {
    const user = userEvent.setup();
    const collateralPolicy = {
      initial_ltv: '0.50',
      maintenance_ltv: '0.70',
      liquidation_ltv: '0.90',
      collateral_assets: [{ collateral_asset_id: 12, oracle_symbol: 'BTCUSDT', oracle_source: 'market_ticker_redis', oracle_max_age_seconds: 30 }]
    };
    renderWithQueryClient(<LoanProductRowActions helpers={rowHelpers()} record={{
      ...productRecord,
      ...collateralPolicy,
      loan_type: 'collateralized',
      user_principal_limit: '9007199254740993.000000000000000001',
      product_principal_capacity: '0',
      deny_borrowing_while_overdue: true
    }} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    expect(await screen.findByLabelText('用户同币种本金上限')).toHaveValue('9007199254740993.000000000000000001');
    expect(screen.getByLabelText('产品本金容量')).toHaveValue('0');
    expect(screen.getByRole('switch', { name: '逾期未结禁止新增借款' })).toBeChecked();
    await waitFor(() => expect(screen.getByRole('button', { name: '提交修改' })).toBeEnabled());
    await user.click(screen.getByRole('button', { name: '提交修改' }));
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '精确限额' } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledTimes(1));
    expect(JSON.parse(String(apiRequestMock.mock.calls[0][1]?.body))).toMatchObject({
      user_principal_limit: '9007199254740993.000000000000000001',
      product_principal_capacity: '0',
      deny_borrowing_while_overdue: true,
      ...collateralPolicy,
      revision: 7
    });
  });

  it('rejects negative, malformed, overprecision and overflow limits and clears blank values to null', async () => {
    const user = userEvent.setup();
    renderWithQueryClient(<LoanProductRowActions helpers={rowHelpers()} record={{
      ...productRecord, user_principal_limit: '10', product_principal_capacity: '20',
      deny_borrowing_while_overdue: true
    }} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    const limit = await screen.findByLabelText('用户同币种本金上限');
    const capacity = screen.getByLabelText('产品本金容量');
    for (const input of [limit, capacity]) {
      for (const value of ['-1', '1oops', '0.0000000000000000001', '100000000000000000000']) {
        fireEvent.change(input, { target: { value } });
        expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
      }
      fireEvent.change(input, { target: { value: '' } });
    }
    await user.click(screen.getByRole('switch', { name: '逾期未结禁止新增借款' }));
    await user.click(screen.getByRole('button', { name: '提交修改' }));
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '取消限制' } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledTimes(1));
    expect(JSON.parse(String(apiRequestMock.mock.calls[0][1]?.body))).toMatchObject({
      user_principal_limit: null, product_principal_capacity: null,
      deny_borrowing_while_overdue: false
    });
  });

  it('does not silently convert a malformed or missing exposure configuration into a disabled policy', () => {
    const record = { ...productRecord, user_principal_limit: 9007199254740992 };
    renderWithQueryClient(<LoanProductRowActions helpers={rowHelpers()} record={record} />);
    expect(screen.getByRole('button', { name: '修改' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '禁用' })).toBeEnabled();
  });

  it('uses the selected asset precision without rounding a new limit', async () => {
    listAdminResourceMock.mockImplementation(async (endpoint) => ({
      rows: endpoint === '/admin/api/v1/assets'
        ? [{ id: 11, symbol: 'USDT', name: 'Tether', precision_scale: 2 }]
        : [{ country_code: 'CN', country_name: '中国', default_locale: 'zh-CN' }],
      raw: {}
    }));
    const user = userEvent.setup();
    renderWithQueryClient(<LoanProductRowActions helpers={rowHelpers()} record={productRecord} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    const limit = await screen.findByLabelText('用户同币种本金上限');
    fireEvent.change(limit, { target: { value: '1.001' } });
    expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
    fireEvent.change(limit, { target: { value: '1.2300' } });
    await waitFor(() => expect(screen.getByRole('button', { name: '提交修改' })).toBeEnabled());
    expect(apiRequestMock).not.toHaveBeenCalled();
  });
});
