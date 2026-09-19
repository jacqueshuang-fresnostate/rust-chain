/** 先校验整数词法，再转换；禁止小数、指数和十六进制被 Number 悄悄取整。 */
export function parseSafeInteger(value: unknown, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null;
  if (typeof value === 'string' && !/^[+-]?\d+$/.test(value.trim())) return null;
  const parsed = typeof value === 'number' ? value : Number(value.trim());
  return Number.isSafeInteger(parsed) && parsed >= minimum && parsed <= maximum ? parsed : null;
}

export function requiredSafeInteger(value: unknown, label: string, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number {
  const parsed = parseSafeInteger(value, minimum, maximum);
  if (parsed === null) throw new Error(`${label}必须为 ${minimum} 至 ${maximum} 的安全整数`);
  return parsed;
}

export function isUnixMillis(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && Math.abs(value) <= 8_640_000_000_000_000;
}

/** 只校验 JSON 数值，不把金额字符串、外部标识或图表小数改成整数。 */
export function assertSafeJsonNumbers(value: unknown, fail: () => never): void {
  if (typeof value === 'number') {
    if (!Number.isFinite(value) || (Number.isInteger(value) && !Number.isSafeInteger(value))) fail();
  } else if (Array.isArray(value)) {
    value.forEach((entry) => assertSafeJsonNumbers(entry, fail));
  } else if (value !== null && typeof value === 'object') {
    Object.values(value).forEach((entry) => assertSafeJsonNumbers(entry, fail));
  }
}
