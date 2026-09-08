import { Button, Toast } from '@douyinfe/semi-ui';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listAdminResource } from '../../api/adminResources';
import { apiRequest } from '../../api/client';
import { AdminResourcePage, type AdminResourceColumn } from './AdminResourcePage';
import { openRecordDetail, type RowActionHelpers } from './actions/shared';
import { UserRowActions } from './actions/users';
import { AgentCommissionBatchActions } from './actions/agents';

vi.mock('../../api/adminResources', () => ({ listAdminResource: vi.fn() }));
vi.mock('../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../api/client')>('../../api/client'),
  apiRequest: vi.fn()
}));

const listMock = vi.mocked(listAdminResource);
const requestMock = vi.mocked(apiRequest);
type Row = { id: number; name: string; status: string };
const columns: Array<AdminResourceColumn<Row>> = [{ key: 'name', title: '名称' }];
const rows: Row[] = [{ id: 1, name: '列表甲', status: 'pending' }, { id: 2, name: '列表乙', status: 'pending' }];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function rowActions(record: Row, helpers: RowActionHelpers) {
  return <>
    <Button onClick={() => openRecordDetail('/admin/detail', String(record.id), helpers)}>异步{record.id}</Button>
    <Button onClick={() => helpers.openDetail({ title: '本地详情', data: { name: `本地${record.id}` } })}>本地{record.id}</Button>
  </>;
}

function page(endpoint = '/admin/list') {
  return <AdminResourcePage<Row> columns={columns} endpoint={endpoint} responseKey="items" rowActions={rowActions} title="资源" />;
}

async function settle<T>(pending: ReturnType<typeof deferred<T>>, value: T) {
  await act(async () => { pending.resolve(value); await pending.promise; });
}

function pagerItem(text: string): HTMLElement {
  const item = [...document.querySelectorAll<HTMLElement>('.semi-page-item')].find((node) => node.textContent?.trim() === text);
  expect(item).toBeDefined();
  return item!;
}

beforeEach(() => {
  listMock.mockReset().mockResolvedValue({ rows, raw: {}, total: 120 });
  requestMock.mockReset();
  vi.spyOn(Toast, 'error').mockImplementation(() => 'error');
});
afterEach(() => vi.restoreAllMocks());

