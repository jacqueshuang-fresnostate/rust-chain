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

describe('DataTable', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('uses local controlled pagination with page size 20', async () => {
    render(<DataTable<Row> columns={columns} data={rows} />);

    expect(screen.getByText('记录 1')).toBeInTheDocument();
    expect(screen.getByText('记录 20')).toBeInTheDocument();
    expect(screen.queryByText('记录 21')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(screen.queryByText('记录 1')).not.toBeInTheDocument();
    expect(screen.getByText('记录 21')).toBeInTheDocument();
    expect(screen.getByText('记录 25')).toBeInTheDocument();
  });

  it('renders bordered and resizable tables with Semi default table styling', () => {
    render(<DataTable<Row> columns={columns} data={rows} />);

    const table = screen.getByRole('grid');
    expect(table.closest('.semi-table-bordered')).toBeInTheDocument();
    expect(table.closest('.semi-table-wrapper')).not.toHaveClass('admin-data-table');
    expect(table.closest('.semi-table-bordered')).not.toHaveClass('semi-table-small');
    expect(table.querySelector('.react-resizable-handle')).toBeInTheDocument();
    expect(document.querySelector('.admin-data-table')).not.toBeInTheDocument();
  });

  it('normalizes missing column widths without changing existing column props', () => {
    const fixedRender = () => '操作';
    const normalized = normalizeTableColumns<Row>([
      { dataIndex: 'name', title: '名称' },
      { dataIndex: 'id', fixed: 'right', render: fixedRender, title: '操作', width: 300 }
    ]);

    expect(normalized[0]).toMatchObject({ dataIndex: 'name', title: '名称', width: 160 });
    expect(normalized[1]).toMatchObject({ dataIndex: 'id', fixed: 'right', title: '操作', width: 300 });
    expect(normalized[1].render).toBe(fixedRender);
  });
});
