import {
  normalizeRealizedReturnAssetSymbol,
  normalizeRealizedReturnTimestamp,
  requiredRealizedReturnNumber,
} from './realizedReturn.ts'

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
export const WALLET_LEDGER_MAX_FRACTION_DIGITS = 8

export type WalletLedgerCategory = typeof WALLET_LEDGER_CATEGORIES[number]
export type WalletLedgerFilter = typeof WALLET_LEDGER_FILTERS[number]

export interface BackendWalletLedgerEntry {
  id: unknown
  symbol: unknown
  change_type: unknown
  category: unknown
  amount: unknown
  fee: unknown
  balance_after: unknown
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
  symbol: string
  changeType: string
  category: WalletLedgerCategory
  amount: number
  fee: number
  balanceAfter: number
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

export interface WalletLedgerPaginationState {
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
  category?: WalletLedgerCategory
}

export type WalletLedgerRequestResult =
  | { state: 'guest' }
  | { state: 'loaded'; filter: WalletLedgerFilter; value: WalletLedgerPage }
  | { state: 'error'; filter: WalletLedgerFilter; error: unknown }
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

export function walletLedgerCategoryTranslationKey(filter: WalletLedgerFilter): string {
  return CATEGORY_TRANSLATION_KEYS[filter]
}

export function walletLedgerAmountSign(amount: number): '+' | '' {
  return amount > 0 ? '+' : ''
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
): WalletLedgerPaginationState {
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

export function formatWalletLedgerDecimal(value: number, locale: string): string {
  if (!Number.isFinite(value)) {
    throw new WalletLedgerContractError('invalid wallet ledger display amount')
  }
  return new Intl.NumberFormat(locale, {
    maximumFractionDigits: WALLET_LEDGER_MAX_FRACTION_DIGITS,
  }).format(Object.is(value, -0) ? 0 : value)
}

export function createWalletLedgerRequestLifecycle(input: {
  sessionKey: () => string
  selectedFilter: () => WalletLedgerFilter
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
      const filter = input.selectedFilter()
      const category = filter === 'all' ? undefined : filter

      try {
        const value = await input.fetchPage({ offset, limit, category })
        if (category && value.entries.some((entry) => entry.category !== category)) {
          throw new WalletLedgerContractError('invalid wallet ledger filtered category')
        }
        if (!active
          || version !== requestVersion
          || input.sessionKey() !== sessionKey
          || input.selectedFilter() !== filter) {
          return { state: 'stale' }
        }
        return { state: 'loaded', filter, value }
      } catch (error) {
        if (!active
          || version !== requestVersion
          || input.sessionKey() !== sessionKey
          || input.selectedFilter() !== filter) {
          return { state: 'stale' }
        }
        return { state: 'error', filter, error }
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

function mapWalletLedgerEntry(value: unknown, index: number): WalletLedgerEntry {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new WalletLedgerContractError(`invalid wallet ledger entry ${index}`)
  }
  const entry = value as BackendWalletLedgerEntry
  const changeType = requiredText(entry.change_type, `entries[${index}].change_type`)
  if (!isWalletLedgerCategory(entry.category)) {
    throw new WalletLedgerContractError(`invalid wallet ledger entries[${index}].category`)
  }
  const fee = requiredRealizedReturnNumber(
    entry.fee,
    `entries[${index}].fee`,
    'wallet ledger',
  )
  if (fee < 0) {
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

  return {
    id: requiredInteger(entry.id, `entries[${index}].id`, 1),
    symbol: normalizeRealizedReturnAssetSymbol(
      entry.symbol,
      `entries[${index}].symbol`,
      'wallet ledger',
    ),
    changeType,
    category: entry.category,
    amount: requiredRealizedReturnNumber(
      entry.amount,
      `entries[${index}].amount`,
      'wallet ledger',
    ),
    fee,
    balanceAfter: requiredRealizedReturnNumber(
      entry.balance_after,
      `entries[${index}].balance_after`,
      'wallet ledger',
    ),
    createdAt,
  }
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
  return right.createdAt - left.createdAt || right.id - left.id
}

function localCalendarKey(date: Date): string {
  const year = String(date.getFullYear()).padStart(4, '0')
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}
