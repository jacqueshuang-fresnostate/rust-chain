declare const decimalTextBrand: unique symbol

/**
 * Canonical, plain-notation decimal text.
 *
 * Finance boundaries use this branded string instead of IEEE-754 numbers. Values can only be
 * created through the validation helpers in this module, so exponent notation and non-finite
 * values never reach a mutation payload.
 */
export type DecimalText = string & { readonly [decimalTextBrand]: 'DecimalText' }

export interface DecimalTextConstraints {
  allowNegative?: boolean
  allowZero?: boolean
  maxIntegerDigits?: number
  maxScale?: number
}

export interface DecimalFormatOptions {
  maximumFractionDigits?: number
  minimumFractionDigits?: number
  preserveNonZero?: boolean
  useGrouping?: boolean
}

export type DecimalBoundary = DecimalText | string | number | null | undefined

export interface DecimalRange {
  available?: DecimalBoundary
  maximum?: DecimalBoundary
  minimum?: DecimalBoundary
}

interface ParsedDecimal {
  coefficient: bigint
  scale: number
}

// A decimal point always requires at least one following digit. Inputs such as
// `12.` are in-progress form drafts, not transport-safe DecimalText values.
const DECIMAL_PATTERN = /^([+-]?)(?:(\d+)(?:\.(\d+))?|\.(\d+))$/
const POWERS_OF_TEN = new Map<number, bigint>([[0, 1n]])

export function normalizeDecimalText(
  value: string,
  constraints: DecimalTextConstraints = {},
): DecimalText {
  if (typeof value !== 'string') throw new TypeError('decimal value must be text')
  const source = value.trim()
  const match = DECIMAL_PATTERN.exec(source)
  if (!match) throw new TypeError('invalid decimal text')

  const integerSource = match[2] ?? '0'
  const fractionSource = match[3] ?? match[4] ?? ''
  const integer = integerSource.replace(/^0+(?=\d)/, '') || '0'
  const fraction = fractionSource.replace(/0+$/, '')
  const isZero = integer === '0' && fraction.length === 0
  const negative = match[1] === '-' && !isZero

  const maxScale = constraints.maxScale
  if (maxScale !== undefined
    && (!Number.isSafeInteger(maxScale) || maxScale < 0 || fraction.length > maxScale)) {
    throw new RangeError('decimal scale exceeds limit')
  }
  const maxIntegerDigits = constraints.maxIntegerDigits
  if (maxIntegerDigits !== undefined
    && (!Number.isSafeInteger(maxIntegerDigits)
      || maxIntegerDigits < 1
      || integer.length > maxIntegerDigits)) {
    throw new RangeError('decimal integer digits exceed limit')
  }
  if (negative && constraints.allowNegative === false) {
    throw new RangeError('negative decimal is not allowed')
  }
  if (isZero && constraints.allowZero === false) {
    throw new RangeError('zero decimal is not allowed')
  }

  return `${negative ? '-' : ''}${integer}${fraction ? `.${fraction}` : ''}` as DecimalText
}

export function tryNormalizeDecimalText(
  value: string,
  constraints: DecimalTextConstraints = {},
): DecimalText | null {
  try {
    return normalizeDecimalText(value, constraints)
  } catch {
    return null
  }
}

/** Strict adapter for untrusted backend JSON. Backend Decimal values must be JSON strings. */
export function requiredDecimalText(
  value: unknown,
  field: string,
  contract: string,
  constraints: DecimalTextConstraints = {},
): DecimalText {
  if (typeof value !== 'string') throw new Error(`invalid ${contract} ${field}`)
  try {
    return normalizeDecimalText(value, constraints)
  } catch {
    throw new Error(`invalid ${contract} ${field}`)
  }
}

export function nullableDecimalText(
  value: unknown,
  field: string,
  contract: string,
  constraints: DecimalTextConstraints = {},
): DecimalText | null {
  return value === null
    ? null
    : requiredDecimalText(value, field, contract, constraints)
}

