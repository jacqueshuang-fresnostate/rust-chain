import { fireEvent, render as testingLibraryRender, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import { optionalNewCoinLocalDateTimeMillis, requiredNewCoinLocalDateTimeMillis } from '../newCoinDateTime';
import { NewCoinActions } from './NewCoinActions';

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

function stubResizeObserver() {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver');
  if (descriptor?.configurable === false) {
    if ('writable' in descriptor && descriptor.writable) {
      (globalThis as typeof globalThis & { ResizeObserver: typeof ResizeObserverMock }).ResizeObserver = ResizeObserverMock;
    }
    return;
  }
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
}

function render(element: ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return testingLibraryRender(<QueryClientProvider client={queryClient}>{element}</QueryClientProvider>);
}

function semiSelectByLabel(label: string, index = 0): HTMLElement {
  const labelNode = [...document.querySelectorAll('label')]
    .filter((item) => item.textContent?.trim().startsWith(label) && item.querySelector('.semi-select'))[index] as HTMLElement | undefined;
  expect(labelNode).toBeDefined();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

async function selectSemiOption(
  user: ReturnType<typeof userEvent.setup>,
  label: string,
  optionLabel: string,
  index = 0
) {
  await user.click(semiSelectByLabel(label, index));
  await waitFor(() => {
    expect([...document.querySelectorAll('.semi-select-option')].some((option) => option.textContent === optionLabel)).toBe(true);
  });
  const option = [...document.querySelectorAll('.semi-select-option')]
    .filter((item) => item.textContent === optionLabel)
    .at(-1) as HTMLElement | undefined;
  expect(option).toBeDefined();
  fireEvent.mouseDown(option as HTMLElement);
  fireEvent.mouseUp(option as HTMLElement);
  fireEvent.click(option as HTMLElement);
  await waitFor(() => expect(semiSelectByLabel(label, index)).toHaveTextContent(optionLabel));
}

async function confirmWithReason(user: ReturnType<typeof userEvent.setup>, reason: string) {
  await user.type(await screen.findByLabelText('操作原因'), reason);
  await user.click(await screen.findByRole('button', { name: '确认' }));
}

describe('NewCoinActions', () => {
  beforeEach(() => {
    stubResizeObserver();
    window.location.hash = '';
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation(async (path) => {
      if (path.startsWith('/admin/api/v1/new-coins?')) {
        return {
          projects: [{ id: 7, asset_id: 11, symbol: 'HIP', lifecycle_status: 'distribution', status: 'active' }]
        };
      }
      if (path.startsWith('/admin/api/v1/users?')) {
        return { users: [] };
      }
      if (path.startsWith('/admin/api/v1/assets?')) {
        return { assets: [] };
      }
      return {};
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('将生命周期与解禁规则的本地日期时间转为 Unix 毫秒，并省略空的可选时间', async () => {
    const user = userEvent.setup();
    render(<NewCoinActions />);

    await waitFor(() => expect(semiSelectByLabel('新币项目', 0)).not.toHaveClass('semi-select-disabled'));
    const listedInputs = screen.getAllByLabelText('上市时间');
    expect(listedInputs).toHaveLength(2);
    expect(listedInputs[0]).toHaveAttribute('type', 'datetime-local');
    expect(listedInputs[1]).toHaveAttribute('type', 'datetime-local');
    expect(screen.getByLabelText('固定解禁时间')).toHaveAttribute('type', 'datetime-local');
    expect(screen.getByLabelText('相对解禁秒数')).not.toHaveAttribute('type', 'datetime-local');
    expect(screen.queryByLabelText('上市时间戳')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('固定解禁时间戳')).not.toBeInTheDocument();

    await selectSemiOption(user, '新币项目', 'HIP · 派发中 · 启用（ID: 7）', 0);
    fireEvent.change(listedInputs[0], { target: { value: '2026-11-10T09:15' } });
    await user.click(screen.getByRole('button', { name: '更新生命周期' }));
    await confirmWithReason(user, 'update lifecycle');
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/new-coins/7/lifecycle', {
        method: 'PATCH',
        body: JSON.stringify({
          lifecycle_status: 'subscription',
          listed_at: new Date(2026, 10, 10, 9, 15).getTime(),
          reason: 'update lifecycle'
        })
      });
    });

    await selectSemiOption(user, '新币项目', 'HIP · 派发中 · 启用（ID: 7）', 1);
    await selectSemiOption(user, '解禁类型', '固定时间解禁');
    fireEvent.change(screen.getByLabelText('固定解禁时间'), { target: { value: '2026-12-01T18:30' } });
    await user.click(screen.getByRole('button', { name: '更新解禁规则' }));
    await confirmWithReason(user, 'update unlock rule');
    await waitFor(() => {
      const request = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/new-coins/7/unlock-rule')?.[1];
      expect(request).toBeDefined();
      expect(JSON.parse(String(request?.body))).toEqual({
        unlock_type: 'fixed_time',
        fixed_unlock_at: new Date(2026, 11, 1, 18, 30).getTime(),
        reason: 'update unlock rule'
      });
    });
  });

  it('对必填空值和非法本地日期时间返回中文错误', () => {
    expect(requiredNewCoinLocalDateTimeMillis('2026-11-10T09:15:30.25', '上市时间'))
      .toBe(new Date(2026, 10, 10, 9, 15, 30, 250).getTime());
    expect(() => requiredNewCoinLocalDateTimeMillis('', '上市时间')).toThrow('上市时间不能为空');
    expect(() => requiredNewCoinLocalDateTimeMillis('2026-02-30T10:00', '上市时间')).toThrow('上市时间必须为有效日期时间');
    expect(() => optionalNewCoinLocalDateTimeMillis('invalid', '固定解禁时间')).toThrow('固定解禁时间必须为有效日期时间');
    expect(optionalNewCoinLocalDateTimeMillis('  ', '固定解禁时间')).toBeUndefined();
  });
});
