import { formatAdminAmount } from './numberFormat';

type AmountTextProps = {
  value?: string | number | null;
  asset?: string;
  appendAsset?: boolean;
  precision?: number;
};

export function AmountText({ value, asset, appendAsset = true, precision }: AmountTextProps) {
  const formatted = formatAdminAmount(value, { asset, precision });
  if (!formatted) {
    return <span>-</span>;
  }

  return <span>{appendAsset && asset ? `${formatted} ${asset}` : formatted}</span>;
}
