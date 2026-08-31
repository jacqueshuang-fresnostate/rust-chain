import {
  decimalAdd,
  decimalCompare,
  decimalDivide,
  decimalMultiply,
  decimalSubtract,
  decimalTextFromBoundary,
  decimalTruncate,
  normalizeDecimalText,
  type DecimalBoundary,
  type DecimalText,
} from './decimal.ts'

export interface WithdrawalFeeTier {
  minAmount: number
  maxAmount?: number
  feeRatePercent: number
  minAmountText?: DecimalText
  maxAmountText?: DecimalText
  feeRatePercentText?: DecimalText
}

interface ExactDecimal {
  units: bigint
  scale: number
}

const WITHDRAWAL_DECIMAL_PATTERN = /^\d+(?:\.\d+)?$/
const MAX_WITHDRAWAL_DECIMAL_TEXT_LENGTH = 80

/** Server DECIMAL values must stay plain, unsigned decimal text; exponent notation is rejected. */
export function isWithdrawalDecimalString(value: string): boolean {
  const normalized = value.trim()
  return normalized.length > 0
    && normalized.length <= MAX_WITHDRAWAL_DECIMAL_TEXT_LENGTH
    && WITHDRAWAL_DECIMAL_PATTERN.test(normalized)
}

/** Checks quote arithmetic with integers so values beyond Number's safe precision remain exact. */
export function withdrawalQuoteAmountsAreConsistent(
  amount: string,
  fee: string,
  net: string,
  totalReserved: string,
): boolean {
  const parsed = [amount, fee, net, totalReserved].map(parseExactDecimal)
  if (parsed.some((value) => value === null)) return false
  const [parsedAmount, parsedFee, parsedNet, parsedTotal] = parsed as [
    ExactDecimal,
    ExactDecimal,
    ExactDecimal,
    ExactDecimal,
  ]
  if (parsedAmount.units <= 0n || parsedFee.units < 0n || parsedNet.units <= 0n || parsedTotal.units <= 0n) {
    return false
  }
  const scale = Math.max(
    parsedAmount.scale,
    parsedFee.scale,
    parsedNet.scale,
    parsedTotal.scale,
  )
  const amountUnits = alignDecimal(parsedAmount, scale)
  const feeUnits = alignDecimal(parsedFee, scale)
  return alignDecimal(parsedNet, scale) === amountUnits
    && alignDecimal(parsedTotal, scale) === amountUnits + feeUnits
}

/** Preview only: the server quote remains the authority used for submission and freezing. */
export function normalizeWithdrawalPreviewAmount(amount: number, precisionScale = 18): number {
  return truncatePreviewDecimal(amount, precisionScale)
}

/** Preview only: the server quote remains the authority used for submission and freezing. */
export function calculateWithdrawalFee(
  amount: number,
  fixedFee: number,
  tiers: readonly WithdrawalFeeTier[],
  precisionScale = 18,
): number {
  const normalizedAmount = normalizeWithdrawalPreviewAmount(amount, precisionScale)
  if (!Number.isFinite(normalizedAmount) || normalizedAmount <= 0) return 0
  const tier = tiers.find((candidate) => (
    normalizedAmount >= candidate.minAmount
    && (candidate.maxAmount === undefined || normalizedAmount < candidate.maxAmount)
  ))
  const rawFee = tier
    ? normalizedAmount * tier.feeRatePercent / 100
    : Math.max(0, fixedFee)
  return truncatePreviewDecimal(Math.max(0, rawFee), precisionScale)
}

/** Finds the largest precision-normalized preview amount whose amount + fee fits available. */
export function maximumQuotedWithdrawalAmount(
  available: number,
  fixedFee: number,
  tiers: readonly WithdrawalFeeTier[],
  precisionScale = 18,
): number {
  if (!Number.isFinite(available) || available <= 0) return 0
  const scale = normalizePrecisionScale(precisionScale)

  // Tier rates can drop at a boundary, so amount + fee is only monotonic inside
  // each tier/fallback segment. Search every segment instead of assuming one
  // globally monotonic curve and accidentally missing a later feasible tier.
  const boundaries = [0, available]
  for (const tier of tiers) {
    if (Number.isFinite(tier.minAmount) && tier.minAmount > 0 && tier.minAmount < available) {
      boundaries.push(tier.minAmount)
    }
    if (tier.maxAmount !== undefined
      && Number.isFinite(tier.maxAmount)
      && tier.maxAmount > 0
      && tier.maxAmount < available) {
      boundaries.push(tier.maxAmount)
    }
  }
  boundaries.sort((left, right) => left - right)
  const points = boundaries.filter((value, index) => index === 0 || value !== boundaries[index - 1])
  const normalizedCandidate = (value: number): number => truncatePreviewDecimal(value, scale)
  const fits = (value: number): boolean => {
    const candidate = normalizedCandidate(value)
    return candidate >= 0
      && candidate + calculateWithdrawalFee(candidate, fixedFee, tiers, scale) <= available
  }
  let best = 0
  for (const point of points) {
    const candidate = normalizedCandidate(point)
    if (fits(candidate)) best = Math.max(best, candidate)
  }
  for (let index = 0; index + 1 < points.length; index += 1) {
    let low = points[index] ?? 0
    let high = points[index + 1] ?? available
    if (!fits(low)) continue
    for (let iteration = 0; iteration < 64; iteration += 1) {
      const middle = (low + high) / 2
      if (fits(middle)) low = middle
      else high = middle
    }
    best = Math.max(best, normalizedCandidate(low))
  }
  return normalizedCandidate(best)
}

