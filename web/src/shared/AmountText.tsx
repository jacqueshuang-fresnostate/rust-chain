type AmountTextProps = {
  value?: string | null;
  asset?: string;
};

export function AmountText({ value, asset }: AmountTextProps) {
  if (value === null || value === undefined || value === '') {
    return <span>-</span>;
  }

  return <span>{asset ? `${value} ${asset}` : value}</span>;
}
