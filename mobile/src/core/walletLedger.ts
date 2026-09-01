import {
  normalizeRealizedReturnAssetSymbol,
  normalizeRealizedReturnTimestamp,
  requiredRealizedReturnDecimal,
} from './realizedReturn.ts'
import {
  decimalCompare,
  decimalSign,
  formatDecimalText,
  normalizeDecimalText,
  type DecimalText,
} from './decimal.ts'

export const WALLET_LEDGER_CATEGORIES = [
  'funding',
  'spot',
  'margin',
  'seconds',
  'convert',
  'earn',
  'new_coin',
  'loan',
  'prediction',
  'other',
] as const

export const WALLET_LEDGER_FILTERS = ['all', ...WALLET_LEDGER_CATEGORIES] as const
export const WALLET_LEDGER_ACCOUNT_TYPES = ['spot', 'margin'] as const
export const WALLET_LEDGER_ACCOUNT_FILTERS = ['all', ...WALLET_LEDGER_ACCOUNT_TYPES] as const
export const WALLET_LEDGER_DIRECTIONS = ['all', 'credit', 'debit'] as const
export const WALLET_LEDGER_DATE_PRESETS = ['all', 'today', 'last7Days', 'last30Days'] as const
export const WALLET_LEDGER_MAX_FRACTION_DIGITS = 18

export type WalletLedgerCategory = typeof WALLET_LEDGER_CATEGORIES[number]
export type WalletLedgerFilter = typeof WALLET_LEDGER_FILTERS[number]
export type WalletLedgerAccountType = typeof WALLET_LEDGER_ACCOUNT_TYPES[number]
export type WalletLedgerAccountFilter = typeof WALLET_LEDGER_ACCOUNT_FILTERS[number]
export type WalletLedgerDirection = typeof WALLET_LEDGER_DIRECTIONS[number]
export type WalletLedgerDatePreset = typeof WALLET_LEDGER_DATE_PRESETS[number]

export interface WalletLedgerDateRange {
  startTime?: string
  endTime?: string
}

export interface BackendWalletLedgerEntry {
  id: unknown
  account_type: unknown
  symbol: unknown
  change_type: unknown
  category: unknown
  amount: unknown
  fee: unknown
  balance_after: unknown
  precision_scale: unknown
  created_at: unknown
}

export interface BackendWalletLedgerPage {
  number: unknown
  size: unknown
  total_elements: unknown
  total_pages: unknown
}

export interface BackendWalletLedgerResponse {
  entries: unknown
  page: unknown
}

export interface WalletLedgerEntry {
  id: number
  accountType: WalletLedgerAccountType
  symbol: string
  changeType: string
  category: WalletLedgerCategory
  amount: DecimalText
  fee: DecimalText
  balanceAfter: DecimalText
  precisionScale: number
  createdAt: number
}

export interface WalletLedgerPage {
  entries: WalletLedgerEntry[]
  page: {
    number: number
    size: number
    totalElements: number
    totalPages: number
  }
}

export interface WalletLedgerPageProgress {
  nextOffset: number
  exhausted: boolean
}

export interface WalletLedgerDateGroup {
  key: string
  relation: 'today' | 'yesterday' | 'date'
  date: Date
  entries: WalletLedgerEntry[]
}

export interface WalletLedgerTypePresentation {
  translationKey: string
  source?: string
}

export interface WalletLedgerFetchOptions {
  limit: number
  offset: number
  assetSymbol?: string
  direction: WalletLedgerDirection
  startTime?: string
  endTime?: string
  category?: WalletLedgerCategory
  accountType?: WalletLedgerAccountFilter
  changeType?: string
}

export interface WalletLedgerRequestOptions {
  limit?: number
  offset?: number
  assetSymbol?: string
  direction?: WalletLedgerDirection
  startTime?: string
  endTime?: string
  category?: WalletLedgerCategory
  accountType?: WalletLedgerAccountFilter
  changeType?: string
}

export interface WalletLedgerRequestParams {
  limit: number
  offset: number
  asset_symbol?: string
  direction: WalletLedgerDirection
  start_time?: string
  end_time?: string
  category?: WalletLedgerCategory
  account_type: WalletLedgerAccountFilter
  change_type?: string
}

