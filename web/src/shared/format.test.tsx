import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AmountText } from './AmountText';
import { StatusTag } from './StatusTag';
import { TimestampText } from './TimestampText';

describe('TimestampText', () => {
  it('renders unix milliseconds as Chinese local date and time', () => {
    render(<TimestampText value={1_735_732_800_000} />);

    expect(screen.getByText(/^2025年1月1日/)).toHaveTextContent(/20:00|12:00/);
  });

  it('renders a dash for missing values', () => {
    const { rerender } = render(<TimestampText value={null} />);

    expect(screen.getByText('-')).toBeInTheDocument();

    rerender(<TimestampText value={undefined} />);

    expect(screen.getByText('-')).toBeInTheDocument();
  });
});

describe('AmountText', () => {
  it('renders decimal strings exactly with an optional asset suffix', () => {
    render(<AmountText value="1000000000000000000.123450000000000001" asset="USDT" />);

    expect(screen.getByText('1000000000000000000.123450000000000001 USDT')).toBeInTheDocument();
  });

  it('renders a dash for missing or empty values', () => {
    const { rerender } = render(<AmountText value={null} />);

    expect(screen.getByText('-')).toBeInTheDocument();

    rerender(<AmountText value="" asset="BTC" />);

    expect(screen.getByText('-')).toBeInTheDocument();
  });
});

describe('StatusTag', () => {
  it('maps known statuses to Chinese labels', () => {
    render(<StatusTag value="enabled" />);

    expect(screen.getByText('启用')).toBeInTheDocument();
  });

  it('maps booleans to enabled and disabled semantics', () => {
    const { rerender } = render(<StatusTag value={true} />);

    expect(screen.getByText('启用')).toBeInTheDocument();

    rerender(<StatusTag value={false} />);

    expect(screen.getByText('禁用')).toBeInTheDocument();
  });

  it('displays unknown statuses robustly', () => {
    render(<StatusTag value="custom_status" />);

    expect(screen.getByText('custom_status')).toBeInTheDocument();
  });
});
