import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { expect, it } from 'vitest';

import { MarketStrategyVolatilityField } from './MarketStrategyVolatilityField';

function Field() {
  const [value, setValue] = useState('0.01');
  return <MarketStrategyVolatilityField value={value} onChange={setValue} />;
}

it('labels decimal ratios and interprets them without converting or truncating input drafts', () => {
  render(<Field />);
  const input = screen.getByRole('textbox', { name: '波动率' });
  expect(screen.getByText(/波动率（小数比例）/)).toBeInTheDocument();
  expect(screen.getByText(/当前为 1%/)).toBeInTheDocument();
  for (const value of ['1.', '1', '0.010000000000000001', '']) {
    fireEvent.change(input, { target: { value } });
    expect(input).toHaveValue(value);
    if (value === '1') expect(screen.getByText(/当前为 100%/)).toBeInTheDocument();
    if (value.startsWith('0.01')) expect(screen.getByText(/当前为 1.0000000000000001%/)).toBeInTheDocument();
  }
  expect(screen.getByText(/请输入非负小数比例/)).toBeInTheDocument();
});
