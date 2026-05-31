import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { DataTable } from './DataTable';

type Row = {
  id: number;
  name: string;
};

const rows: Row[] = Array.from({ length: 25 }, (_, index) => ({
  id: index + 1,
  name: `记录 ${index + 1}`
}));

const columns = [{ dataIndex: 'name', title: '名称' }];

describe('DataTable', () => {
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
});