describe('resource-owned detail requests', () => {
  it('keeps the newest row intent when earlier detail resolves last', async () => {
    const first = deferred<Row>();
    const second = deferred<Row>();
    requestMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    render(page());
    await userEvent.click(await screen.findByRole('button', { name: '异步1' }));
    await userEvent.click(screen.getByRole('button', { name: '异步2' }));
    await settle(second, { ...rows[1], name: '详情乙' });
    expect(screen.getByText('详情乙')).toBeInTheDocument();
    await settle(first, { ...rows[0], name: '过期详情甲' });
    expect(screen.queryByText('过期详情甲')).not.toBeInTheDocument();
    expect(screen.getByText('详情乙')).toBeInTheDocument();
    expect(requestMock.mock.calls[0][1]?.signal?.aborted).toBe(true);
  });

  it('closing the current drawer invalidates a pending replacement', async () => {
    const pending = deferred<Row>();
    requestMock.mockReturnValueOnce(pending.promise);
    render(page());
    await userEvent.click(await screen.findByRole('button', { name: '本地2' }));
    await userEvent.click(screen.getByRole('button', { name: '异步1' }));
    const close = document.querySelector<HTMLElement>('.semi-sidesheet-close');
    expect(close).toBeInTheDocument();
    fireEvent.click(close!);
    await settle(pending, { ...rows[0], name: '关闭后详情' });
    expect(screen.queryByText('关闭后详情')).not.toBeInTheDocument();
    expect(requestMock.mock.calls[0][1]?.signal?.aborted).toBe(true);
  });

  it('opening static detail supersedes a pending network detail', async () => {
    const pending = deferred<Row>();
    requestMock.mockReturnValueOnce(pending.promise);
    render(page());
    await userEvent.click(await screen.findByRole('button', { name: '异步1' }));
    await userEvent.click(screen.getByRole('button', { name: '本地2' }));
    await settle(pending, { ...rows[0], name: '过期网络详情' });
    expect(screen.queryByText('过期网络详情')).not.toBeInTheDocument();
    expect(within(document.querySelector<HTMLElement>('.admin-detail-drawer')!).getByText('本地2')).toBeInTheDocument();
  });

  it.each(['reload', 'resource', 'unmount'] as const)('aborts pending detail on %s', async (change) => {
    const pending = deferred<Row>();
    requestMock.mockReturnValueOnce(pending.promise);
    const view = render(page());
    await userEvent.click(await screen.findByRole('button', { name: '异步1' }));
    if (change === 'reload') await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    if (change === 'resource') view.rerender(page('/admin/other'));
    if (change === 'unmount') view.unmount();
    expect(requestMock.mock.calls[0][1]?.signal?.aborted).toBe(true);
    await settle(pending, { ...rows[0], name: '过期上下文详情' });
    expect(screen.queryByText('过期上下文详情')).not.toBeInTheDocument();
  });

  it.each(['newer', 'close', 'reload', 'unmount'] as const)('consumes obsolete detail failures after %s without a toast', async (change) => {
    const pending = deferred<Row>();
    requestMock.mockReturnValueOnce(pending.promise).mockResolvedValueOnce({ ...rows[1], name: '当前详情' });
    const view = render(page());
    await userEvent.click(await screen.findByRole('button', { name: '本地2' }));
    await userEvent.click(screen.getByRole('button', { name: '异步1' }));
    if (change === 'newer') await userEvent.click(screen.getByRole('button', { name: '异步2' }));
    if (change === 'close') fireEvent.click(document.querySelector<HTMLElement>('.semi-sidesheet-close')!);
    if (change === 'reload') await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    if (change === 'unmount') view.unmount();
    await act(async () => { pending.reject(new Error('obsolete failure')); });
    expect(Toast.error).not.toHaveBeenCalled();
    if (change === 'newer') expect(screen.getByText('当前详情')).toBeInTheDocument();
  });

  it('surfaces a current detail failure once and consumes the rejected request', async () => {
    const pending = deferred<Row>();
    requestMock.mockReturnValueOnce(pending.promise);
    render(page());
    await userEvent.click(await screen.findByRole('button', { name: '异步1' }));
    await act(async () => { pending.reject(new Error('current failure')); });
    expect(Toast.error).toHaveBeenCalledExactlyOnceWith(expect.stringContaining('current failure'));
    expect(document.querySelector('.admin-detail-drawer')).not.toBeInTheDocument();
  });

  it('uses the same request owner for custom user assets and standard detail', async () => {
    const assets = deferred<{ accounts: Row[] }>();
    const detail = deferred<Row>();
    requestMock.mockReturnValueOnce(assets.promise).mockReturnValueOnce(detail.promise);
    render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <AdminResourcePage<Row> columns={columns} endpoint="/admin/list" responseKey="items" rowActions={(record, helpers) => <UserRowActions record={record} helpers={helpers} />} title="用户" />
    </QueryClientProvider>);
    const firstRow = (await screen.findByText('列表甲')).closest('tr')!;
    await userEvent.click(within(firstRow).getByRole('button', { name: '查看资产' }));
    await userEvent.click(within(firstRow).getByRole('button', { name: '查看详情' }));
    await settle(detail, { ...rows[0], name: '权威用户详情' });
    await settle(assets, { accounts: [{ ...rows[0], name: '过期资产详情' }] });
    expect(screen.queryByText('过期资产详情')).not.toBeInTheDocument();
    expect(screen.getByText('权威用户详情')).toBeInTheDocument();
    expect(requestMock.mock.calls[0][1]?.signal?.aborted).toBe(true);
  });
});

