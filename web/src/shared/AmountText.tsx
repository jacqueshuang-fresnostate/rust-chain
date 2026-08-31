import { formatAdminNumber } from './numberFormat';

type AmountTextProps = {
  value?: string | number | null;
  asset?: string;
  precision?: number;
};

export function AmountText({ value, asset, precision }: AmountTextProps) {
  const formatted = formatAdminNumber(value, precision === undefined ? {} : { precision });
  if (!formatted) {
    return <span>-</span>;
  }

  return <span>{asset ? `${formatted} ${asset}` : formatted}</span>;
}