export interface WalletLedgerPaginationState {
  entries: WalletLedgerEntry[]
  loading: boolean
  loadingMore: boolean
  nextOffset: number
  exhausted: boolean
  initialError: unknown | null
  appendError: unknown | null
}

export type WalletLedgerPaginationOperation =
  | 'loaded'
  | 'error'
  | 'guest'
  | 'stale'
  | 'ignored'

export interface WalletLedgerPaginationController {
  snapshot: () => WalletLedgerPaginationState
  loadInitial: () => Promise<WalletLedgerPaginationOperation>
  loadMore: () => Promise<WalletLedgerPaginationOperation>
  retryLoadMore: () => Promise<WalletLedgerPaginationOperation>
  reset: () => void
  stop: () => void
}

export type WalletLedgerRequestResult =
  | { state: 'guest' }
  | {
    state: 'loaded'
    assetSymbol?: string
    direction: WalletLedgerDirection
    datePreset: WalletLedgerDatePreset
    value: WalletLedgerPage
  }
  | {
    state: 'error'
    assetSymbol?: string
    direction: WalletLedgerDirection
    datePreset: WalletLedgerDatePreset
    error: unknown
  }
  | { state: 'stale' }

export interface WalletLedgerRequestLifecycle {
  load: (offset: number, limit: number) => Promise<WalletLedgerRequestResult>
  invalidate: () => void
  stop: () => void
}

export class WalletLedgerContractError extends Error {
  override readonly name = 'WalletLedgerContractError'
}

const CATEGORY_TRANSLATION_KEYS: Record<WalletLedgerFilter, string> = {
  all: 'ledger.categoryAll',
  funding: 'ledger.categoryFunding',
  spot: 'ledger.categorySpot',
  margin: 'ledger.categoryMargin',
  seconds: 'ledger.categorySeconds',
  convert: 'ledger.categoryConvert',
  earn: 'ledger.categoryEarn',
  new_coin: 'ledger.categoryNewCoin',
  loan: 'ledger.categoryLoan',
  prediction: 'ledger.categoryPrediction',
  other: 'ledger.categoryOther',
}

const ACCOUNT_TRANSLATION_KEYS: Record<WalletLedgerAccountFilter, string> = {
  all: 'ledger.accountAll',
  spot: 'ledger.accountSpot',
  margin: 'ledger.accountMargin',
}

const DIRECTION_TRANSLATION_KEYS: Record<WalletLedgerDirection, string> = {
  all: 'ledger.directionAll',
  credit: 'ledger.directionCredit',
  debit: 'ledger.directionDebit',
}

const DATE_PRESET_TRANSLATION_KEYS: Record<WalletLedgerDatePreset, string> = {
  all: 'ledger.dateAll',
  today: 'ledger.dateToday',
  last7Days: 'ledger.dateLast7Days',
  last30Days: 'ledger.dateLast30Days',
}

