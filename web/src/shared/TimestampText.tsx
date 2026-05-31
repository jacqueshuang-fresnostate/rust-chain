type TimestampTextProps = {
  value?: number | null;
};

const formatter = new Intl.DateTimeFormat('zh-CN', {
  year: 'numeric',
  month: 'long',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: false
});

export function TimestampText({ value }: TimestampTextProps) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return <span>-</span>;
  }

  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return <span>-</span>;
  }

  return <time dateTime={date.toISOString()}>{formatter.format(date)}</time>;
}
