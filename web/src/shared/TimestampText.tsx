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

export function formatAdminTimestamp(value?: number | null): string | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return null;
  }

  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return null;
  }

  return formatter.format(date);
}

export function TimestampText({ value }: TimestampTextProps) {
  const formatted = formatAdminTimestamp(value);
  if (!formatted) {
    return <span>-</span>;
  }

  return <time dateTime={new Date(value as number).toISOString()}>{formatted}</time>;
}