const CHANGE_TYPE_TRANSLATION_KEYS = {
  deposit: 'ledger.typeDeposit',
  deposit_credit: 'ledger.typeDepositCredit',
  deposit_confirm: 'ledger.typeDepositConfirm',
  deposit_reorg_reverse: 'ledger.typeDepositReorgReverse',
  withdrawal_reserve: 'ledger.typeWithdrawalReserve',
  withdrawal_release: 'ledger.typeWithdrawalRelease',
  withdrawal_confirm: 'ledger.typeWithdrawalConfirm',
  admin_recharge: 'ledger.typeAdminRecharge',
  quick_recharge: 'ledger.typeQuickRecharge',
  spot_freeze: 'ledger.typeSpotFreeze',
  spot_unfreeze: 'ledger.typeSpotUnfreeze',
  spot_fill: 'ledger.typeSpotFill',
  spot_price_improvement_release: 'ledger.typeSpotPriceImprovementRelease',
  spot_trade_settlement: 'ledger.typeSpotSettlement',
  margin_transfer_in: 'ledger.typeMarginTransferIn',
  margin_transfer_out: 'ledger.typeMarginTransferOut',
  margin_position_open: 'ledger.typeMarginOpen',
  margin_position_close: 'ledger.typeMarginClose',
  margin_position_cancel: 'ledger.typeMarginCancel',
  margin_position_liquidate: 'ledger.typeMarginLiquidate',
  margin_cross_position_close: 'ledger.typeMarginCrossClose',
  margin_cross_account_liquidate: 'ledger.typeMarginCrossLiquidate',
  seconds_contract_open: 'ledger.typeSecondsOpen',
  seconds_contract_settle_win: 'ledger.typeSecondsSettleWin',
  convert_settlement: 'ledger.typeConvertSettlement',
  earn_subscribe: 'ledger.typeEarnSubscribe',
  earn_redeem: 'ledger.typeEarnRedeem',
  new_coin_subscription_payment: 'ledger.typeNewCoinSubscriptionPayment',
  new_coin_subscription_lock: 'ledger.typeNewCoinSubscriptionLock',
  new_coin_distribution_lock: 'ledger.typeNewCoinDistributionLock',
  new_coin_purchase_payment: 'ledger.typeNewCoinPurchasePayment',
  new_coin_purchase_lock: 'ledger.typeNewCoinPurchaseLock',
  new_coin_unlock_release: 'ledger.typeNewCoinUnlockRelease',
  loan_collateral_freeze: 'ledger.typeLoanCollateralFreeze',
  loan_collateral_release: 'ledger.typeLoanCollateralRelease',
  loan_disbursement: 'ledger.typeLoanDisbursement',
  loan_repayment: 'ledger.typeLoanRepayment',
  prediction_stake_freeze: 'ledger.typePredictionStakeFreeze',
  prediction_fee: 'ledger.typePredictionFee',
  prediction_settle_win: 'ledger.typePredictionSettleWin',
  prediction_settle_loss: 'ledger.typePredictionSettleLoss',
  prediction_payout: 'ledger.typePredictionPayout',
  prediction_stake_refund: 'ledger.typePredictionStakeRefund',
  prediction_fee_refund: 'ledger.typePredictionFeeRefund',
  agent_commission_payout: 'ledger.typeAgentCommissionPayout',
} as const satisfies Record<string, string>

export const WALLET_LEDGER_KNOWN_CHANGE_TYPES = Object.freeze(
  Object.keys(CHANGE_TYPE_TRANSLATION_KEYS),
)

export function isWalletLedgerCategory(value: unknown): value is WalletLedgerCategory {
  return typeof value === 'string'
    && (WALLET_LEDGER_CATEGORIES as readonly string[]).includes(value)
}

export function isWalletLedgerAccountType(value: unknown): value is WalletLedgerAccountType {
  return typeof value === 'string'
    && (WALLET_LEDGER_ACCOUNT_TYPES as readonly string[]).includes(value)
}

export function isWalletLedgerAccountFilter(value: unknown): value is WalletLedgerAccountFilter {
  return typeof value === 'string'
    && (WALLET_LEDGER_ACCOUNT_FILTERS as readonly string[]).includes(value)
}

export function isWalletLedgerDirection(value: unknown): value is WalletLedgerDirection {
  return typeof value === 'string'
    && (WALLET_LEDGER_DIRECTIONS as readonly string[]).includes(value)
}

export function isWalletLedgerDatePreset(value: unknown): value is WalletLedgerDatePreset {
  return typeof value === 'string'
    && (WALLET_LEDGER_DATE_PRESETS as readonly string[]).includes(value)
}

export function walletLedgerCategoryTranslationKey(filter: WalletLedgerFilter): string {
  return CATEGORY_TRANSLATION_KEYS[filter]
}

export function walletLedgerAccountTranslationKey(
  accountType: WalletLedgerAccountFilter,
): string {
  return ACCOUNT_TRANSLATION_KEYS[accountType]
}

export function walletLedgerDirectionTranslationKey(
  direction: WalletLedgerDirection,
): string {
  return DIRECTION_TRANSLATION_KEYS[direction]
}

export function walletLedgerDatePresetTranslationKey(
  preset: WalletLedgerDatePreset,
): string {
  return DATE_PRESET_TRANSLATION_KEYS[preset]
}

export function normalizeWalletLedgerAssetSymbol(value: unknown): string {
  try {
    return normalizeRealizedReturnAssetSymbol(value, 'asset_symbol', 'wallet ledger')
  } catch {
    throw new WalletLedgerContractError('invalid wallet ledger asset_symbol')
  }
}