describe('resource selection ownership', () => {
  it.each(['page', 'page-size', 'filter', 'toolbar', 'reload'] as const)('invalidates selection and CSV before a pending %s request and after failure', async (change) => {
    const pending = deferred<Awaited<ReturnType<typeof listAdminResource>>>();
    listMock.mockResolvedValueOnce({ rows, raw: {}, total: 120 }).mockReturnValueOnce(pending.promise);
    render(<AdminResourcePage<Row> columns={columns} endpoint="/admin/list" responseKey="items" title="资源" serverPaged csvFileName="rows.csv"
      filters={[{ key: 'keyword', label: '关键词' }]} toolbarFilters={[{ key: 'enabled', label: '仅启用', type: 'switch' }]}
      batchActions={{ render: ({ selectedRows }) => <output aria-label="选择目标">{selectedRows.map((row) => row.id).join(',')}</output> }} />);
    await screen.findByText('列表甲');
    await userEvent.click(screen.getAllByRole('checkbox').at(-1)!);
    expect(screen.getByLabelText('选择目标')).toHaveTextContent('2');
    if (change === 'page') await userEvent.click(pagerItem('2'));
    if (change === 'page-size') {
      const sizeSelect = document.querySelector<HTMLElement>('.semi-page-switch .semi-select');
      expect(sizeSelect).toBeInTheDocument();
      await userEvent.click(sizeSelect!);
      const option = await screen.findByRole('option', { name: /每页条数：20/ });
      const popup = option.closest('[x-placement]');
      expect(popup).toBeInTheDocument();
      // Finish the real enter/leave lifecycle; React uses the WebKit event in jsdom.
      fireEvent.animationEnd(popup!);
      fireEvent(popup!, new Event('webkitAnimationEnd', { bubbles: true }));
      await userEvent.click(option);
      // Native Pagination Select publishes controlled changes on its CSS leave event.
      fireEvent.animationEnd(popup!);
      fireEvent(popup!, new Event('webkitAnimationEnd', { bubbles: true }));
    }
    if (change === 'filter') {
      fireEvent.change(screen.getByLabelText('关键词'), { target: { value: '  alice  ' } });
      await userEvent.click(screen.getByRole('button', { name: '查询' }));
    }
    if (change === 'toolbar') await userEvent.click(screen.getByRole('switch', { name: '仅启用' }));
    if (change === 'reload') await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => expect(listMock).toHaveBeenCalledTimes(2));
    expect(screen.getByLabelText('选择目标')).toBeEmptyDOMElement();
    expect(screen.getByRole('button', { name: '导出已加载数据' })).toBeDisabled();
    await act(async () => { pending.reject(new Error('list failure')); });
    expect(screen.getByRole('alert')).toHaveTextContent('list failure');
    expect(screen.getByLabelText('选择目标')).toBeEmptyDOMElement();
  });

  it('clears local-page selections rather than retaining invisible rows', async () => {
    listMock.mockResolvedValue({ rows: Array.from({ length: 12 }, (_, id) => ({ id: id + 1, name: `本地行${id + 1}`, status: 'pending' })), raw: {} });
    render(<AdminResourcePage<Row> columns={columns} endpoint="/admin/list" responseKey="items" title="资源"
      batchActions={{ render: ({ selectedRows }) => <output aria-label="选择目标">{selectedRows.map((row) => row.id).join(',')}</output> }} />);
    await screen.findByText('本地行1');
    await userEvent.click(screen.getAllByRole('checkbox').at(-1)!);
    expect(screen.getByLabelText('选择目标')).toHaveTextContent('10');
    await userEvent.click(pagerItem('2'));
    expect(screen.getByLabelText('选择目标')).toBeEmptyDOMElement();
    expect(screen.getByText('本地行11')).toBeInTheDocument();
  });

  it('blocks an already-open batch confirmation while the list reloads', async () => {
    const pending = deferred<Awaited<ReturnType<typeof listAdminResource>>>();
    listMock.mockResolvedValueOnce({ rows, raw: {}, total: 120 }).mockReturnValueOnce(pending.promise);
    render(<AdminResourcePage<Row> columns={columns} endpoint="/admin/list" responseKey="items" title="资源" serverPaged
      batchActions={{ render: (helpers) => <AgentCommissionBatchActions helpers={helpers} /> }} />);
    await screen.findByText('列表甲');
    await userEvent.click(screen.getAllByRole('checkbox').at(-1)!);
    await userEvent.click(screen.getByRole('button', { name: '批量结算' }));
    await screen.findByRole('button', { name: '确认批量结算' });
    await userEvent.click(screen.getByRole('button', { name: '刷新' }));
    expect(screen.getByRole('button', { name: '确认批量结算' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '确认批量结算' }));
    expect(requestMock).not.toHaveBeenCalled();
    await settle(pending, { rows: [rows[0]], raw: {}, total: 1 });
    expect(screen.getByRole('button', { name: '确认批量结算' })).toBeDisabled();
  });
});