/** Number adapter for legacy read models only. Mutation contracts must accept DecimalText. */
export function decimalTextFromFiniteNumber(value: number): DecimalText {
  if (!Number.isFinite(value)) throw new TypeError('decimal number must be finite')
  return normalizeDecimalText(expandExponentialNotation(value.toString()))
}

/** Preserves backend/input text and confines legacy read-model numbers to one adapter. */
export function decimalTextFromBoundary(
  value: DecimalBoundary,
  constraints: DecimalTextConstraints = {},
): DecimalText | null {
  if (typeof value === 'string') return tryNormalizeDecimalText(value, constraints)
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  try {
    return normalizeDecimalText(decimalTextFromFiniteNumber(value), constraints)
  } catch {
    return null
  }
}

export function positiveDecimalInput(value: string, maxScale = 18): DecimalText | null {
  return tryNormalizeDecimalText(value, {
    allowNegative: false,
    allowZero: false,
    maxIntegerDigits: 20,
    maxScale,
  })
}

export function decimalWithinRange(value: DecimalText | null, range: DecimalRange): boolean {
  if (!value || decimalSign(value) <= 0) return false
  const minimum = decimalTextFromBoundary(range.minimum)
  const maximum = decimalTextFromBoundary(range.maximum)
  const available = decimalTextFromBoundary(range.available)
  return (!minimum || decimalCompare(value, minimum) >= 0)
    && (!maximum || decimalCompare(value, maximum) <= 0)
    && (!available || decimalCompare(value, available) <= 0)
}

export function decimalMinimum(
  left: DecimalBoundary,
  right: DecimalBoundary,
): DecimalText | null {
  const normalizedLeft = decimalTextFromBoundary(left)
  const normalizedRight = decimalTextFromBoundary(right)
  if (!normalizedLeft) return normalizedRight
  if (!normalizedRight) return normalizedLeft
  return decimalCompare(normalizedLeft, normalizedRight) <= 0 ? normalizedLeft : normalizedRight
}

/** Applies a non-money UI ratio while keeping the resulting financial value exact. */
export function decimalPortion(
  value: DecimalText,
  numerator: number,
  denominator = 100,
  scale = 18,
): DecimalText {
  if (!Number.isSafeInteger(numerator) || numerator < 0) throw new RangeError('invalid decimal portion')
  if (!Number.isSafeInteger(denominator) || denominator <= 0) throw new RangeError('invalid decimal portion')
  return decimalTruncate(decimalDivide(
    decimalMultiply(value, decimalTextFromFiniteNumber(numerator)),
    decimalTextFromFiniteNumber(denominator),
    scale,
  ), scale)
}

export function decimalCompare(left: DecimalText, right: DecimalText): -1 | 0 | 1 {
  const leftValue = parseDecimal(left)
  const rightValue = parseDecimal(right)
  const scale = Math.max(leftValue.scale, rightValue.scale)
  const leftCoefficient = leftValue.coefficient * powerOfTen(scale - leftValue.scale)
  const rightCoefficient = rightValue.coefficient * powerOfTen(scale - rightValue.scale)
  return leftCoefficient < rightCoefficient ? -1 : leftCoefficient > rightCoefficient ? 1 : 0
}

export function decimalSign(value: DecimalText): -1 | 0 | 1 {
  const coefficient = parseDecimal(value).coefficient
  return coefficient < 0n ? -1 : coefficient > 0n ? 1 : 0
}

export function decimalAdd(left: DecimalText, right: DecimalText): DecimalText {
  const leftValue = parseDecimal(left)
  const rightValue = parseDecimal(right)
  const scale = Math.max(leftValue.scale, rightValue.scale)
  return renderDecimal({
    coefficient: leftValue.coefficient * powerOfTen(scale - leftValue.scale)
      + rightValue.coefficient * powerOfTen(scale - rightValue.scale),
    scale,
  })
}

export function decimalSubtract(left: DecimalText, right: DecimalText): DecimalText {
  return decimalAdd(left, decimalNegate(right))
}

