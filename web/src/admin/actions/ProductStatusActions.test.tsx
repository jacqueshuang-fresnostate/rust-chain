import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import { ProductStatusActions } from './ProductStatusActions';

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

describe('ProductStatusActions', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue({});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('does not expose spot, seconds, or margin action categories from the Earn action page', () => {
    render(<ProductStatusActions />);

    expect(screen.queryByRole('button', { name: '创建现货交易对' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '创建杠杆产品' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '创建秒合约产品' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('基础资产ID')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('杠杆交易对ID')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('秒合约交易对ID')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('产品模块')).not.toBeInTheDocument();
  });

  it('updates only Earn product status with a required reason', async () => {
    const user = userEvent.setup();
    render(<ProductStatusActions />);

    await user.type(screen.getByLabelText('理财产品ID'), '88');
    expect(screen.getByLabelText('理财产品ID').closest('.semi-input-wrapper')).toBeInTheDocument();
    semiSelectByLabel('目标状态');
    await selectSemiOption(user, '目标状态', '禁用');
    await user.click(screen.getByRole('button', { name: '更新理财产品状态' }));
    await user.type(screen.getByLabelText('操作原因'), 'disable earn product');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/earn/products/88/status', {
        method: 'PATCH',
        body: JSON.stringify({ status: 'disabled', reason: 'disable earn product' })
      });
    });
  });
});
