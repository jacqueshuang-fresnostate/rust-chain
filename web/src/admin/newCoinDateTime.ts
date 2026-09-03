const localDateTimePattern = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?$/;

function parseNewCoinLocalDateTime(value: string): number | undefined {
  const trimmed = value.trim();
  const match = localDateTimePattern.exec(trimmed);
  if (!match) {
    return undefined;
  }

  const [, yearText, monthText, dayText, hourText, minuteText, secondText = '0', millisecondText = '0'] = match;
  const expected = {
    year: Number(yearText),
    month: Number(monthText) - 1,
    day: Number(dayText),
    hour: Number(hourText),
    minute: Number(minuteText),
    second: Number(secondText),
    millisecond: Number(millisecondText.padEnd(3, '0'))
  };
  const date = new Date(0);
  date.setFullYear(expected.year, expected.month, expected.day);
  date.setHours(expected.hour, expected.minute, expected.second, expected.millisecond);
  const milliseconds = date.getTime();
  if (
    !Number.isFinite(milliseconds) ||
    milliseconds <= 0 ||
    date.getFullYear() !== expected.year ||
    date.getMonth() !== expected.month ||
    date.getDate() !== expected.day ||
    date.getHours() !== expected.hour ||
    date.getMinutes() !== expected.minute ||
    date.getSeconds() !== expected.second ||
    date.getMilliseconds() !== expected.millisecond
  ) {
    return undefined;
  }

  return milliseconds;
}

export function isValidNewCoinLocalDateTime(value: string): boolean {
  return parseNewCoinLocalDateTime(value) !== undefined;
}

export function requiredNewCoinLocalDateTimeMillis(value: string, label: string): number {
  if (!value.trim()) {
    throw new Error(`${label}不能为空`);
  }
  const milliseconds = parseNewCoinLocalDateTime(value);
  if (milliseconds === undefined) {
    throw new Error(`${label}必须为有效日期时间`);
  }
  return milliseconds;
}

export function optionalNewCoinLocalDateTimeMillis(value: string, label: string): number | undefined {
  return value.trim() ? requiredNewCoinLocalDateTimeMillis(value, label) : undefined;
}