export function decimalNegate(value: DecimalText): DecimalText {
  const parsed = parseDecimal(value)
  return renderDecimal({ coefficient: -parsed.coefficient, scale: parsed.scale })
}

export function decimalAbsolute(value: DecimalText): DecimalText {
  const parsed = parseDecimal(value)
  return renderDecimal({
    coefficient: parsed.coefficient < 0n ? -parsed.coefficient : parsed.coefficient,
    scale: parsed.scale,
  })
}

export function decimalMultiply(left: DecimalText, right: DecimalText): DecimalText {
  const leftValue = parseDecimal(left)
  const rightValue = parseDecimal(right)
  return renderDecimal({
    coefficient: leftValue.coefficient * rightValue.coefficient,
    scale: leftValue.scale + rightValue.scale,
  })
}

/** Divide with deterministic truncation toward zero at the requested decimal scale. */
export function decimalDivide(
  dividend: DecimalText,
  divisor: DecimalText,
  scale = 18,
): DecimalText {
  if (!Number.isSafeInteger(scale) || scale < 0 || scale > 100) {
    throw new RangeError('invalid decimal division scale')
  }
  const left = parseDecimal(dividend)
  const right = parseDecimal(divisor)
  if (right.coefficient === 0n) throw new RangeError('decimal division by zero')

  let numerator = left.coefficient
  let denominator = right.coefficient
  const exponent = right.scale + scale - left.scale
  if (exponent >= 0) numerator *= powerOfTen(exponent)
  else denominator *= powerOfTen(-exponent)

  return renderDecimal({ coefficient: numerator / denominator, scale })
}

export function decimalTruncate(value: DecimalText, scale: number): DecimalText {
  if (!Number.isSafeInteger(scale) || scale < 0 || scale > 100) {
    throw new RangeError('invalid decimal scale')
  }
  const parsed = parseDecimal(value)
  if (parsed.scale <= scale) return value
  return renderDecimal({
    coefficient: parsed.coefficient / powerOfTen(parsed.scale - scale),
    scale,
  })
}

/** Rounds a DecimalText half away from zero without converting through IEEE-754. */
export function decimalRoundHalfUp(value: DecimalText, scale: number): DecimalText {
  if (!Number.isSafeInteger(scale) || scale < 0 || scale > 100) {
    throw new RangeError('invalid decimal scale')
  }
  const parsed = parseDecimal(value)
  if (parsed.scale <= scale) return value

  const divisor = powerOfTen(parsed.scale - scale)
  const negative = parsed.coefficient < 0n
  const absolute = negative ? -parsed.coefficient : parsed.coefficient
  let rounded = absolute / divisor
  if ((absolute % divisor) * 2n >= divisor) rounded += 1n

  return renderDecimal({
    coefficient: negative ? -rounded : rounded,
    scale,
  })
}

export function decimalFractionDigits(value: string): number {
  if (typeof value !== 'string') throw new TypeError('decimal value must be text')
  const match = DECIMAL_PATTERN.exec(value.trim())
  if (!match) throw new TypeError('invalid decimal text')
  return (match[3] ?? match[4] ?? '').length
}

