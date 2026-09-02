import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AmountText } from './AmountText';
import { formatAdminAmount, formatAdminNumber } from './numberFormat';
import { compareDecimalText } from './decimal';
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

    expect(screen.getByText('1,234.57 USDT')).toBeInTheDocument();
  });

  it('按独立显示精度处理资产且不经过 Number', () => {
    const { rerender } = render(<AmountText precision={0} value="9007199254740993" />);
    expect(screen.getByText('9,007,199,254,740,993')).toBeInTheDocument();

    rerender(<AmountText asset="BTC" precision={8} value="0.00000001" />);
    expect(screen.getByText('0.00000001 BTC')).toBeInTheDocument();

    rerender(<AmountText asset="BTC" precision={18} value="1e-18" />);
    expect(screen.getByText('<0.00000001 BTC')).toBeInTheDocument();
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

  it('caps generic display at six digits while preserving source precision for comparisons', () => {
    expect(formatAdminNumber('0.000000000000000001')).toBe('<0.000001');
    expect(formatAdminNumber('123456789012345678.123456789012345678')).toBe('123,456,789,012,345,678.123457');
    expect(formatAdminNumber('1e-18')).toBe('<0.000001');
    expect(formatAdminNumber('-1134.335')).toBe('-1,134.335');
    expect(compareDecimalText('9007199254740993.000000000000000001', '9007199254740993')).toBe(1);
  });

  it('uses stablecoin and crypto display caps without changing the raw decimal', () => {
    const raw = '1134.331253942506787192';
    expect(formatAdminAmount(raw, { asset: 'USDT' })).toBe('1,134.33');
    expect(formatAdminAmount('-1134.335', { asset: 'USDT' })).toBe('-1,134.34');
    expect(formatAdminAmount('999.999', { asset: 'USDT' })).toBe('1,000.00');
    expect(formatAdminAmount('1.234567895', { asset: 'BTC' })).toBe('1.2345679');
    expect(formatAdminAmount('-0.0001', { asset: 'USDT' })).toBe('>-0.01');
    expect(formatAdminAmount('-0.0000', { asset: 'USDT' })).toBe('0.00');
    expect(formatAdminAmount('not-a-decimal', { asset: 'USDT' })).toBeNull();
    expect(raw).toBe('1134.331253942506787192');
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
