export type DecimalFormatOptions = {
  /**
   * 资产小数精度。显示时补足到该精度，但不会静默截断服务端返回的更高精度。
   */
  precision?: number;
  /** 未提供资产精度时的最少小数位。 */
  minimumFractionDigits?: number;
};

type ParsedDecimal = {
  canonical: string;
  fraction: string;
  integer: string;
  negative: boolean;
};

const DECIMAL_PATTERN = /^([+-]?)(?:(\d+)(?:\.(\d*))?|\.(\d+))(?:[eE]([+-]?\d+))?$/;
const MAX_EXPONENT = 10_000;
const MAX_FORMAT_DIGITS = 20_000;
const MAX_ASSET_PRECISION = 18;

function parseBoundedExponent(value: string | undefined): number | null {
  if (!value) return 0;
  const negative = value.startsWith('-');
  const digits = value.replace(/^[+-]/, '').replace(/^0+/, '') || '0';
  if (digits.length > String(MAX_EXPONENT).length) return null;
  const parsed = Number.parseInt(digits, 10);
  if (!Number.isSafeInteger(parsed) || parsed > MAX_EXPONENT) return null;
  return negative ? -parsed : parsed;
}

function parseDecimal(value: string | number): ParsedDecimal | null {
  const source = typeof value === 'number' ? String(value) : value.trim();
  if (!source || source.length > MAX_FORMAT_DIGITS) return null;

  const match = DECIMAL_PATTERN.exec(source);
  if (!match) return null;

  const exponent = parseBoundedExponent(match[5]);
  if (exponent === null) return null;

  const negative = match[1] === '-';
  const integerSource = match[2] ?? '0';
  const fractionSource = match[3] ?? match[4] ?? '';
  const coefficient = `${integerSource}${fractionSource}`;
  const firstSignificant = coefficient.search(/[1-9]/);
  if (firstSignificant === -1) {
    return { canonical: '0', fraction: '', integer: '0', negative: false };
  }

  const decimalPosition = integerSource.length + exponent;
  if (Math.abs(decimalPosition) + coefficient.length > MAX_FORMAT_DIGITS) return null;

  let integer: string;
  let fraction: string;
  if (decimalPosition <= 0) {
    integer = '0';
    fraction = `${'0'.repeat(-decimalPosition)}${coefficient}`;
  } else if (decimalPosition >= coefficient.length) {
    integer = `${coefficient}${'0'.repeat(decimalPosition - coefficient.length)}`;
    fraction = '';
  } else {
    integer = coefficient.slice(0, decimalPosition);
    fraction = coefficient.slice(decimalPosition);
  }

  integer = integer.replace(/^0+(?=\d)/, '');
  fraction = fraction.replace(/0+$/, '');
  const canonicalMagnitude = fraction ? `${integer}.${fraction}` : integer;
  return {
    canonical: negative ? `-${canonicalMagnitude}` : canonicalMagnitude,
    fraction,
    integer,
    negative
  };
}

/**
 * 把十进制文本规范化为唯一表示。金额的系数从不转为 JavaScript Number。
 */
export function canonicalDecimalText(value: string | number): string | null {
  return parseDecimal(value)?.canonical ?? null;
}

function compareMagnitude(left: ParsedDecimal, right: ParsedDecimal): -1 | 0 | 1 {
  if (left.integer.length !== right.integer.length) {
    return left.integer.length < right.integer.length ? -1 : 1;
  }
  if (left.integer !== right.integer) {
    return left.integer < right.integer ? -1 : 1;
  }

  const fractionLength = Math.max(left.fraction.length, right.fraction.length);
  for (let index = 0; index < fractionLength; index += 1) {
    const leftDigit = left.fraction[index] ?? '0';
    const rightDigit = right.fraction[index] ?? '0';
    if (leftDigit !== rightDigit) return leftDigit < rightDigit ? -1 : 1;
  }
  return 0;
}

