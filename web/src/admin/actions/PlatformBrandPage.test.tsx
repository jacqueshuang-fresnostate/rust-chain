import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PlatformBrandPage } from './PlatformBrandPage';
import { ApiError, apiRequest } from '../../api/client';

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

vi.mock('../../shared/AdminImageUpload', () => ({
  AdminImageUpload: ({ label, onChange, value }: { label: string; onChange: (value: string) => void; value: string }) => (
    <label>
      {label}
      <input aria-label={label} onChange={(event) => onChange(event.currentTarget.value)} value={value} />
    </label>
  )
}));

const apiRequestMock = vi.mocked(apiRequest);

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

const originalResizeObserver = globalThis.ResizeObserver;

const brandConfig = {
  created_at: 1_735_732_700_000,
  id: 1,
  logo_url: 'https://cdn.example.test/logo.png',
  name: 'default',
  platform_name: 'Hippo Exchange',
  chart_provider: 'klinecharts',
  updated_at: 1_735_732_800_000,
  updated_by: 9
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

function renderPlatformBrandPage() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { gcTime: 0, retry: false },
      mutations: { retry: false }
    }
  });
  const router = createMemoryRouter(
    [
      { path: '/brand', element: <PlatformBrandPage /> },
      { path: '/other', element: <div>其他页面</div> }
    ],
    { initialEntries: ['/brand'] }
  );
  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
  return { queryClient, router, ...view };
}

describe('PlatformBrandPage', () => {
  beforeEach(() => {
    if (!globalThis.ResizeObserver) {
      Object.defineProperty(globalThis, 'ResizeObserver', {
        configurable: true,
        value: ResizeObserverMock
      });
    }
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/platform/brand' && !init?.method) {
        return Promise.resolve(brandConfig);
      }
      if (path === '/admin/api/v1/platform/brand' && init?.method === 'PATCH') {
        return Promise.resolve({
          ...brandConfig,
          chart_provider: 'tradingview',
          logo_url: 'https://cdn.example.test/new-logo.png',
          platform_name: 'Rust Chain'
        });
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    if (!originalResizeObserver) {
      Reflect.deleteProperty(globalThis, 'ResizeObserver');
    }
  });

  it('loads and previews the saved PC brand config', async () => {
    renderPlatformBrandPage();

    expect(await screen.findByDisplayValue('Hippo Exchange')).toBeInTheDocument();
    expect(screen.getByDisplayValue('https://cdn.example.test/logo.png')).toHaveAccessibleName('PC Logo');
    expect(semiSelectByLabel('K线图引擎')).toHaveTextContent('系统 K 线');
    expect(screen.getByRole('img', { name: 'Hippo Exchange' })).toHaveAttribute('src', 'https://cdn.example.test/logo.png');
    expect(screen.getByText('PC 端预览')).toBeInTheDocument();
  });

  it('saves platform name and logo URL with an operation reason', async () => {
    const user = userEvent.setup();
    renderPlatformBrandPage();

    await user.clear(await screen.findByLabelText('平台名称'));
    await user.type(screen.getByLabelText('平台名称'), 'Rust Chain');
    await user.clear(screen.getByLabelText('PC Logo'));
    await user.type(screen.getByLabelText('PC Logo'), 'https://cdn.example.test/new-logo.png');
    await selectSemiOption(user, 'K线图引擎', 'TradingView Lightweight Charts');
    expect(screen.getByRole('status')).toHaveTextContent('有未保存的变更');
    await user.click(screen.getByRole('button', { name: '保存品牌配置' }));

    expect(await screen.findByText('字段差异（3 项）')).toBeInTheDocument();
    expect(screen.getByText('保存后将立即影响 PC 端平台名称、Logo 与 K 线图展示。')).toBeInTheDocument();
    expect(screen.getAllByText('平台名称').length).toBeGreaterThan(0);
    expect(screen.getAllByText('K线图引擎').length).toBeGreaterThan(0);
    expect(screen.getAllByText('PC Logo').length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: '确认保存' })).toBeDisabled();
    await user.type(await screen.findByLabelText('操作原因'), 'update pc brand');
    await user.click(await screen.findByRole('button', { name: '确认保存' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/platform/brand',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/platform/brand' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      chart_provider: 'tradingview',
      logo_url: 'https://cdn.example.test/new-logo.png',
      platform_name: 'Rust Chain',
      reason: 'update pc brand'
    });
    expect(await screen.findByText('PC 品牌配置已保存。')).toBeInTheDocument();
    expect(screen.queryByText('有未保存的变更')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '保存品牌配置' })).toBeDisabled();
  });

  it('keeps the brand draft and shows the unified Chinese conflict after a 409 response', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/platform/brand' && !init?.method) {
        return Promise.resolve(brandConfig);
      }
      if (path === '/admin/api/v1/platform/brand' && init?.method === 'PATCH') {
        return Promise.reject(new ApiError(409, 'CONFIG_CONFLICT', 'stale revision'));
      }
      return Promise.resolve({});
    });
    renderPlatformBrandPage();

    await user.clear(await screen.findByLabelText('平台名称'));
    await user.type(screen.getByLabelText('平台名称'), '本地品牌草稿');
    await user.click(screen.getByRole('button', { name: '保存品牌配置' }));
    await user.type(await screen.findByLabelText('操作原因'), '更新品牌');
    await user.click(screen.getByRole('button', { name: '确认保存' }));

    await waitFor(() => {
      expect(screen.getAllByText('配置已被其他管理员更新，当前草稿尚未覆盖；请重新加载最新配置后再修改。').length).toBeGreaterThan(0);
    });
    expect(screen.getByLabelText('平台名称')).toHaveValue('本地品牌草稿');
  });
});
