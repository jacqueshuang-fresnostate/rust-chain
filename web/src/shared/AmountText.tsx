import { formatAdminNumber } from './numberFormat';

type AmountTextProps = {
  value?: string | number | null;
  asset?: string;
};

export function AmountText({ value, asset }: AmountTextProps) {
  const formatted = formatAdminNumber(value);
  if (!formatted) {
    return <span>-</span>;
  }

  return <span>{asset ? `${formatted} ${asset}` : formatted}</span>;
}
