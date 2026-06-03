import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AmountText } from './AmountText';
import { formatAdminNumber } from './numberFormat';
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
  it('renders decimal strings with the Admin numeral format and optional asset suffix', () => {
    const { rerender } = render(<AmountText value="1234.5" />);

    expect(screen.getByText('1,234.50')).toBeInTheDocument();

    rerender(<AmountText value="1234.567891" asset="USDT" />);

    expect(screen.getByText('1,234.567891 USDT')).toBeInTheDocument();
  });

  it('renders a dash for missing or empty values', () => {
    const { rerender } = render(<AmountText value={null} />);

    expect(screen.getByText('-')).toBeInTheDocument();

    rerender(<AmountText value="" asset="BTC" />);

    expect(screen.getByText('-')).toBeInTheDocument();
  });
});

describe('formatAdminNumber', () => {
  it('uses the Admin numeral format for integer and decimal display values', () => {
    expect(formatAdminNumber('70000')).toBe('70,000.00');
    expect(formatAdminNumber('70000.123456')).toBe('70,000.123456');
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