export function walletLedgerDateRange(
  preset: WalletLedgerDatePreset,
  now = new Date(),
): WalletLedgerDateRange {
  if (!isWalletLedgerDatePreset(preset) || Number.isNaN(now.getTime())) {
    throw new WalletLedgerContractError('invalid wallet ledger date preset')
  }
  if (preset === 'all') return {}

  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  if (preset === 'last7Days') start.setDate(start.getDate() - 6)
  if (preset === 'last30Days') start.setDate(start.getDate() - 29)
  return {
    startTime: start.toISOString(),
    endTime: now.toISOString(),
  }
}

/**
 * Build the exact transport query and encode UTC boundaries as MySQL-safe
 * `YYYY-MM-DD HH:mm:ss.SSS` text. The backend also parses into a typed UTC
 * value, so neither layer relies on MySQL accepting an RFC3339 `Z` suffix.
 */
export function createWalletLedgerRequestParams(
  options: WalletLedgerRequestOptions = {},
): WalletLedgerRequestParams {
  const limit = options.limit ?? 30
  const offset = options.offset ?? 0
  if (!Number.isSafeInteger(limit) || limit < 1) {
    throw new WalletLedgerContractError('invalid wallet ledger limit')
  }
  if (!Number.isSafeInteger(offset) || offset < 0) {
    throw new WalletLedgerContractError('invalid wallet ledger offset')
  }
  if (options.category !== undefined && !isWalletLedgerCategory(options.category)) {
    throw new WalletLedgerContractError('invalid wallet ledger category')
  }

  const accountType = options.accountType ?? 'all'
  if (!isWalletLedgerAccountFilter(accountType)) {
    throw new WalletLedgerContractError('invalid wallet ledger account type')
  }
  const assetSymbol = options.assetSymbol === undefined
    ? undefined
    : normalizeWalletLedgerAssetSymbol(options.assetSymbol)
  const direction = options.direction ?? 'all'
  if (!isWalletLedgerDirection(direction)) {
    throw new WalletLedgerContractError('invalid wallet ledger direction')
  }
  const start = normalizeWalletLedgerRequestTime(options.startTime, 'start_time')
  const end = normalizeWalletLedgerRequestTime(options.endTime, 'end_time')
  if (start && end && start.timestamp > end.timestamp) {
    throw new WalletLedgerContractError('invalid wallet ledger date range')
  }

  return {
    limit,
    offset,
    category: options.category,
    account_type: accountType,
    change_type: options.changeType?.trim() || undefined,
    asset_symbol: assetSymbol,
    direction,
    start_time: start?.mysqlText,
    end_time: end?.mysqlText,
  }
}

export function walletLedgerEntryIdentity(
  entry: Pick<WalletLedgerEntry, 'accountType' | 'id'>,
): string {
  return `${entry.accountType}:${entry.id}`
}

export function mergeWalletLedgerEntries(
  current: readonly WalletLedgerEntry[],
  incoming: readonly WalletLedgerEntry[],
): WalletLedgerEntry[] {
  const byIdentity = new Map(current.map((entry) => [walletLedgerEntryIdentity(entry), entry]))
  for (const entry of incoming) byIdentity.set(walletLedgerEntryIdentity(entry), entry)
  return [...byIdentity.values()]
}

export function walletLedgerAmountSign(amount: DecimalText): '+' | '' {
  return decimalSign(amount) > 0 ? '+' : ''
}

export function isWalletLedgerContractError(error: unknown): error is WalletLedgerContractError {
  return error instanceof WalletLedgerContractError
}

export function walletLedgerTypePresentation(changeType: string): WalletLedgerTypePresentation {
  const source = changeType.trim()
  const translationKey = CHANGE_TYPE_TRANSLATION_KEYS[
    source as keyof typeof CHANGE_TYPE_TRANSLATION_KEYS
  ]
  return translationKey
    ? { translationKey }
    : { translationKey: 'ledger.typeOther', source }
}

export function mapWalletLedgerResponse(payload: BackendWalletLedgerResponse): WalletLedgerPage {
  try {
    return mapWalletLedgerResponseUnchecked(payload)
  } catch (error) {
    if (isWalletLedgerContractError(error)) throw error
    throw new WalletLedgerContractError(
      error instanceof Error && error.message
        ? error.message
        : 'invalid wallet ledger response',
    )
  }
}