export function compareDecimalText(leftValue: string | number, rightValue: string | number): -1 | 0 | 1 | null {
  const left = parseDecimal(leftValue);
  const right = parseDecimal(rightValue);
  if (!left || !right) return null;
  if (left.negative !== right.negative) return left.negative ? -1 : 1;
  const magnitude = compareMagnitude(left, right);
  return left.negative ? ((magnitude * -1) as -1 | 0 | 1) : magnitude;
}

export function isPositiveDecimalText(value: string | number): boolean {
  return compareDecimalText(value, '0') === 1;
}

export function isNonNegativeDecimalText(value: string | number): boolean {
  const comparison = compareDecimalText(value, '0');
  return comparison === 0 || comparison === 1;
}

type DecimalCoefficient = { coefficient: bigint; scale: number };

function toCoefficient(value: string | number): DecimalCoefficient | null {
  const parsed = parseDecimal(value);
  if (!parsed) return null;
  const digits = `${parsed.integer}${parsed.fraction}`;
  const coefficient = BigInt(digits) * (parsed.negative ? -1n : 1n);
  return { coefficient, scale: parsed.fraction.length };
}

function fromCoefficient(coefficient: bigint, scale: number): string | null {
  const negative = coefficient < 0n;
  const digits = (negative ? -coefficient : coefficient).toString().padStart(scale + 1, '0');
  const source = scale === 0 ? digits : `${digits.slice(0, -scale)}.${digits.slice(-scale)}`;
  return canonicalDecimalText(`${negative ? '-' : ''}${source}`);
}

/** 任意精度十进制加法，不经过浮点数。 */
export function addDecimalText(leftValue: string | number, rightValue: string | number): string | null {
  const left = toCoefficient(leftValue);
  const right = toCoefficient(rightValue);
  if (!left || !right) return null;
  const scale = Math.max(left.scale, right.scale);
  const leftCoefficient = left.coefficient * 10n ** BigInt(scale - left.scale);
  const rightCoefficient = right.coefficient * 10n ** BigInt(scale - right.scale);
  return fromCoefficient(leftCoefficient + rightCoefficient, scale);
}

/** 任意精度十进制乘法，不经过浮点数。 */
export function multiplyDecimalText(leftValue: string | number, rightValue: string | number): string | null {
  const left = toCoefficient(leftValue);
  const right = toCoefficient(rightValue);
  if (!left || !right) return null;
  return fromCoefficient(left.coefficient * right.coefficient, left.scale + right.scale);
}

/** 精确地乘以 (1 + percent / 100)。 */
export function applyPercentDecimalText(value: string | number, percent: string | number): string | null {
  const normalizedPercent = canonicalDecimalText(percent);
  if (normalizedPercent === null) return null;
  const factor = addDecimalText('100', normalizedPercent);
  if (factor === null) return null;
  const multiplied = multiplyDecimalText(value, factor);
  return multiplied === null ? null : canonicalDecimalText(`${multiplied}e-2`);
}

function validPrecision(value: number | undefined): number | null {
  if (value === undefined) return null;
  return Number.isInteger(value) && value >= 0 && value <= MAX_ASSET_PRECISION ? value : null;
}

export function formatDecimalText(value: string | number, options: DecimalFormatOptions = {}): string | null {
  const parsed = parseDecimal(value);
  if (!parsed) return null;

  const precision = validPrecision(options.precision);
  if (options.precision !== undefined && precision === null) return null;
  const requestedMinimum = validPrecision(options.minimumFractionDigits);
  if (options.minimumFractionDigits !== undefined && requestedMinimum === null) return null;
  const minimumFractionDigits = precision ?? requestedMinimum ?? 2;
  // 若后端返回超出已知精度的数值，保留原值以便暴露合约问题，不在展示层截断或舍入。
  const fractionLength = Math.max(parsed.fraction.length, minimumFractionDigits);
  const fraction = parsed.fraction.padEnd(fractionLength, '0');
  const groupedInteger = parsed.integer.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  const sign = parsed.negative ? '-' : '';
  return fraction ? `${sign}${groupedInteger}.${fraction}` : `${sign}${groupedInteger}`;
}
