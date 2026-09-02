import {
  decimalCompare,
  decimalDivide,
  decimalMultiply,
  decimalNegate,
  decimalTextFromBoundary,
  formatDecimalText,
  normalizeDecimalText,
  positiveDecimalInput,
  type DecimalBoundary,
  type DecimalText,
} from './decimal.ts'
import { formatFinancialAmount } from './financialDisplay.ts'
import { splitSymbol } from './format.ts'

export interface SecondsStakeRange {
  minimum: ExactSecondsDecimal
  maximum?: ExactSecondsDecimal
  available: ExactSecondsDecimal
}

export interface SecondsStakeValidation {
  isValid: boolean
  stakeAmount: DecimalText | null
  minimum: DecimalText | null
  maximum: DecimalText | null
  available: DecimalText | null
}

export interface SecondsOrderReviewRequest {
  readonly productId: number
  readonly durationSeconds: number
  readonly direction: 'up' | 'down'
  readonly stakeAmount: DecimalText
  readonly idempotencyKey: string
}

export interface SecondsOrderReviewSnapshot {
  readonly productId: number
  readonly cycleId: number
  readonly symbol: string
  readonly stakeAssetId: number
  readonly stakeAssetSymbol: string
  readonly durationSeconds: number
  readonly direction: 'up' | 'down'
  readonly stakeAmount: DecimalText
  readonly stakeAmountText: DecimalText
  readonly payoutRate: DecimalText
  readonly payoutRateText: DecimalText
  readonly referencePrice: DecimalText | null
  readonly referencePriceText: DecimalText | null
  readonly estimatedProfit: DecimalText
  readonly idempotencyKey: string
  readonly request: Readonly<SecondsOrderReviewRequest>
}

export interface SecondsOrderReviewInput {
  productId: number
  cycleId: number
  symbol: string
  stakeAssetId: number
  stakeAssetSymbol: string
  durationSeconds: number
  direction: 'up' | 'down'
  stakeAmount: string | DecimalText
  minimumStake: ExactSecondsDecimal
  maximumStake?: ExactSecondsDecimal
  available: ExactSecondsDecimal
  payoutRate: ExactSecondsDecimal
  referencePrice?: ExactSecondsDecimal
  idempotencyKey: string
}

export interface SecondsProfitLossInput {
  result?: string
  stake: DecimalBoundary
  payoutRate: DecimalBoundary
}

export type SecondsProfitLoss =
  | { kind: 'profit'; amount: DecimalText }
  | { kind: 'loss'; amount: DecimalText }
  | { kind: 'unavailable'; amount: null }

export interface SecondsFinancialOrderSource {
  stakeAmount: DecimalBoundary
  stakeAmountText?: DecimalBoundary
  payoutRate: DecimalBoundary
  payoutRateText?: DecimalBoundary
  entryPrice?: DecimalBoundary
  entryPriceText?: DecimalBoundary
  settlementPrice?: DecimalBoundary
  settlementPriceText?: DecimalBoundary
}

export interface SecondsFinancialOrderValues {
  stakeAmount: DecimalText | null
  payoutRate: DecimalText | null
  entryPrice: DecimalText | null
  settlementPrice: DecimalText | null
}

export interface SecondsFinancialOrderRecord extends SecondsFinancialOrderSource {
  id: number
  result?: string
}

export interface SecondsCycleFinancialSource {
  minStake: DecimalBoundary
  maxStake?: DecimalBoundary
  minStakeText?: DecimalBoundary
  maxStakeText?: DecimalBoundary
  payoutRate?: DecimalBoundary
  payoutRateText?: DecimalBoundary
}

export interface SecondsWalletFinancialSource {
  available: DecimalBoundary
  availableText?: DecimalBoundary
}

export interface SecondsTickerFinancialSource {
  lastPrice?: unknown
  lastPriceText?: unknown
  changePercent?: unknown
}

export interface SecondsFinancialPresentationOptions {
  locale: () => string
  exactByOrderId: ReadonlyMap<number, Pick<SecondsFinancialOrderValues, 'stakeAmount' | 'payoutRate'>>
  normalizeSymbol: (symbol: string) => string
  liveTickerFor: (symbol: string) => SecondsTickerFinancialSource | undefined
  marketTickerFor: (symbol: string) => SecondsTickerFinancialSource | undefined
  selectedSymbol: () => string
  selectedCandleClose: () => unknown
  translate: (key: string, params?: Record<string, string>) => string
}

const ZERO = normalizeDecimalText('0')
const HUNDRED = normalizeDecimalText('100')
type ExactSecondsDecimal = string | DecimalText | null | undefined