export function advanceWalletLedgerPagination(
  requestOffset: number,
  result: WalletLedgerPage,
): WalletLedgerPageProgress {
  if (!Number.isSafeInteger(requestOffset) || requestOffset < 0) {
    throw new WalletLedgerContractError('invalid wallet ledger request offset')
  }

  const nextOffset = requestOffset + result.entries.length
  return {
    nextOffset,
    exhausted: result.entries.length === 0
      || nextOffset >= result.page.totalElements
      || result.page.number + 1 >= result.page.totalPages,
  }
}

function mapWalletLedgerResponseUnchecked(payload: BackendWalletLedgerResponse): WalletLedgerPage {
  if (!payload || typeof payload !== 'object') {
    throw new WalletLedgerContractError('invalid wallet ledger response')
  }
  if (!Array.isArray(payload.entries)) {
    throw new WalletLedgerContractError('invalid wallet ledger entries')
  }
  if (!payload.page || typeof payload.page !== 'object' || Array.isArray(payload.page)) {
    throw new WalletLedgerContractError('invalid wallet ledger page')
  }

  const page = payload.page as BackendWalletLedgerPage
  const mappedPage = {
    number: requiredInteger(page.number, 'page.number', 0),
    size: requiredInteger(page.size, 'page.size', 1),
    totalElements: requiredInteger(page.total_elements, 'page.total_elements', 0),
    totalPages: requiredInteger(page.total_pages, 'page.total_pages', 1),
  }
  if (payload.entries.length > mappedPage.size) {
    throw new WalletLedgerContractError('invalid wallet ledger page entries')
  }
  if (payload.entries.length > mappedPage.totalElements) {
    throw new WalletLedgerContractError('invalid wallet ledger page total_elements')
  }
  const expectedTotalPages = Math.max(1, Math.ceil(mappedPage.totalElements / mappedPage.size))
  if (mappedPage.totalPages !== expectedTotalPages) {
    throw new WalletLedgerContractError('invalid wallet ledger page total_pages')
  }

  return {
    entries: payload.entries.map((entry, index) => mapWalletLedgerEntry(entry, index)),
    page: mappedPage,
  }
}

export function groupWalletLedgerEntries(
  entries: readonly WalletLedgerEntry[],
  now = new Date(),
): WalletLedgerDateGroup[] {
  const todayKey = localCalendarKey(now)
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1)
  const yesterdayKey = localCalendarKey(yesterday)
  const groups = new Map<string, WalletLedgerDateGroup>()

  for (const entry of [...entries].sort(compareWalletLedgerEntries)) {
    const date = new Date(entry.createdAt)
    const key = localCalendarKey(date)
    const existing = groups.get(key)
    if (existing) {
      existing.entries.push(entry)
      continue
    }
    groups.set(key, {
      key,
      relation: key === todayKey ? 'today' : key === yesterdayKey ? 'yesterday' : 'date',
      date: new Date(date.getFullYear(), date.getMonth(), date.getDate()),
      entries: [entry],
    })
  }

  return [...groups.values()]
}

export function formatWalletLedgerGroupHeading(
  group: WalletLedgerDateGroup,
  locale: string,
  labels: { today: string; yesterday: string },
): string {
  if (group.relation === 'today') return labels.today
  if (group.relation === 'yesterday') return labels.yesterday
  return new Intl.DateTimeFormat(locale, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    weekday: 'short',
  }).format(group.date)
}

export function formatWalletLedgerTime(timestamp: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(new Date(timestamp))
}

export function formatWalletLedgerDecimal(
  value: DecimalText,
  locale: string,
  precisionScale = WALLET_LEDGER_MAX_FRACTION_DIGITS,
): string {
  if (!Number.isSafeInteger(precisionScale) || precisionScale < 0 || precisionScale > 18) {
    throw new WalletLedgerContractError('invalid wallet ledger display precision')
  }
  try {
    return formatDecimalText(value, locale, {
      maximumFractionDigits: precisionScale,
      preserveNonZero: false,
    })
  } catch {
    throw new WalletLedgerContractError('invalid wallet ledger display amount')
  }
}

