import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { FilterBar, type FilterField } from './FilterBar';

const fields: FilterField[] = [{ key: 'keyword', label: '关键词', placeholder: '输入关键词' }];

describe('FilterBar', () => {
  it('keeps edits as a draft until submit, follows external values, and resets immediately', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { rerender } = render(<FilterBar fields={fields} onChange={onChange} value={{ keyword: '初始值' }} />);
    const input = screen.getByRole('textbox', { name: '关键词' });

    await user.clear(input);
    await user.type(input, '待查询');
    expect(onChange).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: '查询' }));
    expect(onChange).toHaveBeenLastCalledWith({ keyword: '待查询' });

    rerender(<FilterBar fields={fields} onChange={onChange} value={{ keyword: '服务端同步值' }} />);
    await waitFor(() => expect(input).toHaveValue('服务端同步值'));

    await user.click(screen.getByRole('button', { name: '重置' }));
    expect(onChange).toHaveBeenLastCalledWith({});
    expect(input).toHaveValue('');
  });

  it('prunes blank values and disables every action while loading', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { rerender } = render(<FilterBar fields={fields} onChange={onChange} value={{}} />);

    await user.type(screen.getByRole('textbox', { name: '关键词' }), '   ');
    await user.click(screen.getByRole('button', { name: '查询' }));
    expect(onChange).toHaveBeenLastCalledWith({});

    rerender(<FilterBar fields={fields} loading onChange={onChange} value={{ keyword: '锁定值' }} />);
    expect(screen.getByRole('textbox', { name: '关键词' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '查询' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '重置' })).toBeDisabled();
  });
});
