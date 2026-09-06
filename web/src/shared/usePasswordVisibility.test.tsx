import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AdminPasswordInput } from './SemiFormControls';

describe('Chinese password visibility control', () => {
  it('toggles visibility with Chinese accessible labels without submitting or changing the secret', () => {
    const submit = vi.fn();
    const change = vi.fn();
    render(<form onSubmit={submit}><AdminPasswordInput ariaLabel="测试密码" value="fixture-password" onChange={change} /></form>);
    const input = screen.getByLabelText('测试密码');
    expect(input).toHaveAttribute('type', 'password');
    fireEvent.click(screen.getByRole('button', { name: '显示密码' }));
    expect(input).toHaveAttribute('type', 'text');
    expect(input).toHaveValue('fixture-password');
    fireEvent.click(screen.getByRole('button', { name: '隐藏密码' }));
    expect(input).toHaveAttribute('type', 'password');
    expect(submit).not.toHaveBeenCalled();
    expect(change).not.toHaveBeenCalled();
  });

  it('does not expose a visibility action for a disabled password input', () => {
    render(<AdminPasswordInput ariaLabel="只读密码" value="" onChange={() => undefined} disabled />);
    expect(screen.getByLabelText('只读密码')).toBeDisabled();
    expect(screen.queryByRole('button', { name: '显示密码' })).not.toBeInTheDocument();
  });
});