export function createWalletLedgerRequestLifecycle(input: {
  sessionKey: () => string
  sessionGeneration: () => number
  selectedAssetSymbol: () => string | undefined
  selectedDirection: () => WalletLedgerDirection
  selectedDatePreset: () => WalletLedgerDatePreset
  selectedDateRange: () => WalletLedgerDateRange
  fetchPage: (options: WalletLedgerFetchOptions) => Promise<WalletLedgerPage>
}): WalletLedgerRequestLifecycle {
  let requestVersion = 0
  let active = true

  return {
    async load(offset: number, limit: number): Promise<WalletLedgerRequestResult> {
      const version = ++requestVersion
      if (!active) return { state: 'stale' }
      const sessionKey = input.sessionKey()
      if (!sessionKey) return { state: 'guest' }
      const sessionGeneration = input.sessionGeneration()
      const assetSymbol = input.selectedAssetSymbol()
      const direction = input.selectedDirection()
      const datePreset = input.selectedDatePreset()
      const selectedRange = input.selectedDateRange()
      const startTime = selectedRange.startTime
      const endTime = selectedRange.endTime

      try {
        const value = await input.fetchPage({
          offset,
          limit,
          ...(assetSymbol ? { assetSymbol } : {}),
          direction,
          ...(startTime ? { startTime } : {}),
          ...(endTime ? { endTime } : {}),
        })
        if (!active
          || version !== requestVersion
          || input.sessionKey() !== sessionKey
          || input.sessionGeneration() !== sessionGeneration
          || input.selectedAssetSymbol() !== assetSymbol
          || input.selectedDirection() !== direction
          || input.selectedDatePreset() !== datePreset
          || !sameWalletLedgerDateRange(input.selectedDateRange(), { startTime, endTime })) {
          return { state: 'stale' }
        }
        assertWalletLedgerFilteredPage(value, { assetSymbol, direction, startTime, endTime })
        return { state: 'loaded', assetSymbol, direction, datePreset, value }
      } catch (error) {
        if (!active
          || version !== requestVersion
          || input.sessionKey() !== sessionKey
          || input.sessionGeneration() !== sessionGeneration
          || input.selectedAssetSymbol() !== assetSymbol
          || input.selectedDirection() !== direction
          || input.selectedDatePreset() !== datePreset
          || !sameWalletLedgerDateRange(input.selectedDateRange(), { startTime, endTime })) {
          return { state: 'stale' }
        }
        return { state: 'error', assetSymbol, direction, datePreset, error }
      }
    },
    invalidate(): void {
      requestVersion += 1
    },
    stop(): void {
      active = false
      requestVersion += 1
    },
  }
}

export function createWalletLedgerPaginationState(): WalletLedgerPaginationState {
  return {
    entries: [],
    loading: false,
    loadingMore: false,
    nextOffset: 0,
    exhausted: false,
    initialError: null,
    appendError: null,
  }
}

/**
 * Own initial and append state as one executable lifecycle. Append failures
 * retain the loaded snapshot and offset, retries reuse that exact offset, and
 * stale session/filter responses never toggle state owned by a newer request.
 */