export function normalizeWithdrawalPreviewAmountText(
  amount: DecimalText,
  precisionScale = 18,
): DecimalText {
  return decimalTruncate(amount, normalizePrecisionScale(precisionScale))
}

export function calculateWithdrawalFeeText(
  amount: DecimalText,
  fixedFee: DecimalBoundary,
  tiers: readonly WithdrawalFeeTier[],
  precisionScale = 18,
): DecimalText {
  const normalizedAmount = normalizeWithdrawalPreviewAmountText(amount, precisionScale)
  if (decimalCompare(normalizedAmount, normalizeDecimalText('0')) <= 0) return normalizeDecimalText('0')
  const tier = tiers.find((candidate) => {
    const minimum = tierBoundary(candidate.minAmountText ?? candidate.minAmount) || normalizeDecimalText('0')
    const maximum = tierBoundary(candidate.maxAmountText ?? candidate.maxAmount)
    return decimalCompare(normalizedAmount, minimum) >= 0
      && (!maximum || decimalCompare(normalizedAmount, maximum) < 0)
  })
  const fee = tier
    ? decimalDivide(
        decimalMultiply(normalizedAmount, tierBoundary(tier.feeRatePercentText ?? tier.feeRatePercent) || normalizeDecimalText('0')),
        normalizeDecimalText('100'),
        normalizePrecisionScale(precisionScale),
      )
    : tierBoundary(fixedFee) || normalizeDecimalText('0')
  return decimalTruncate(fee, normalizePrecisionScale(precisionScale))
}

/** Exact piecewise candidate search for the largest amount whose reserve can fit. */
export function maximumQuotedWithdrawalAmountText(
  available: DecimalText,
  fixedFee: DecimalBoundary,
  tiers: readonly WithdrawalFeeTier[],
  precisionScale = 18,
): DecimalText {
  const scale = normalizePrecisionScale(precisionScale)
  const zero = normalizeDecimalText('0')
  const normalizedAvailable = decimalTruncate(available, scale)
  if (decimalCompare(normalizedAvailable, zero) <= 0) return zero
  const unit = normalizeDecimalText(scale === 0 ? '1' : `0.${'0'.repeat(scale - 1)}1`)
  const candidates: DecimalText[] = [normalizedAvailable]
  const fixed = tierBoundary(fixedFee) || zero
  if (decimalCompare(normalizedAvailable, fixed) > 0) {
    candidates.push(decimalTruncate(decimalSubtract(normalizedAvailable, fixed), scale))
  }

  for (const tier of tiers) {
    const minimum = tierBoundary(tier.minAmountText ?? tier.minAmount) || zero
    const maximum = tierBoundary(tier.maxAmountText ?? tier.maxAmount)
    const rate = tierBoundary(tier.feeRatePercentText ?? tier.feeRatePercent) || zero
    const divisor = decimalAdd(normalizeDecimalText('1'), decimalDivide(rate, normalizeDecimalText('100'), 36))
    let candidate = decimalTruncate(decimalDivide(normalizedAvailable, divisor, scale), scale)
    if (decimalCompare(candidate, minimum) < 0) candidate = minimum
    if (maximum && decimalCompare(candidate, maximum) >= 0) {
      candidate = decimalCompare(maximum, unit) > 0 ? decimalSubtract(maximum, unit) : zero
    }
    candidates.push(candidate, minimum)
    if (maximum && decimalCompare(maximum, unit) > 0) candidates.push(decimalSubtract(maximum, unit))
  }

  return candidates.reduce((best, rawCandidate) => {
    const candidate = decimalTruncate(rawCandidate, scale)
    if (decimalCompare(candidate, zero) <= 0 || decimalCompare(candidate, normalizedAvailable) > 0) return best
    const reserved = decimalAdd(candidate, calculateWithdrawalFeeText(candidate, fixedFee, tiers, scale))
    return decimalCompare(reserved, normalizedAvailable) <= 0 && decimalCompare(candidate, best) > 0
      ? candidate
      : best
  }, zero)
}

function parseExactDecimal(value: string): ExactDecimal | null {
  const normalized = value.trim()
  if (!isWithdrawalDecimalString(normalized)) return null
  const [whole = '0', fraction = ''] = normalized.split('.')
  return {
    units: BigInt(`${whole}${fraction}`),
    scale: fraction.length,
  }
}

function alignDecimal(value: ExactDecimal, targetScale: number): bigint {
  return value.units * (10n ** BigInt(targetScale - value.scale))
}

function normalizePrecisionScale(value: number): number {
  if (!Number.isFinite(value)) return 18
  return Math.min(18, Math.max(0, Math.trunc(value)))
}

function truncatePreviewDecimal(value: number, precisionScale: number): number {
  if (!Number.isFinite(value)) return 0
  const factor = 10 ** normalizePrecisionScale(precisionScale)
  const scaled = value * factor
  // Offset only binary-representation noise at integer boundaries; the server
  // still performs the authoritative decimal truncation before quoting.
  const tolerance = Math.min(
    0.000_000_1,
    Number.EPSILON * Math.max(1, Math.abs(scaled)) * 2,
  )
  return Math.trunc(scaled + Math.sign(scaled) * tolerance) / factor
}

function tierBoundary(value: DecimalBoundary): DecimalText | null {
  return decimalTextFromBoundary(value, { allowNegative: false, maxIntegerDigits: 20, maxScale: 18 })
}
