import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AdminResourcePage, type AdminResourceColumn } from './AdminResourcePage';
import { listAdminResource } from '../../api/adminResources';

vi.mock('../../api/adminResources', () => ({
  listAdminResource: vi.fn()
}));

const listAdminResourceMock = vi.mocked(listAdminResource);

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

type TestRecord = {
  id: number;
  name: string;
  enabled: boolean;
  amount: string;
  created_at: number;
  fee_rate?: string;
  market_type?: string;
  metadata?: Record<string, unknown>;
  order_count?: number;
  total_balance?: string;
  user_id?: number;
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

  it('loads and renders resource rows with shared formatters without static helper copy', async () => {
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
    expect(screen.getByText('123.45 USDT')).toBeInTheDocument();
    expect(screen.getByText(/^2025年1月1日/)).toBeInTheDocument();
    expect(screen.queryByText('后台资源检索视图，敏感操作需走二次确认。')).not.toBeInTheDocument();
    expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/accounts', 'items', {});
  });

  it('fixes the operation column on the right side', async () => {
    listAdminResourceMock.mockResolvedValueOnce({
      rows: [
        {
          id: 9,
          name: '固定操作列',
          enabled: true,
          amount: '10.0000',
          created_at: 1_735_732_800_000
        }
      ],
      raw: { items: [] }
    });

    render(<AdminResourcePage<TestRecord> title="管理员资源" endpoint="/admin/accounts" responseKey="items" columns={columns} />);

    expect(await screen.findByText('固定操作列')).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: '操作' })).toHaveClass('semi-table-cell-fixed-right');
    expect(screen.getByRole('button', { name: '查看详情' }).closest('td')).toHaveClass('semi-table-cell-fixed-right');
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

  it('reloads with selected filter values', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockResolvedValue({ rows: [], raw: { items: [] } });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={columns}
        filters={[
          {
            key: 'status',
            label: '状态',
            type: 'select',
            options: [
              { label: '启用', value: 'active' },
              { label: '禁用', value: 'disabled' }
            ]
          }
        ]}
      />
    );

    await screen.findByText('暂无数据');
    semiSelectByLabel('状态');
    await selectSemiOption(user, '状态', '禁用');
    expect(semiSelectByLabel('状态')).toHaveTextContent('禁用');
    await user.click(screen.getByRole('button', { name: '查询' }));

    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenLastCalledWith('/admin/accounts', 'items', { status: 'disabled' });
    });
  });

  it('renders mapped column values', async () => {
    listAdminResourceMock.mockResolvedValueOnce({
      rows: [{ id: 4, name: 'BTC-USDT', enabled: true, amount: '1.0000', created_at: 1_735_732_800_000, market_type: 'external' }],
      raw: { items: [] }
    });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={[...columns, { key: 'market_type', title: '市场类型', valueMap: { external: '外部行情' } }]}
      />
    );

    expect(await screen.findByText('外部行情')).toBeInTheDocument();
    expect(screen.queryByText('external')).not.toBeInTheDocument();
  });

  it('formats business numeric columns without changing IDs or timestamps', async () => {
    listAdminResourceMock.mockResolvedValueOnce({
      rows: [
        {
          id: 1234,
          user_id: 5678,
          name: '数值资源',
          enabled: true,
          amount: '1.0000',
          total_balance: '1234.5',
          order_count: 1200,
          fee_rate: '0.123456',
          created_at: 1_735_732_800_000
        }
      ],
      raw: { items: [] }
    });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={[
          ...columns,
          { key: 'user_id', title: '用户ID' },
          { key: 'total_balance', title: '总余额' },
          { key: 'order_count', title: '订单数' },
          { key: 'fee_rate', title: '费率' }
        ]}
      />
    );

    expect(await screen.findByText('数值资源')).toBeInTheDocument();
    expect(screen.getByText('1,234.50')).toBeInTheDocument();
    expect(screen.getByText('1,200.00')).toBeInTheDocument();
    expect(screen.getByText('0.123456')).toBeInTheDocument();
    expect(screen.getByText('1234')).toBeInTheDocument();
    expect(screen.getByText('5678')).toBeInTheDocument();
    expect(screen.queryByText('5,678.00')).not.toBeInTheDocument();
    expect(screen.getByText(/^2025年1月1日/)).toBeInTheDocument();
  });

  it('opens a formatted detail drawer for the selected row', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockResolvedValueOnce({
      rows: [
        {
          id: 2,
          user_id: 42,
          name: '风控员',
          enabled: false,
          amount: '0.0001',
          total_balance: '1234.5',
          metadata: { order_count: 1200, user_id: 42 },
          created_at: 1_735_732_800_000
        }
      ],
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
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));

    expect(await screen.findByText('字段')).toBeInTheDocument();
    expect(screen.getByText('详情').closest('.semi-sidesheet-inner')).toHaveStyle({ width: '80%' });
    expect(screen.getByText('内容')).toBeInTheDocument();
    expect(screen.getAllByText('1,234.50').length).toBeGreaterThan(0);
    expect(screen.getByText(/order_count: 1,200.00/)).toBeInTheDocument();
    expect(screen.getByText(/用户ID: 42/)).toBeInTheDocument();
    expect(screen.queryByText(/用户ID: 42.00/)).not.toBeInTheDocument();
    expect(screen.queryByText(/"name": "风控员"/)).not.toBeInTheDocument();
  });

  it('renders custom row actions without the default detail action', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockResolvedValue({
      rows: [{ id: 3, name: '交易对', enabled: true, amount: '1.0000', created_at: 1_735_732_800_000 }],
      raw: { items: [] }
    });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={columns}
        rowActions={(record, helpers) => (
          <>
            <button type="button" onClick={() => helpers.openDetail({ title: '详情', data: { detail: record.name } })}>
              查看详情
            </button>
            <button type="button" onClick={helpers.reload}>
              重新加载
            </button>
          </>
        )}
      />
    );

    await screen.findByText('交易对');
    expect(screen.getAllByRole('button', { name: '查看详情' })).toHaveLength(1);
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '查看详情' }));
    expect(await screen.findByText('detail')).toBeInTheDocument();
    expect(screen.queryByText(/"detail": "交易对"/)).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '重新加载' }));
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenCalledTimes(2);
    });
  });

  it('lets header actions trigger a resource reload', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockResolvedValue({
      rows: [{ id: 6, name: '可刷新资源', enabled: true, amount: '1.0000', created_at: 1_735_732_800_000 }],
      raw: { items: [] }
    });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={columns}
        actions={({ reload }) => (
          <button type="button" onClick={reload}>
            刷新资源
          </button>
        )}
      />
    );

    await screen.findByText('可刷新资源');
    await user.click(screen.getByRole('button', { name: '刷新资源' }));

    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenCalledTimes(2);
    });
  });

  it('can hide the default detail action while custom actions still open details', async () => {
    const user = userEvent.setup();
    listAdminResourceMock.mockResolvedValue({
      rows: [{ id: 5, name: '交易对详情', enabled: true, amount: '1.0000', created_at: 1_735_732_800_000 }],
      raw: { items: [] }
    });

    render(
      <AdminResourcePage<TestRecord>
        title="管理员资源"
        endpoint="/admin/accounts"
        responseKey="items"
        columns={columns}
        showJsonAction={false}
        rowActions={(record, helpers) => (
          <button type="button" onClick={() => helpers.openDetail({ title: '详情', data: { detail: record.name } })}>
            查看详情
          </button>
        )}
      />
    );

    await screen.findByText('交易对详情');
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看详情' }));
    expect(await screen.findByText('detail')).toBeInTheDocument();
    expect(screen.queryByText(/"detail": "交易对详情"/)).not.toBeInTheDocument();
  });
});