export function validateSecondsStake(
  value: string | DecimalText,
  range: SecondsStakeRange,
): SecondsStakeValidation {
  const stakeAmount = positiveDecimalInput(value, 18)
  const minimum = nonNegativeSecondsBoundary(range.minimum)
  const maximum = positiveSecondsBoundary(range.maximum)
  const available = nonNegativeSecondsBoundary(range.available)
  const isValid = Boolean(
    stakeAmount
    && minimum
    && available
    && decimalCompare(stakeAmount, minimum) >= 0
    && (!maximum || decimalCompare(stakeAmount, maximum) <= 0)
    && decimalCompare(stakeAmount, available) <= 0,
  )
  return { isValid, stakeAmount, minimum, maximum, available }
}

/** Creates the sole immutable Seconds confirmation and request snapshot. */
export function createSecondsOrderReviewSnapshot(
  input: SecondsOrderReviewInput,
): Readonly<SecondsOrderReviewSnapshot> | null {
  const stake = validateSecondsStake(input.stakeAmount, {
    minimum: input.minimumStake,
    maximum: input.maximumStake,
    available: input.available,
  })
  const payoutRate = positiveSecondsBoundary(input.payoutRate)
  const referencePrice = positiveSecondsBoundary(input.referencePrice)
  if (
    !stake.isValid
    || !stake.stakeAmount
    || !payoutRate
    || !Number.isSafeInteger(input.productId)
    || input.productId <= 0
    || !Number.isSafeInteger(input.cycleId)
    || input.cycleId <= 0
    || !Number.isSafeInteger(input.durationSeconds)
    || input.durationSeconds <= 0
  ) {
    return null
  }

  const estimatedProfit = decimalMultiply(stake.stakeAmount, payoutRate)
  const request = Object.freeze({
    productId: input.productId,
    durationSeconds: input.durationSeconds,
    direction: input.direction,
    stakeAmount: stake.stakeAmount,
    idempotencyKey: input.idempotencyKey,
  })
  return Object.freeze({
    productId: input.productId,
    cycleId: input.cycleId,
    symbol: input.symbol,
    stakeAssetId: input.stakeAssetId,
    stakeAssetSymbol: input.stakeAssetSymbol,
    durationSeconds: input.durationSeconds,
    direction: input.direction,
    stakeAmount: stake.stakeAmount,
    stakeAmountText: stake.stakeAmount,
    payoutRate,
    payoutRateText: payoutRate,
    referencePrice,
    referencePriceText: referencePrice,
    estimatedProfit,
    idempotencyKey: input.idempotencyKey,
    request,
  })
}

export function deriveSecondsEstimatedProfit(
  stake: DecimalBoundary,
  payoutRate: DecimalBoundary,
): DecimalText | null {
  const stakeText = nonNegativeSecondsBoundary(stake)
  const payoutRateText = nonNegativeSecondsBoundary(payoutRate)
  return stakeText && payoutRateText
    ? decimalMultiply(stakeText, payoutRateText)
    : null
}

export function deriveSecondsProfitLoss(input: SecondsProfitLossInput): SecondsProfitLoss {
  const result = input.result?.trim().toLowerCase()
  const stake = nonNegativeSecondsBoundary(input.stake)
  if (!stake) return { kind: 'unavailable', amount: null }
  if (result === 'win') {
    const amount = deriveSecondsEstimatedProfit(stake, input.payoutRate)
    return amount ? { kind: 'profit', amount } : { kind: 'unavailable', amount: null }
  }
  if (result === 'loss') return { kind: 'loss', amount: decimalNegate(stake) }
  return { kind: 'unavailable', amount: null }
}

/** Derives a signed return percentage from exact amount/stake text. */
export function deriveSecondsReturnRatePercent(
  amount: ExactSecondsDecimal,
  stake: ExactSecondsDecimal,
  scale = 18,
): DecimalText | null {
  const amountText = exactSignedSecondsBoundary(amount)
  const stakeText = positiveSecondsBoundary(stake)
  if (!amountText || !stakeText) return null
  return decimalMultiply(decimalDivide(amountText, stakeText, scale), HUNDRED)
}

/** Prefer exact DTO/review text; numeric legacy fields are display-only and fail closed here. */
export function secondsFinancialOrderValues(
  source: SecondsFinancialOrderSource,
): SecondsFinancialOrderValues {
  return {
    stakeAmount: nonNegativeSecondsBoundary(exactSecondsSource(
      source.stakeAmountText,
      source.stakeAmount,
    )),
    payoutRate: nonNegativeSecondsBoundary(exactSecondsSource(
      source.payoutRateText,
      source.payoutRate,
    )),
    entryPrice: positiveSecondsBoundary(exactSecondsSource(
      source.entryPriceText,
      source.entryPrice,
    )),
    settlementPrice: positiveSecondsBoundary(exactSecondsSource(
      source.settlementPriceText,
      source.settlementPrice,
    )),
  }
}

