import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';

import { MarketStrategyNodeEditor, type MarketStrategyNodeDraft } from './MarketStrategyNodeEditor';

function Harness() {
  const [nodes, setNodes] = useState<MarketStrategyNodeDraft[]>([]);
  return <MarketStrategyNodeEditor value={nodes} onChange={setNodes} />;
}

describe('MarketStrategyNodeEditor', () => {
  it('adds, edits, and removes a Chinese strategy node row', async () => {
    const user = userEvent.setup();
    render(<Harness />);

    expect(screen.getByText('暂无目标节点，将继续使用兼容终点。')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '新增策略节点' }));
    expect(screen.getByText('节点1')).toBeInTheDocument();
    expect(screen.getByLabelText('节点1目标时间')).toHaveAttribute('type', 'datetime-local');
    expect(screen.getByLabelText('节点1目标值')).toHaveValue('');

    fireEvent.change(screen.getByLabelText('节点1目标时间'), { target: { value: '2026-04-01T10:00' } });
    await user.type(screen.getByLabelText('节点1目标值'), '2.5');
    expect(screen.getByLabelText('节点1目标时间')).toHaveValue('2026-04-01T10:00');
    expect(screen.getByLabelText('节点1目标值')).toHaveValue('2.5');

    await user.click(screen.getByRole('button', { name: '删除节点1' }));
    expect(screen.queryByText('节点1')).not.toBeInTheDocument();
    expect(screen.getByText('暂无目标节点，将继续使用兼容终点。')).toBeInTheDocument();
  });
});
