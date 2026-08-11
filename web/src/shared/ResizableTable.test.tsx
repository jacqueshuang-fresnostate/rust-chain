import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import {
  ResizableTable,
  RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH,
  RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH,
  RESIZABLE_TABLE_MAX_COLUMN_WIDTH,
  RESIZABLE_TABLE_MIN_COLUMN_WIDTH
} from './ResizableTable';

type Row = {
  email: string;
  id: number;
  name: string;
};

const data: Row[] = [{ email: 'admin@example.test', id: 1, name: '管理员' }];

function columnWidth(handleName: string) {
  return Number(screen.getByRole('separator', { name: handleName }).getAttribute('aria-valuenow'));
}

function renderedTableWidth() {
  const table = screen.queryByRole('grid') ?? screen.queryByRole('treegrid');
  return table?.style.width;
}

describe('ResizableTable', () => {
  it('adds one project handle to every declared leaf, including nested and fixed action columns', () => {
    render(
      <ResizableTable<Row>
        aria-label="用户表"
        className="business-fixture"
        columns={[
          {
            key: 'identity',
            title: '身份',
            children: [
              { dataIndex: 'name', title: '名称', width: 120 },
              { dataIndex: 'email', title: '邮箱' }
            ]
          },
          { dataIndex: 'id', fixed: 'right', key: 'actions', title: '操作', width: 216 }
        ]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );

    expect(screen.getByRole('grid', { name: '用户表' }).closest('.semi-table-wrapper')).toHaveClass('admin-resizable-table', 'business-fixture');
    expect(screen.getAllByRole('separator')).toHaveLength(3);
    expect(columnWidth('调整名称列宽')).toBe(120);
    expect(columnWidth('调整邮箱列宽')).toBe(RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH);
    expect(columnWidth('调整操作列宽')).toBe(216);
    expect(screen.queryByRole('separator', { name: '调整身份列宽' })).not.toBeInTheDocument();
    expect(document.querySelector('.react-resizable-handle')).not.toBeInTheDocument();
    expect(screen.getAllByRole('columnheader', { name: '操作' })).toHaveLength(1);
    expect(screen.getByRole('columnheader', { name: '操作' })).toHaveClass('admin-table-action-column', 'semi-table-cell-fixed-right');
    expect(document.querySelector('.semi-table-row-cell.admin-table-action-column')).toBeInTheDocument();
    expect(screen.getByRole('separator', { name: '调整操作列宽' })).toHaveAttribute('aria-valuemin', String(RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH));
  });

  it('recognizes only key actions as an action column and enforces its pointer and keyboard minimum', async () => {
    render(
      <ResizableTable<Row>
        columns={[
          { dataIndex: 'name', key: 'action', title: '操作', width: 96 },
          {
            className: 'existing-action-class',
            dataIndex: 'id',
            key: 'actions',
            render: () => <button type="button">管理</button>,
            title: '管理动作',
            width: 96
          }
        ]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );

    const businessHandle = screen.getByRole('separator', { name: '调整操作列宽' });
    const actionHandle = screen.getByRole('separator', { name: '调整管理动作列宽' });
    expect(businessHandle).toHaveAttribute('aria-valuemin', String(RESIZABLE_TABLE_MIN_COLUMN_WIDTH));
    expect(actionHandle).toHaveAttribute('aria-valuemin', String(RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH));
    expect(columnWidth('调整操作列宽')).toBe(96);
    expect(columnWidth('调整管理动作列宽')).toBe(RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH);
    expect(screen.getByRole('columnheader', { name: '操作' })).not.toHaveClass('admin-table-action-column');
    expect(screen.getByRole('columnheader', { name: '管理动作' })).toHaveClass('admin-table-action-column', 'existing-action-class');
    expect(screen.getByText('管理').closest('td')).toHaveClass('admin-table-action-column', 'existing-action-class');

    fireEvent.keyDown(actionHandle, { key: 'ArrowLeft' });
    expect(columnWidth('调整管理动作列宽')).toBe(RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH);
    fireEvent.keyDown(businessHandle, { key: 'Home' });
    expect(columnWidth('调整操作列宽')).toBe(RESIZABLE_TABLE_MIN_COLUMN_WIDTH);

    fireEvent.pointerDown(actionHandle, { button: 0, clientX: 120, pointerId: 21, pointerType: 'mouse' });
    fireEvent.pointerMove(document, { clientX: 0, pointerId: 21, pointerType: 'mouse' });
    await waitFor(() => {
      expect(columnWidth('调整管理动作列宽')).toBe(RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH);
    });
    fireEvent.pointerUp(document, { clientX: 0, pointerId: 21, pointerType: 'mouse' });
  });

  it('updates a controlled width and numeric scroll width during pointer dragging, then cleans document state', async () => {
    const onChange = vi.fn();
    const { unmount } = render(
      <ResizableTable<Row>
        columns={[
          { dataIndex: 'name', sorter: true, title: '名称', width: 120 },
          { dataIndex: 'email', title: '邮箱', width: 160 }
        ]}
        dataSource={data}
        onChange={onChange}
        pagination={false}
        rowKey="id"
      />
    );
    const handle = screen.getByRole('separator', { name: '调整名称列宽' });
    expect(renderedTableWidth()).toBe('280px');

    fireEvent.pointerDown(handle, { button: 2, clientX: 100, pointerId: 5, pointerType: 'pen' });
    fireEvent.pointerMove(document, { clientX: 180, pointerId: 5, pointerType: 'pen' });
    expect(columnWidth('调整名称列宽')).toBe(120);
    expect(document.body).not.toHaveClass('admin-table-column-resizing');

    fireEvent.click(handle);
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.pointerDown(handle, { button: 0, clientX: 100, pointerId: 7, pointerType: 'mouse' });
    expect(document.body).toHaveClass('admin-table-column-resizing');
    fireEvent.pointerDown(handle, { button: 0, clientX: 100, pointerId: 8, pointerType: 'touch' });
    fireEvent.pointerMove(document, { clientX: 180, pointerId: 8, pointerType: 'touch' });
    fireEvent.pointerUp(document, { clientX: 180, pointerId: 8, pointerType: 'touch' });
    expect(columnWidth('调整名称列宽')).toBe(120);
    expect(document.body).toHaveClass('admin-table-column-resizing');
    fireEvent.pointerMove(document, { clientX: 148, pointerId: 7, pointerType: 'mouse' });

    await waitFor(() => {
      expect(columnWidth('调整名称列宽')).toBe(168);
      expect(renderedTableWidth()).toBe('328px');
    });
    expect(screen.getByRole('separator', { name: '调整名称列宽' })).toHaveClass('is-dragging');
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.pointerUp(document, {
      clientX: 148,
      pointerId: 7,
      pointerType: 'mouse'
    });
    await waitFor(() => {
      expect(document.body).not.toHaveClass('admin-table-column-resizing');
      expect(screen.getByRole('separator', { name: '调整名称列宽' })).not.toHaveClass('is-dragging');
    });

    fireEvent.pointerDown(screen.getByRole('separator', { name: '调整名称列宽' }), {
      button: 0,
      clientX: 148,
      pointerId: 8,
      pointerType: 'mouse'
    });
    expect(document.body).toHaveClass('admin-table-column-resizing');
    fireEvent.pointerCancel(document, { clientX: 148, pointerId: 8, pointerType: 'mouse' });
    expect(document.body).not.toHaveClass('admin-table-column-resizing');

    fireEvent.pointerDown(screen.getByRole('separator', { name: '调整名称列宽' }), {
      button: 0,
      clientX: 148,
      pointerId: 9,
      pointerType: 'mouse'
    });
    expect(document.body).toHaveClass('admin-table-column-resizing');
    unmount();
    expect(document.body).not.toHaveClass('admin-table-column-resizing');
  });

  it('preserves unchanged dynamic columns, resets changed declarations, and cancels a removed active column', async () => {
    const firstColumns = [
      { dataIndex: 'name' as const, key: 'name', title: '名称', width: 120 },
      { dataIndex: 'email' as const, key: 'email', title: '邮箱', width: 180 }
    ];
    const { rerender } = render(
      <ResizableTable<Row> columns={firstColumns} dataSource={data} pagination={false} rowKey="id" />
    );

    fireEvent.keyDown(screen.getByRole('separator', { name: '调整名称列宽' }), { key: 'ArrowRight' });
    expect(columnWidth('调整名称列宽')).toBe(136);

    rerender(
      <ResizableTable<Row>
        columns={[firstColumns[1], firstColumns[0], { dataIndex: 'id', key: 'id', title: 'ID' }]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );
    expect(columnWidth('调整名称列宽')).toBe(136);
    expect(columnWidth('调整ID列宽')).toBe(RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH);

    rerender(
      <ResizableTable<Row>
        columns={[{ ...firstColumns[0], width: 240 }, firstColumns[1]]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );
    await waitFor(() => expect(columnWidth('调整名称列宽')).toBe(240));

    fireEvent.pointerDown(screen.getByRole('separator', { name: '调整名称列宽' }), {
      button: 0,
      clientX: 100,
      pointerId: 12,
      pointerType: 'mouse'
    });
    expect(document.body).toHaveClass('admin-table-column-resizing');
    rerender(
      <ResizableTable<Row> columns={[firstColumns[1]]} dataSource={data} pagination={false} rowKey="id" />
    );
    await waitFor(() => expect(document.body).not.toHaveClass('admin-table-column-resizing'));
    fireEvent.pointerMove(document, { clientX: 300, pointerId: 12, pointerType: 'mouse' });
    expect(columnWidth('调整邮箱列宽')).toBe(180);

    rerender(
      <ResizableTable<Row> columns={firstColumns} dataSource={data} pagination={false} rowKey="id" />
    );
    expect(columnWidth('调整名称列宽')).toBe(120);
  });

  it('keeps duplicate key and dataIndex leaves isolated with collision-safe path identities', () => {
    const { rerender } = render(
      <ResizableTable<Row>
        columns={[
          { dataIndex: 'name', key: 'duplicate', title: '首个重复 Key', width: 100 },
          { dataIndex: 'email', key: 'duplicate', title: '第二个重复 Key', width: 180 },
          { dataIndex: 'name', title: '首个重复 dataIndex', width: 140 },
          { dataIndex: 'name', title: '第二个重复 dataIndex', width: 200 }
        ]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );

    const handles = screen.getAllByRole('separator');
    expect(new Set(handles.map((handle) => handle.getAttribute('data-column-key'))).size).toBe(4);
    fireEvent.keyDown(screen.getByRole('separator', { name: '调整首个重复 Key列宽' }), { key: 'ArrowRight' });
    expect(columnWidth('调整首个重复 Key列宽')).toBe(116);
    expect(columnWidth('调整第二个重复 Key列宽')).toBe(180);

    rerender(
      <ResizableTable<Row>
        columns={[{ dataIndex: 'email', key: 'duplicate', title: '第二个重复 Key', width: 180 }]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );
    expect(columnWidth('调整第二个重复 Key列宽')).toBe(180);
  });

  it('supports separator keyboard controls and clamps both boundaries', () => {
    render(
      <ResizableTable<Row>
        columns={[{ dataIndex: 'name', title: '名称', width: 96 }]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );
    const handle = screen.getByRole('separator', { name: '调整名称列宽' });
    expect(handle).toHaveAttribute('aria-orientation', 'vertical');
    expect(handle).toHaveAttribute('aria-valuemin', String(RESIZABLE_TABLE_MIN_COLUMN_WIDTH));
    expect(handle).toHaveAttribute('aria-valuemax', String(RESIZABLE_TABLE_MAX_COLUMN_WIDTH));

    fireEvent.keyDown(handle, { key: 'ArrowLeft' });
    expect(columnWidth('调整名称列宽')).toBe(80);
    fireEvent.keyDown(handle, { key: 'ArrowLeft' });
    expect(columnWidth('调整名称列宽')).toBe(80);
    fireEvent.keyDown(handle, { key: 'End' });
    expect(columnWidth('调整名称列宽')).toBe(RESIZABLE_TABLE_MAX_COLUMN_WIDTH);
    fireEvent.keyDown(handle, { key: 'ArrowRight' });
    expect(columnWidth('调整名称列宽')).toBe(RESIZABLE_TABLE_MAX_COLUMN_WIDTH);
    fireEvent.keyDown(handle, { key: 'Home' });
    expect(columnWidth('调整名称列宽')).toBe(80);
  });

  it('retains a legal wide declaration while normalizing invalid and unsafe declarations', () => {
    render(
      <ResizableTable<Row>
        columns={[
          { dataIndex: 'name', key: 'wide', title: '宽列', width: 720 },
          { dataIndex: 'email', key: 'invalid', title: '非法列', width: Number.NaN },
          { dataIndex: 'id', key: 'unsafe', title: '超限列', width: 5000 }
        ]}
        dataSource={data}
        pagination={false}
        rowKey="id"
      />
    );

    expect(columnWidth('调整宽列列宽')).toBe(720);
    expect(columnWidth('调整非法列列宽')).toBe(RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH);
    expect(columnWidth('调整超限列列宽')).toBe(RESIZABLE_TABLE_MAX_COLUMN_WIDTH);
  });

  it('includes the visible row-selection width and preserves custom body, scroll, pagination, class and aria props', () => {
    function CustomTable(props: React.ComponentPropsWithoutRef<'table'>) {
      return <table {...props} data-testid="custom-table" />;
    }

    render(
      <ResizableTable<Row>
        aria-label="可选用户表"
        className="forwarded-class"
        columns={[{ dataIndex: 'name', title: '名称' }]}
        components={{ body: { outer: CustomTable } }}
        dataSource={data}
        pagination={{ pageSize: 10 }}
        rowKey="id"
        rowSelection={{ width: 52 }}
        scroll={{ scrollToFirstRowOnChange: false, y: 240 }}
      />
    );

    const grid = screen.getByRole('grid', { name: '可选用户表' });
    expect(grid).toHaveAttribute('data-testid', 'custom-table');
    expect(grid.closest('.semi-table-wrapper')).toHaveClass('admin-resizable-table', 'forwarded-class');
    expect(renderedTableWidth()).toBe('212px');
    expect(document.querySelector('.semi-table-body')).toHaveStyle({ maxHeight: '240px' });
    expect(document.querySelector('.semi-page')).toBeInTheDocument();
    expect(screen.getAllByRole('columnheader')).toHaveLength(2);
    expect(screen.getAllByRole('separator')).toHaveLength(1);
    expect(within(grid).getAllByRole('gridcell')).not.toHaveLength(0);
  });

  it('accounts for selection and the opt-in dedicated expand column using Semi default visibility semantics', () => {
    const expandedRowRender = () => <span>展开内容</span>;
    const { rerender } = render(
      <ResizableTable<Row>
        aria-label="可展开用户表"
        columns={[{ dataIndex: 'name', title: '名称' }]}
        dataSource={data}
        expandedRowRender={expandedRowRender}
        hideExpandedColumn={false}
        pagination={false}
        rowKey="id"
        rowSelection
      />
    );

    expect(screen.getByRole('treegrid', { name: '可展开用户表' })).toHaveStyle({ width: '256px' });
    expect(screen.getAllByRole('columnheader')).toHaveLength(3);
    expect(screen.getAllByRole('separator')).toHaveLength(1);

    rerender(
      <ResizableTable<Row>
        aria-label="可展开用户表"
        columns={[{ dataIndex: 'name', title: '名称' }]}
        dataSource={data}
        expandedRowRender={expandedRowRender}
        pagination={false}
        rowKey="id"
        rowSelection={{ width: '52px' }}
      />
    );

    expect(screen.getByRole('treegrid', { name: '可展开用户表' })).toHaveStyle({ width: '212px' });
    expect(screen.getAllByRole('columnheader')).toHaveLength(2);
  });
});