export function formatDecimalText(
  value: DecimalText,
  locale: string,
  options: DecimalFormatOptions = {},
): string {
  const parsed = parseDecimal(value)
  const absoluteCoefficient = parsed.coefficient < 0n ? -parsed.coefficient : parsed.coefficient
  const padded = absoluteCoefficient.toString().padStart(parsed.scale + 1, '0')
  const integer = parsed.scale ? padded.slice(0, -parsed.scale) || '0' : padded
  const fullFraction = parsed.scale ? padded.slice(-parsed.scale) : ''
  const requestedMaximum = options.maximumFractionDigits ?? Math.max(18, parsed.scale)
  const minimum = options.minimumFractionDigits ?? 0
  if (!Number.isSafeInteger(requestedMaximum) || requestedMaximum < 0 || requestedMaximum > 100) {
    throw new RangeError('invalid maximum fraction digits')
  }
  if (!Number.isSafeInteger(minimum) || minimum < 0 || minimum > requestedMaximum) {
    throw new RangeError('invalid minimum fraction digits')
  }

  let maximum = requestedMaximum
  if (options.preserveNonZero !== false && parsed.coefficient !== 0n && integer === '0') {
    const firstNonZero = fullFraction.search(/[1-9]/)
    if (firstNonZero >= 0) maximum = Math.max(maximum, firstNonZero + 1)
  }
  let fraction = fullFraction.slice(0, maximum).replace(/0+$/, '')
  if (fraction.length < minimum) fraction = fraction.padEnd(minimum, '0')

  const formatter = new Intl.NumberFormat(locale, {
    maximumFractionDigits: 0,
    useGrouping: options.useGrouping !== false,
  })
  const groupedInteger = formatter.format(BigInt(integer))
  const parts = new Intl.NumberFormat(locale, { minimumFractionDigits: 1 }).formatToParts(1.1)
  const decimalSeparator = parts.find((part) => part.type === 'decimal')?.value || '.'
  const minusSign = new Intl.NumberFormat(locale).formatToParts(-1)
    .find((part) => part.type === 'minusSign')?.value || '-'
  return `${parsed.coefficient < 0n ? minusSign : ''}${groupedInteger}${fraction ? `${decimalSeparator}${fraction}` : ''}`
}

/** Converts only an already-bounded unit ratio to number for pixel geometry. */
export function decimalUnitRatioToNumber(value: DecimalText): number {
  const zero = normalizeDecimalText('0')
  const one = normalizeDecimalText('1')
  if (decimalCompare(value, zero) < 0 || decimalCompare(value, one) > 0) {
    throw new RangeError('decimal ratio is outside 0..1')
  }
  return Number(value)
}

function parseDecimal(value: DecimalText): ParsedDecimal {
  const source = value as string
  const negative = source.startsWith('-')
  const unsigned = negative ? source.slice(1) : source
  const [integer = '0', fraction = ''] = unsigned.split('.')
  const coefficient = BigInt(`${integer}${fraction}`)
  return { coefficient: negative ? -coefficient : coefficient, scale: fraction.length }
}

function renderDecimal(value: ParsedDecimal): DecimalText {
  if (value.coefficient === 0n) return '0' as DecimalText
  const negative = value.coefficient < 0n
  const absolute = negative ? -value.coefficient : value.coefficient
  const digits = absolute.toString().padStart(value.scale + 1, '0')
  const integer = value.scale ? digits.slice(0, -value.scale) || '0' : digits
  const fraction = value.scale ? digits.slice(-value.scale).replace(/0+$/, '') : ''
  return `${negative ? '-' : ''}${integer}${fraction ? `.${fraction}` : ''}` as DecimalText
}

function powerOfTen(exponent: number): bigint {
  if (!Number.isSafeInteger(exponent) || exponent < 0 || exponent > 200) {
    throw new RangeError('invalid decimal exponent')
  }
  const cached = POWERS_OF_TEN.get(exponent)
  if (cached !== undefined) return cached
  const value = 10n ** BigInt(exponent)
  POWERS_OF_TEN.set(exponent, value)
  return value
}

function expandExponentialNotation(value: string): string {
  const match = /^([+-]?)(\d+)(?:\.(\d*))?[eE]([+-]?\d+)$/.exec(value)
  if (!match) return value
  const sign = match[1] || ''
  const whole = match[2] || '0'
  const fraction = match[3] || ''
  const exponent = Number(match[4])
  if (!Number.isSafeInteger(exponent)) throw new TypeError('invalid decimal exponent')
  const digits = `${whole}${fraction}`
  const point = whole.length + exponent
  if (point <= 0) return `${sign}0.${'0'.repeat(-point)}${digits}`
  if (point >= digits.length) return `${sign}${digits}${'0'.repeat(point - digits.length)}`
  return `${sign}${digits.slice(0, point)}.${digits.slice(point)}`
}