export function createWalletLedgerPaginationController(input: {
  sessionKey: () => string
  sessionGeneration: () => number
  selectedAssetSymbol: () => string | undefined
  selectedDirection: () => WalletLedgerDirection
  selectedDatePreset: () => WalletLedgerDatePreset
  selectedDateRange: () => WalletLedgerDateRange
  fetchPage: (options: WalletLedgerFetchOptions) => Promise<WalletLedgerPage>
  pageSize?: number
  onChange?: (state: WalletLedgerPaginationState) => void
}): WalletLedgerPaginationController {
  const pageSize = input.pageSize ?? 30
  if (!Number.isSafeInteger(pageSize) || pageSize < 1 || pageSize > 100) {
    throw new WalletLedgerContractError('invalid wallet ledger page size')
  }
  const requests = createWalletLedgerRequestLifecycle(input)
  let state = createWalletLedgerPaginationState()
  let active = true

  const snapshot = (): WalletLedgerPaginationState => ({
    ...state,
    entries: [...state.entries],
  })
  const replaceState = (next: WalletLedgerPaginationState): void => {
    state = next
    input.onChange?.(snapshot())
  }

  async function loadInitial(): Promise<WalletLedgerPaginationOperation> {
    if (!active) return 'stale'
    if (!input.sessionKey()) {
      requests.invalidate()
      replaceState(createWalletLedgerPaginationState())
      return 'guest'
    }
    replaceState({
      ...createWalletLedgerPaginationState(),
      loading: true,
    })

    const result = await requests.load(0, pageSize)
    if (result.state === 'stale') return 'stale'
    if (result.state === 'guest') {
      replaceState(createWalletLedgerPaginationState())
      return 'guest'
    }
    if (result.state === 'error') {
      replaceState({
        ...createWalletLedgerPaginationState(),
        initialError: result.error,
      })
      return 'error'
    }

    const entries = mergeWalletLedgerEntries([], result.value.entries)
    const pagination = advanceWalletLedgerPagination(0, result.value)
    replaceState({
      ...createWalletLedgerPaginationState(),
      entries,
      nextOffset: pagination.nextOffset,
      exhausted: pagination.exhausted,
    })
    return 'loaded'
  }

  async function append(retry: boolean): Promise<WalletLedgerPaginationOperation> {
    if (!active) return 'stale'
    if (!input.sessionKey()) {
      requests.invalidate()
      replaceState(createWalletLedgerPaginationState())
      return 'guest'
    }
    if (state.loading
      || state.loadingMore
      || state.exhausted
      || state.entries.length === 0
      || (retry ? state.appendError === null : state.appendError !== null)) {
      return 'ignored'
    }

    const requestState = state
    const offset = state.nextOffset
    replaceState({ ...state, loadingMore: true })
    const result = await requests.load(offset, pageSize)
    if (result.state === 'stale') return 'stale'
    if (result.state === 'guest') {
      replaceState(createWalletLedgerPaginationState())
      return 'guest'
    }
    if (result.state === 'error') {
      replaceState({
        ...requestState,
        loadingMore: false,
        appendError: result.error,
      })
      return 'error'
    }

    const entries = mergeWalletLedgerEntries(requestState.entries, result.value.entries)
    const pagination = advanceWalletLedgerPagination(offset, result.value)
    const madeProgress = entries.length > requestState.entries.length
    replaceState({
      ...requestState,
      entries,
      loadingMore: false,
      nextOffset: pagination.nextOffset,
      exhausted: pagination.exhausted || (result.value.entries.length > 0 && !madeProgress),
      initialError: null,
      appendError: null,
    })
    return 'loaded'
  }

  return {
    snapshot,
    loadInitial,
    loadMore: () => append(false),
    retryLoadMore: () => append(true),
    reset(): void {
      requests.invalidate()
      replaceState(createWalletLedgerPaginationState())
    },
    stop(): void {
      active = false
      requests.stop()
      replaceState(createWalletLedgerPaginationState())
    },
  }
}

function mapWalletLedgerEntry(value: unknown, index: number): WalletLedgerEntry {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new WalletLedgerContractError(`invalid wallet ledger entry ${index}`)
  }
  const entry = value as BackendWalletLedgerEntry
  const changeType = requiredText(entry.change_type, `entries[${index}].change_type`)
  if (!isWalletLedgerAccountType(entry.account_type)) {
    throw new WalletLedgerContractError(`invalid wallet ledger entries[${index}].account_type`)
  }
  if (!isWalletLedgerCategory(entry.category)) {
    throw new WalletLedgerContractError(`invalid wallet ledger entries[${index}].category`)
  }
  const fee = requiredRealizedReturnDecimal(
    entry.fee,
    `entries[${index}].fee`,
    'wallet ledger',
  )
  if (decimalCompare(fee, normalizeDecimalText('0')) < 0) {
    throw new WalletLedgerContractError(`invalid wallet ledger entries[${index}].fee`)
  }
  const createdAt = normalizeRealizedReturnTimestamp(
    entry.created_at,
    `entries[${index}].created_at`,
    'wallet ledger',
  )
  if (Number.isNaN(new Date(createdAt).getTime())) {
    throw new WalletLedgerContractError(`invalid wallet ledger entries[${index}].created_at`)
  }

  const amount = requiredRealizedReturnDecimal(
    entry.amount,
    `entries[${index}].amount`,
    'wallet ledger',
  )
  const balanceAfter = requiredRealizedReturnDecimal(
    entry.balance_after,
    `entries[${index}].balance_after`,
    'wallet ledger',
  )
  const precisionScale = requiredPrecisionScale(
    entry.precision_scale,
    `entries[${index}].precision_scale`,
  )

  return {
    id: requiredInteger(entry.id, `entries[${index}].id`, 1),
    accountType: entry.account_type,
    symbol: normalizeWalletLedgerAssetSymbol(entry.symbol),
    changeType,
    category: entry.category,
    amount,
    fee,
    balanceAfter,
    precisionScale,
    createdAt,
  }
}

