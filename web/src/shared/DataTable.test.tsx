import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { DataTable, normalizeTableColumns } from './DataTable';
import { containedTableScrollForColumns } from './tableLayout';

type Row = {
  id: number;
  name: string;
};

const rows: Row[] = Array.from({ length: 25 }, (_, index) => ({
  id: index + 1,
  name: `记录 ${index + 1}`
}));

const columns = [{ dataIndex: 'name', title: '名称' }];

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

describe('DataTable', () => {
  beforeEach(() => {
    stubResizeObserver();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('uses local controlled pagination with page size 10', async () => {
    render(<DataTable<Row> columns={columns} data={rows} />);

    expect(screen.getByText('记录 1')).toBeInTheDocument();
    expect(screen.getByText('记录 10')).toBeInTheDocument();
    expect(screen.queryByText('记录 11')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(screen.queryByText('记录 1')).not.toBeInTheDocument();
    expect(screen.getByText('记录 11')).toBeInTheDocument();
    expect(screen.getByText('记录 20')).toBeInTheDocument();
  });

  it('renders bordered tables with compact table styling by default', () => {
    render(<DataTable<Row> columns={columns} data={rows} />);

    const table = screen.getByRole('grid');
    const wrapper = table.closest('.semi-table-wrapper');
    expect(table.closest('.semi-table-bordered')).toBeInTheDocument();
    expect(wrapper).toHaveClass('admin-data-table', 'admin-business-table', 'admin-data-table-compact');
    expect(wrapper).toHaveStyle({ maxWidth: '100%', width: '100%' });
    expect(table.closest('.semi-table-bordered')).toHaveClass('semi-table-small');
    expect(document.querySelector('.admin-data-table')).toBeInTheDocument();
  });

  it('uses Semi small table density for compact mode', () => {
    render(<DataTable<Row> columns={columns} data={rows} displayMode="compact" />);

    const table = screen.getByRole('grid');
    expect(table.closest('.semi-table-wrapper')).toHaveClass('admin-data-table-compact');
    expect(table.closest('.semi-table-bordered')).toHaveClass('semi-table-small');
    expect(screen.getByRole('separator', { name: '调整名称列宽' })).toHaveAttribute('aria-valuenow', '160');
    expect(table.querySelector('.react-resizable-handle')).not.toBeInTheDocument();
  });

  it('uses project-controlled widths and handles in adaptive mode as well', () => {
    render(<DataTable<Row> columns={columns} data={rows} displayMode="adaptive" />);

    const table = screen.getByRole('grid');
    expect(table.closest('.semi-table-wrapper')).toHaveClass('admin-data-table-adaptive', 'admin-resizable-table');
    expect(table.closest('.semi-table-bordered')).not.toHaveClass('semi-table-small');
    expect(screen.getByRole('separator', { name: '调整名称列宽' })).toHaveAttribute('aria-valuenow', '160');
    expect(table).toHaveStyle({ width: '160px' });
  });

  it('keeps server pagination controlled by the caller', async () => {
    const onPageChange = vi.fn();
    const onPageSizeChange = vi.fn();
    render(
      <DataTable<Row>
        columns={columns}
        data={rows.slice(0, 10)}
        pagination={{ currentPage: 1, onPageChange, onPageSizeChange, pageSize: 10, total: 25 }}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(onPageChange).toHaveBeenCalledWith(2);
    expect(onPageSizeChange).not.toHaveBeenCalled();
    expect(screen.getByText('记录 1')).toBeInTheDocument();
  });

  it('keeps the framework selection column unhandled while including its width in scroll.x', () => {
    render(<DataTable<Row> columns={columns} data={rows} rowSelection={{}} />);

    const table = screen.getByRole('grid');
    expect(table).toHaveStyle({ width: '208px' });
    expect(screen.getAllByRole('columnheader')).toHaveLength(2);
    expect(screen.getAllByRole('separator')).toHaveLength(1);
    expect(screen.queryByRole('separator', { name: /Select all rows/ })).not.toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole('separator', { name: '调整名称列宽' }), { key: 'ArrowRight' });
    expect(table).toHaveStyle({ width: '224px' });
  });

  it('normalizes missing column widths without changing existing column props', () => {
    const fixedRender = () => '操作';
    const normalized = normalizeTableColumns<Row>([
      { dataIndex: 'name', title: '名称' },
      { dataIndex: 'id', fixed: 'right', render: fixedRender, title: '操作', width: 300 }
    ], 'compact');

    expect(normalized[0]).toMatchObject({ dataIndex: 'name', title: '名称', width: 160 });
    expect(normalized[1]).toMatchObject({ dataIndex: 'id', fixed: 'right', title: '操作', width: 300 });
    expect(normalized[1].render).toBe(fixedRender);
    expect(containedTableScrollForColumns(normalized)).toEqual({ x: 460 });
  });

  it('keeps adaptive columns fluid when adaptive mode is explicitly configured', () => {
    const normalized = normalizeTableColumns<Row>([
      { dataIndex: 'name', title: '名称' },
      { dataIndex: 'id', title: 'ID', width: 96 }
    ], 'adaptive');

    expect(normalized[0]).not.toHaveProperty('width');
    expect(normalized[1]).toMatchObject({ dataIndex: 'id', title: 'ID', width: 96 });
  });

  it('normalizes compact nested leaves without assigning a synthetic width to their group', () => {
    const normalized = normalizeTableColumns<Row>([
      {
        key: 'identity',
        title: '身份',
        children: [
          { dataIndex: 'name', title: '名称' },
          { dataIndex: 'id', title: 'ID', width: 720 }
        ]
      }
    ]);

    expect(normalized[0]).not.toHaveProperty('width');
    expect(normalized[0].children?.[0]).toMatchObject({ dataIndex: 'name', width: 160 });
    expect(normalized[0].children?.[1]).toMatchObject({ dataIndex: 'id', width: 720 });
  });
});
