import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { ConfirmAction } from './ConfirmAction';

describe('ConfirmAction', () => {
  it('submits a trimmed reason with primary semantics and clears cancelled drafts', async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn().mockResolvedValue(undefined);

    render(<ConfirmAction actionText="保存配置" title="确认保存配置" onConfirm={onConfirm} />);

    const trigger = screen.getByRole('button', { name: '保存配置' });
    expect(trigger).toHaveClass('semi-button-primary', 'semi-button-solid');
    await user.click(trigger);

    const confirm = screen.getByRole('button', { name: '确认' });
    expect(confirm).toBeDisabled();
    await user.type(screen.getByLabelText('操作原因'), '  配置复核通过  ');
    expect(confirm).toHaveClass('semi-button-primary');
    await user.click(confirm);

    await waitFor(() => expect(onConfirm).toHaveBeenCalledWith('配置复核通过'));
    await waitFor(() => expect(screen.queryByLabelText('操作原因')).not.toBeInTheDocument());

    await user.click(trigger);
    await user.type(screen.getByLabelText('操作原因'), '不应保留');
    await user.click(screen.getByRole('button', { name: '取消' }));
    await user.click(trigger);
    expect(screen.getByLabelText('操作原因')).toHaveValue('');
  });

  it('uses danger semantics for irreversible actions in both trigger and confirmation', async () => {
    const user = userEvent.setup();

    render(<ConfirmAction actionText="冲正" title="确认冲正充值" onConfirm={vi.fn()} />);

    const trigger = screen.getByRole('button', { name: '冲正' });
    expect(trigger).toHaveClass('semi-button-danger', 'semi-button-light');
    await user.click(trigger);
    await user.type(screen.getByLabelText('操作原因'), '链上重组');
    expect(screen.getByRole('button', { name: '确认' })).toHaveClass('semi-button-danger');
  });
});