function sameWalletLedgerDateRange(
  left: WalletLedgerDateRange,
  right: WalletLedgerDateRange,
): boolean {
  return left.startTime === right.startTime && left.endTime === right.endTime
}

function normalizeWalletLedgerRequestTime(
  value: string | undefined,
  field: string,
): { timestamp: number; mysqlText: string } | undefined {
  if (value === undefined) return undefined
  const normalized = value.trim()
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/.exec(normalized)
  if (!match) {
    throw new WalletLedgerContractError(`invalid wallet ledger ${field}`)
  }
  const [year, month, day, hour, minute, second] = match.slice(1).map(Number)
  const wallClock = new Date(Date.UTC(year, month - 1, day, hour, minute, second))
  if (year < 1000
    || wallClock.getUTCFullYear() !== year
    || wallClock.getUTCMonth() !== month - 1
    || wallClock.getUTCDate() !== day
    || wallClock.getUTCHours() !== hour
    || wallClock.getUTCMinutes() !== minute
    || wallClock.getUTCSeconds() !== second) {
    throw new WalletLedgerContractError(`invalid wallet ledger ${field}`)
  }
  const timestamp = Date.parse(normalized)
  if (!Number.isFinite(timestamp)) {
    throw new WalletLedgerContractError(`invalid wallet ledger ${field}`)
  }
  return {
    timestamp,
    mysqlText: new Date(timestamp).toISOString().slice(0, 23).replace('T', ' '),
  }
}

function assertWalletLedgerFilteredPage(
  value: WalletLedgerPage,
  filter: {
    assetSymbol?: string
    direction: WalletLedgerDirection
    startTime?: string
    endTime?: string
  },
): void {
  if (filter.assetSymbol
    && value.entries.some((entry) => entry.symbol !== filter.assetSymbol)) {
    throw new WalletLedgerContractError('invalid wallet ledger filtered asset')
  }
  if (filter.direction === 'credit'
    && value.entries.some((entry) => decimalSign(entry.amount) <= 0)) {
    throw new WalletLedgerContractError('invalid wallet ledger filtered direction')
  }
  if (filter.direction === 'debit'
    && value.entries.some((entry) => decimalSign(entry.amount) >= 0)) {
    throw new WalletLedgerContractError('invalid wallet ledger filtered direction')
  }

  const start = filter.startTime ? Date.parse(filter.startTime) : undefined
  const end = filter.endTime ? Date.parse(filter.endTime) : undefined
  if ((start !== undefined && !Number.isFinite(start))
    || (end !== undefined && !Number.isFinite(end))
    || (start !== undefined && end !== undefined && start > end)) {
    throw new WalletLedgerContractError('invalid wallet ledger filtered date')
  }
  if (value.entries.some((entry) => (
    (start !== undefined && entry.createdAt < start)
    || (end !== undefined && entry.createdAt > end)
  ))) {
    throw new WalletLedgerContractError('invalid wallet ledger filtered date')
  }
}

function requiredPrecisionScale(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > 18) {
    throw new WalletLedgerContractError(`invalid wallet ledger ${field}`)
  }
  return value
}

function requiredInteger(value: unknown, field: string, minimum: number): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < minimum) {
    throw new WalletLedgerContractError(`invalid wallet ledger ${field}`)
  }
  return value
}

function requiredText(value: unknown, field: string): string {
  if (typeof value !== 'string' || !value.trim()) {
    throw new WalletLedgerContractError(`invalid wallet ledger ${field}`)
  }
  return value.trim()
}

function compareWalletLedgerEntries(left: WalletLedgerEntry, right: WalletLedgerEntry): number {
  const createdAtOrder = right.createdAt - left.createdAt
  if (createdAtOrder) return createdAtOrder
  if (left.accountType !== right.accountType) {
    return left.accountType < right.accountType ? -1 : 1
  }
  return right.id - left.id
}

function localCalendarKey(date: Date): string {
  const year = String(date.getFullYear()).padStart(4, '0')
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}
