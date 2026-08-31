import { render, screen, waitFor } from '@testing-library/react';
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
  max_amount: '1000'
};

function rowHelpers() {
  return {
    openDetail: vi.fn(),
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
        return { rows: [{ id: 11, symbol: 'USDT', name: 'Tether' }], raw: {} };
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
});
