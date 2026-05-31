import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AdminResourcePage, type AdminResourceColumn } from './AdminResourcePage';
import { listAdminResource } from '../../api/adminResources';

vi.mock('../../api/adminResources', () => ({
  listAdminResource: vi.fn()
}));

const listAdminResourceMock = vi.mocked(listAdminResource);

type TestRecord = {
  id: number;
  name: string;
  enabled: boolean;
  amount: string;
  created_at: number;
};

const columns: Array<AdminResourceColumn<TestRecord>> = [
  { key: 'id', title: 'ID' },
  { key: 'name', title: '名称' },
  { key: 'enabled', title: '状态', type: 'status' },
  { key: 'amount', title: '数量', type: 'amount', asset: 'USDT' },
  { key: 'created_at', title: '创建时间', type: 'timestamp' }
];

describe('AdminResourcePage', () => {
  beforeEach(() => {
    listAdminResourceMock.mockReset();
  });

  it('loads and renders resource rows with shared formatters', async () => {
    listAdminResourceMock.mockResolvedValueOnce({
      rows: [
        {
          id: 1,
          name: '主账户',
          enabled: true,
          amount: '123.4500',
          created_at: 1_735_732_800_000
        }
      ],
      raw: { data: [] }
    });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={columns}
        filters={[{ key: 'keyword', label: '关键词' }]}
      />
    );

    expect(await screen.findByText('主账户')).toBeInTheDocument();
    expect(screen.getByText('启用')).toBeInTheDocument();
    expect(screen.getByText('123.4500 USDT')).toBeInTheDocument();
    expect(screen.getByText(/^2025年1月1日/)).toBeInTheDocument();
    expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/accounts', 'items', {});
  });

  it('reloads with non-empty filter values', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockResolvedValue({ rows: [], raw: { items: [] } });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={columns}
        filters={[{ key: 'keyword', label: '关键词' }]}
      />
    );

    await screen.findByText('暂无数据');
    await user.type(screen.getByLabelText('关键词'), 'alice');
    await user.click(screen.getByRole('button', { name: '查询' }));

    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/accounts', 'items', { keyword: 'alice' });
    });
  });

  it('opens a JSON drawer for the selected row', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockResolvedValueOnce({
      rows: [{ id: 2, name: '风控员', enabled: false, amount: '0.0001', created_at: 1_735_732_800_000 }],
      raw: { items: [] }
    });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={columns}
      />
    );

    await screen.findByText('风控员');
    await user.click(screen.getByRole('button', { name: '查看JSON' }));

    expect(await screen.findByText(/"name": "风控员"/)).toBeInTheDocument();
  });
});
