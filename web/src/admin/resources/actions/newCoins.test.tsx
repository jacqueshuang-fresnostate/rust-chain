import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, useLocation } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import type { RowActionHelpers } from './shared';
import { NewCoinProjectRowActions } from './newCoins';

vi.mock('../../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../../api/client')>('../../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const apiRequestMock = vi.mocked(apiRequest);

function LocationProbe() {
  const location = useLocation();
  return <output aria-label="当前路由">{`${location.pathname}${location.search}${location.hash}`}</output>;
}

function renderRow(record: ApiRecord, helpers: RowActionHelpers) {
  return render(
    <MemoryRouter initialEntries={['/admin/new-coins/projects']}>
      <NewCoinProjectRowActions helpers={helpers} record={record} />
      <LocationProbe />
    </MemoryRouter>
  );
}

describe('NewCoinProjectRowActions', () => {
  let helpers: RowActionHelpers;

  beforeEach(() => {
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue({});
    helpers = { openDetail: vi.fn(), reload: vi.fn() };
  });

  it('将启用的预热项目直接推进为申购中并重载列表', async () => {
    const user = userEvent.setup();
    renderRow({ id: 7, lifecycle_status: 'preheat', status: 'active', symbol: 'HIP' }, helpers);

    await user.click(screen.getByRole('button', { name: '开始申购 HIP（ID: 7）' }));
    await user.type(await screen.findByLabelText('操作原因'), 'open subscription');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/new-coins/7/lifecycle', {
        method: 'PATCH',
        body: JSON.stringify({ lifecycle_status: 'subscription', reason: 'open subscription' })
      });
      expect(helpers.reload).toHaveBeenCalledTimes(1);
    });
  });

  it('只为预热项目提供开始申购，且禁用状态不可执行', () => {
    const { rerender } = renderRow(
      { id: 7, lifecycle_status: 'subscription', status: 'active', symbol: 'HIP' },
      helpers
    );

    expect(screen.queryByRole('button', { name: '开始申购 HIP（ID: 7）' })).not.toBeInTheDocument();

    rerender(
      <MemoryRouter initialEntries={['/admin/new-coins/projects']}>
        <NewCoinProjectRowActions
          helpers={helpers}
          record={{ id: 8, lifecycle_status: 'preheat', status: 'disabled', symbol: 'LOCK' }}
        />
        <LocationProbe />
      </MemoryRouter>
    );

    expect(screen.getByRole('button', { name: '开始申购 LOCK（ID: 8）' })).toBeDisabled();
    expect(apiRequestMock).not.toHaveBeenCalled();
  });

  it('通过 BrowserRouter 路径打开项目配置页而不是写入 hash', async () => {
    const user = userEvent.setup();
    renderRow({ id: 7, lifecycle_status: 'preheat', status: 'active', symbol: 'HIP' }, helpers);

    await user.click(screen.getByRole('button', { name: '配置新币项目 HIP（ID: 7）' }));

    expect(screen.getByLabelText('当前路由')).toHaveTextContent('/admin/new-coins/projects/7');
    expect(screen.getByLabelText('当前路由')).not.toHaveTextContent('#');
  });
});