export function formatSecondsFinancial(
  value: DecimalBoundary,
  locale: string,
  maximumFractionDigits = 8,
  unavailable = '--',
): string {
  return formatFinancialAmount(value, locale, {
    maximumFractionDigits,
    unavailable,
  })
}

export function formatSecondsPercent(
  value: DecimalBoundary,
  locale: string,
  fractionDigits: number,
): string {
  const normalized = decimalTextFromBoundary(value)
  if (!normalized) return '--'
  return formatDecimalText(normalized, locale, {
    maximumFractionDigits: fractionDigits,
    minimumFractionDigits: fractionDigits,
    preserveNonZero: false,
    useGrouping: false,
  })
}

/** Binds view-local exact snapshots and locale without moving arithmetic back into the SFC. */
export function createSecondsFinancialPresentation(options: SecondsFinancialPresentationOptions) {
  const formatValue = (
    value: DecimalBoundary,
    maximumFractionDigits = 8,
    unavailable = '--',
  ) => formatSecondsFinancial(value, options.locale(), maximumFractionDigits, unavailable)
  const formatPayoutRate = (value: DecimalBoundary, fractionDigits: number): string => {
    // Compatibility presentation only. This numeric-capable branch never feeds a review or PnL.
    const normalized = decimalTextFromBoundary(value)
    const percent = normalized ? decimalMultiply(normalized, HUNDRED) : null
    return percent ? formatSecondsPercent(percent, options.locale(), fractionDigits) : '--'
  }
  const orderFinancials = (order: SecondsFinancialOrderRecord): SecondsFinancialOrderValues => {
    const exact = options.exactByOrderId.get(order.id)
    return secondsFinancialOrderValues({
      stakeAmount: order.stakeAmount,
      stakeAmountText: exact?.stakeAmount ?? order.stakeAmountText,
      payoutRate: order.payoutRate,
      payoutRateText: exact?.payoutRate ?? order.payoutRateText,
      entryPrice: order.entryPrice,
      entryPriceText: order.entryPriceText,
      settlementPrice: order.settlementPrice,
      settlementPriceText: order.settlementPriceText,
    })
  }
  const profitLoss = (order: SecondsFinancialOrderRecord): SecondsProfitLoss => {
    const financials = orderFinancials(order)
    return deriveSecondsProfitLoss({
      result: order.result,
      stake: financials.stakeAmount,
      payoutRate: financials.payoutRate,
    })
  }
  const estimatedProfit = (order: SecondsFinancialOrderRecord): DecimalText | null => {
    const financials = orderFinancials(order)
    return deriveSecondsEstimatedProfit(financials.stakeAmount, financials.payoutRate)
  }
  const cycleMinimum = (cycle: SecondsCycleFinancialSource | undefined): DecimalText | null => (
    nonNegativeSecondsBoundary(exactSecondsSource(cycle?.minStakeText, cycle?.minStake))
  )
  const cycleMaximum = (cycle: SecondsCycleFinancialSource | undefined): DecimalText | null => (
    positiveSecondsBoundary(exactSecondsSource(cycle?.maxStakeText, cycle?.maxStake))
  )
  const walletAvailable = (wallet: SecondsWalletFinancialSource | undefined): DecimalText | null => (
    nonNegativeSecondsBoundary(exactSecondsSource(wallet?.availableText, wallet?.available))
  )
  const normalizeProductSymbol = (value: string): string => options.normalizeSymbol(value)
  const displayProductSymbol = (value: string): string => {
    const pair = splitSymbol(value)
    return pair.base && pair.quote ? `${pair.base}/${pair.quote}` : value
  }
  const baseSymbol = (value: string): string => splitSymbol(value).base || value
  const matchesProductSearch = (symbol: string, rawQuery: string): boolean => {
    const query = rawQuery.trim().toUpperCase()
    if (!query) return true
    const pair = splitSymbol(symbol)
    return [symbol, displayProductSymbol(symbol), pair.base, pair.quote]
      .some((value) => value.toUpperCase().includes(query))
  }
  const cycleHasMaximum = (cycle: SecondsCycleFinancialSource | undefined): boolean => (
    cycle?.maxStakeText !== undefined || cycle?.maxStake !== undefined
  )
  const hasExactStakeRange = (cycle: SecondsCycleFinancialSource | undefined): boolean => Boolean(
    cycle
    && cycleMinimum(cycle) !== null
    && (!cycleHasMaximum(cycle) || cycleMaximum(cycle) !== null),
  )
  const exactCyclePayoutRate = (
    cycle: SecondsCycleFinancialSource | undefined,
  ): DecimalText | null => positiveSecondsBoundary(exactSecondsSource(
    cycle?.payoutRateText,
    cycle?.payoutRate,
  ))
  const exactPriceForSymbol = (symbol: string): DecimalText | null => {
    const livePrice = positiveSecondsBoundary(options.liveTickerFor(symbol)?.lastPriceText)
    return livePrice
      ?? positiveSecondsBoundary(options.marketTickerFor(symbol)?.lastPriceText)
  }
  /** Compatibility price for rendering only; it never feeds a review or financial decision. */
  const priceFor = (symbol: string): DecimalText | number | null => {
    const liveTicker = options.liveTickerFor(symbol)
    const livePrice = positiveSecondsBoundary(liveTicker?.lastPriceText)
    if (livePrice) return livePrice
    const legacyLivePrice = positiveLegacyDisplayNumber(liveTicker?.lastPrice)
    if (legacyLivePrice !== null) return legacyLivePrice
    if (normalizeProductSymbol(symbol) === normalizeProductSymbol(options.selectedSymbol())) {
      const candlePrice = positiveLegacyDisplayNumber(options.selectedCandleClose())
      if (candlePrice !== null) return candlePrice
    }
    const ticker = options.marketTickerFor(symbol)
    return positiveSecondsBoundary(ticker?.lastPriceText)
      ?? positiveLegacyDisplayNumber(ticker?.lastPrice)
  }
  const displayChangePercent = (
    liveTicker: SecondsTickerFinancialSource | undefined,
    snapshotTicker: SecondsTickerFinancialSource | undefined,
  ): number | null => (
    finiteDisplayNumber(liveTicker?.changePercent)
    ?? finiteDisplayNumber(snapshotTicker?.changePercent)
  )
  const countdownLabel = (milliseconds: number): string => {
    const totalSeconds = Math.max(0, Math.ceil(milliseconds / 1000))
    const minutes = Math.floor(totalSeconds / 60)
    const seconds = totalSeconds % 60
    return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
  }
  const formatCycleLimit = (
    cycle: SecondsCycleFinancialSource | undefined,
    asset: string,
  ): string => {
    if (!cycle || !hasExactStakeRange(cycle)) return '--'
    const minimum = cycleMinimum(cycle)
    const maximum = cycleMaximum(cycle)
    if (!minimum) return '--'
    return cycleHasMaximum(cycle) && maximum
      ? options.translate('seconds.cycleLimitRange', {
          minimum: formatValue(minimum),
          maximum: formatValue(maximum),
          asset,
        })
      : options.translate('seconds.cycleLimitMinimum', {
          minimum: formatValue(minimum),
          asset,
        })
  }
  const formatOrderAction = (
    direction: 'up' | 'down',
    stake: DecimalText | null,
    asset: string,
  ): string => options.translate('seconds.orderAction', {
    direction: options.translate(direction === 'up' ? 'seconds.bullish' : 'seconds.bearish'),
    amount: stake ? formatValue(stake) : '--',
    asset,
  })
  return {
    baseSymbol,
    countdownLabel,
    cycleMaximum,
    cycleHasMaximum,
    cycleMinimum,
    displayChangePercent,
    displayProductSymbol,
    estimatedProfit,
    exactCyclePayoutRate,
    exactPriceForSymbol,
    formatCycleLimit,
    formatOrderAction,
    formatPayoutRate,
    formatValue,
    hasExactStakeRange,
    matchesProductSearch,
    normalizeProductSymbol,
    orderFinancials,
    priceFor,
    profitLoss,
    walletAvailable,
  }
}

function finiteDisplayNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function positiveLegacyDisplayNumber(value: unknown): number | null {
  const display = finiteDisplayNumber(value)
  return display !== null && display > 0 ? display : null
}

export function positiveSecondsBoundary(value: unknown): DecimalText | null {
  return typeof value === 'string' ? positiveDecimalInput(value, 18) : null
}

export function nonNegativeSecondsBoundary(value: unknown): DecimalText | null {
  if (typeof value !== 'string') return null
  const normalized = decimalTextFromBoundary(value, {
    allowNegative: false,
    maxIntegerDigits: 20,
    maxScale: 18,
  })
  return normalized && decimalCompare(normalized, ZERO) >= 0 ? normalized : null
}

function exactSignedSecondsBoundary(value: unknown): DecimalText | null {
  if (typeof value !== 'string') return null
  // Derived products can legitimately carry the sum of both operand scales.
  const normalized = decimalTextFromBoundary(value)
  return normalized
}

function exactSecondsSource(preferred: unknown, legacy: unknown): string | null {
  if (typeof preferred === 'string') return preferred
  return typeof legacy === 'string' ? legacy : null
}
