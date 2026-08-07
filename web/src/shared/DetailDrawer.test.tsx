import { render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { DetailDrawer, DetailFieldTable } from './DetailDrawer';

describe('DetailDrawer tables', () => {
  it('makes both field-table leaves resizable', () => {
    render(<DetailFieldTable record={{ id: 7, status: 'active' }} />);

    expect(screen.getAllByRole('separator')).toHaveLength(2);
    expect(screen.getByRole('separator', { name: '调整字段列宽' })).toHaveAttribute('aria-valuenow', '220');
    expect(screen.getByRole('separator', { name: '调整内容列宽' })).toHaveAttribute('aria-valuenow', '640');
    expect(document.querySelector('.react-resizable-handle')).not.toBeInTheDocument();
  });

  it('adds handles to every dynamic record column in array details', () => {
    render(
      <DetailDrawer
        detail={{
          data: [{ id: 7, name: '测试记录', status: 'active' }],
          title: '动态详情'
        }}
        onClose={vi.fn()}
      />
    );

    const sheet = screen.getByText('动态详情').closest('.semi-sidesheet-inner');
    expect(sheet).toBeInTheDocument();
    expect(within(sheet as HTMLElement).getAllByRole('separator')).toHaveLength(3);
    expect(within(sheet as HTMLElement).getByRole('separator', { name: '调整ID列宽' })).toBeInTheDocument();
    expect(within(sheet as HTMLElement).getByRole('separator', { name: '调整名称列宽' })).toBeInTheDocument();
    expect(within(sheet as HTMLElement).getByRole('separator', { name: '调整状态列宽' })).toBeInTheDocument();
  });
});
