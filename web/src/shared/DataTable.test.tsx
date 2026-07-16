import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { DataTable, normalizeTableColumns } from './DataTable';

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

  it('renders bordered and resizable tables with compact table styling by default', () => {
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
    expect(table.querySelector('.react-resizable-handle')).toBeInTheDocument();
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
  });

  it('keeps adaptive columns fluid when adaptive mode is explicitly configured', () => {
    const normalized = normalizeTableColumns<Row>([
      { dataIndex: 'name', title: '名称' },
      { dataIndex: 'id', title: 'ID', width: 96 }
    ], 'adaptive');

    expect(normalized[0]).not.toHaveProperty('width');
    expect(normalized[1]).toMatchObject({ dataIndex: 'id', title: 'ID', width: 96 });
  });
});
