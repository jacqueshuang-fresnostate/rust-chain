import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PlatformBrandPage } from './PlatformBrandPage';
import { apiRequest } from '../../api/client';

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
    render(<PlatformBrandPage />);

    expect(await screen.findByDisplayValue('Hippo Exchange')).toBeInTheDocument();
    expect(screen.getByDisplayValue('https://cdn.example.test/logo.png')).toHaveAccessibleName('PC Logo');
    expect(semiSelectByLabel('K线图引擎')).toHaveTextContent('系统 K 线');
    expect(screen.getByRole('img', { name: 'Hippo Exchange' })).toHaveAttribute('src', 'https://cdn.example.test/logo.png');
    expect(screen.getByText('PC 端预览')).toBeInTheDocument();
  });

  it('saves platform name and logo URL with an operation reason', async () => {
    const user = userEvent.setup();
    render(<PlatformBrandPage />);

    await user.clear(await screen.findByLabelText('平台名称'));
    await user.type(screen.getByLabelText('平台名称'), 'Rust Chain');
    await user.clear(screen.getByLabelText('PC Logo'));
    await user.type(screen.getByLabelText('PC Logo'), 'https://cdn.example.test/new-logo.png');
    await selectSemiOption(user, 'K线图引擎', 'TradingView Lightweight Charts');
    await user.click(screen.getByRole('button', { name: '保存品牌配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'update pc brand');
    await user.click(screen.getByRole('button', { name: '确认' }));

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
  });
});
