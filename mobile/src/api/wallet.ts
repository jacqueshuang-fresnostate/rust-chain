import { client, readAuthSessionSnapshot, requestUrl } from './client'
import { canonicalRequestIntent, RetryStableIdempotencyKeys } from './idempotency'
import {
  createReferenceRequestKey,
  referenceRequestRegistry,
  type ReferenceRequestOptions,
} from './requestCache'
import { asNumber } from '@/core/format'
import {
  mapTodayReturn,
  type BackendTodayReturn,
  type TodayReturn,
} from '@/core/todayReturn'
import {
  isReturnHistoryPeriod,
  mapReturnHistory,
  type BackendReturnHistory,
  type ReturnHistory,
  type ReturnHistoryPeriodDays,
} from '@/core/returnHistory'
import {
  createWalletLedgerRequestParams,
  mapWalletLedgerResponse,
  type BackendWalletLedgerResponse,
  type WalletLedgerPage,
  type WalletLedgerRequestOptions,
} from '@/core/walletLedger'

import type { DepositAddress, DepositAsset, DepositNetwork, WalletAccount } from '@/core/types'
import {
  isWithdrawalDecimalString,
  withdrawalQuoteAmountsAreConsistent,
  type WithdrawalFeeTier,
} from '@/core/withdrawalQuote'
import {
  decimalCompare,
  normalizeDecimalText,
  requiredDecimalText,
  type DecimalText,
} from '@/core/decimal'

const WALLET_DECIMAL_CONSTRAINTS = {
  allowNegative: false,
  maxIntegerDigits: 20,
  maxScale: 18,
} as const

export class WalletFinancialContractError extends TypeError {
  constructor(field: string) {
    super(`invalid wallet financial ${field}`)
    this.name = 'WalletFinancialContractError'
  }
}

const walletTransferIdempotencyKeys = new RetryStableIdempotencyKeys('mobile-transfer')

export {
  calculateWithdrawalFee,
  calculateWithdrawalFeeText,
  maximumQuotedWithdrawalAmount,
  maximumQuotedWithdrawalAmountText,
  normalizeWithdrawalPreviewAmount,
  normalizeWithdrawalPreviewAmountText,
} from '@/core/withdrawalQuote'
export type { WithdrawalFeeTier } from '@/core/withdrawalQuote'

export {
  createTodayReturnRequestLifecycle,
  isCompleteTodayReturn,
  mapTodayReturn,
} from '@/core/todayReturn'
export type { TodayReturn, TodayReturnStatus } from '@/core/todayReturn'
export {
  createReturnHistoryRequestLifecycle,
  isReturnHistoryPeriod,
  mapReturnHistory,
  RETURN_HISTORY_PERIODS,
} from '@/core/returnHistory'
export type {
  ReturnHistory,
  ReturnHistoryPeriodDays,
  ReturnHistoryStatus,
  ReturnHistoryViewState,
} from '@/core/returnHistory'
export {
  advanceWalletLedgerPagination,
  createWalletLedgerPaginationController,
  createWalletLedgerPaginationState,
  createWalletLedgerRequestLifecycle,
  createWalletLedgerRequestParams,
  formatWalletLedgerDecimal,
  formatWalletLedgerGroupHeading,
  formatWalletLedgerTime,
  groupWalletLedgerEntries,
  isWalletLedgerAccountFilter,
  isWalletLedgerAccountType,
  isWalletLedgerContractError,
  isWalletLedgerCategory,
  isWalletLedgerDatePreset,
  isWalletLedgerDirection,
  mapWalletLedgerResponse,
  mergeWalletLedgerEntries,
  normalizeWalletLedgerAssetSymbol,
  WALLET_LEDGER_ACCOUNT_FILTERS,
  WALLET_LEDGER_ACCOUNT_TYPES,
  WALLET_LEDGER_CATEGORIES,
  WALLET_LEDGER_DATE_PRESETS,
  WALLET_LEDGER_DIRECTIONS,
  WALLET_LEDGER_FILTERS,
  WALLET_LEDGER_KNOWN_CHANGE_TYPES,
  WALLET_LEDGER_MAX_FRACTION_DIGITS,
  walletLedgerAmountSign,
  walletLedgerAccountTranslationKey,
  walletLedgerCategoryTranslationKey,
  walletLedgerDatePresetTranslationKey,
  walletLedgerDateRange,
  walletLedgerDirectionTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerTypePresentation,
  WalletLedgerContractError,
} from '@/core/walletLedger'
export type {
  WalletLedgerAccountFilter,
  WalletLedgerAccountType,
  WalletLedgerCategory,
  WalletLedgerDateGroup,
  WalletLedgerDatePreset,
  WalletLedgerDateRange,
  WalletLedgerDirection,
  WalletLedgerEntry,
  WalletLedgerFilter,
  WalletLedgerPage,
  WalletLedgerPageProgress,
  WalletLedgerPaginationController,
  WalletLedgerPaginationOperation,
  WalletLedgerPaginationState,
  WalletLedgerRequestOptions,
  WalletLedgerRequestParams,
  WalletLedgerRequestLifecycle,
  WalletLedgerRequestResult,
} from '@/core/walletLedger'

export interface WalletDepositAsset extends DepositAsset {
  minDepositAmountText: DecimalText
  depositFee: number
  depositFeeText: DecimalText
}

export interface StrictWithdrawalFeeTier extends WithdrawalFeeTier {
  minAmountText: DecimalText
  maxAmountText?: DecimalText
  feeRatePercentText: DecimalText
}

export interface StrictWalletAccount extends WalletAccount {
  availableText: DecimalText
  frozenText: DecimalText
  lockedText: DecimalText
}

export interface WithdrawalAsset extends WalletDepositAsset {
  withdrawEnabled: boolean
  withdrawFee: number
  withdrawFeeText: DecimalText
  precisionScale: number
  withdrawFeeTiers: StrictWithdrawalFeeTier[]
}

export interface WithdrawalQuote {
  quoteId: string
  assetSymbol: string
  network: string
  amount: DecimalText
  fee: DecimalText
  net: DecimalText
  totalReserved: DecimalText
  feeConfigVersion: string
  expiresAt: number
}

export interface WithdrawalSubmission extends WithdrawalQuote {
  id: number
  status: string
  securityMethod: string
}

export interface WithdrawalRecord {
  id: number
  assetSymbol: string
  network?: string
  address: string
  amount: number
  fee: number
  amountText: DecimalText
  feeText: DecimalText
  status: string
  txHash?: string
  failureReason?: string
  reviewReason?: string
  createdAt: number
}

export interface QuickRechargeConfig {
  enabled: boolean
  currency: string
  token: string
  network: string
  minAmount: number
  maxAmount?: number
  minAmountText: DecimalText
  maxAmountText?: DecimalText
}

export interface QuickRechargeOrder {
  id: number
  orderId: string
  assetSymbol: string
  currency: string
  token: string
  network: string
  fiatAmount: number
  actualAmount?: number
  fiatAmountText: DecimalText
  actualAmountText?: DecimalText
  paymentUrl?: string
  redirectUrl?: string
  status: string
  createdAt?: number
}

interface BackendDepositAsset {
  symbol: string
  name?: string | null
  logo_url?: string | null
  deposit_enabled?: boolean | null
  min_deposit_amount?: string | number | null
  deposit_fee?: string | number | null
  withdraw_enabled?: boolean | null
  withdraw_fee?: string | number | null
  precision_scale?: number | null
  withdraw_fee_tiers?: BackendWithdrawalFeeTier[] | null
}

interface BackendWithdrawalFeeTier {
  min_amount?: string | number | null
  max_amount?: string | number | null
  fee_rate_percent?: string | number | null
}

interface BackendWithdrawalQuote {
  quote_id?: string
  asset_symbol?: string
  network?: string
  amount?: string | number
  fee?: string | number
  net?: string | number
  total_reserved?: string | number
  fee_config_version?: string
  expires_at?: number
}

interface BackendWithdrawalSubmission extends BackendWithdrawalQuote {
  id?: number
  status?: string
  security_method?: string
}

interface BackendDepositNetwork {
  network: string
  display_name?: string | null
}

interface BackendDepositAddress {
  asset_symbol: string
  network: string
  address: string
  memo?: string | null
}

interface BackendWalletAccount {
  asset_id?: number
  symbol: string
  logo_url?: string | null
  available: string | number
  frozen: string | number
  locked: string | number
}

interface BackendWalletTransferAccount {
  asset_id: number
  available: string | number
  frozen: string | number
  locked: string | number
}

interface BackendWalletTransferResponse {
  transfer_id: string
  spot_wallet: BackendWalletTransferAccount
  margin_wallet: BackendWalletTransferAccount
}

export interface WalletTransferResult {
  transferId: string
  spotWallet: StrictWalletAccount
  marginWallet: StrictWalletAccount
}

export async function fetchDepositAssets(options: ReferenceRequestOptions = {}): Promise<WalletDepositAsset[]> {
  const url = requestUrl('/wallet/deposit-assets')
  return referenceRequestRegistry.request(walletReferenceKey(url), 30_000, async () => {
    const response = await client.get<{ assets?: BackendDepositAsset[] }>(url)
    return (response.data.assets || [])
      .map(mapDepositAsset)
      .filter((asset) => asset.depositEnabled)
  }, options)
}

export async function fetchWithdrawalAssets(options: ReferenceRequestOptions = {}): Promise<WithdrawalAsset[]> {
  const url = requestUrl('/wallet/withdraw-assets')
  return referenceRequestRegistry.request(walletReferenceKey(url), 30_000, async () => {
    const response = await client.get<{ assets?: BackendDepositAsset[] }>(url)
    return (response.data.assets || [])
      .map((asset) => {
        const deposit = mapDepositAsset(asset)
        const withdrawFeeText = walletDecimal(asset.withdraw_fee, 'withdraw_fee')
        const withdrawFeeTiers = (asset.withdraw_fee_tiers || []).map((tier, index) => {
          const minAmountText = walletDecimal(tier.min_amount, `withdraw_fee_tiers[${index}].min_amount`)
          const maxAmountText = nullableWalletDecimal(tier.max_amount, `withdraw_fee_tiers[${index}].max_amount`)
          const feeRatePercentText = walletDecimal(tier.fee_rate_percent, `withdraw_fee_tiers[${index}].fee_rate_percent`)
          return {
            minAmount: decimalDisplayNumber(minAmountText, 'withdraw tier minimum'),
            maxAmount: maxAmountText ? decimalDisplayNumber(maxAmountText, 'withdraw tier maximum') : undefined,
            feeRatePercent: decimalDisplayNumber(feeRatePercentText, 'withdraw tier rate'),
            minAmountText,
            maxAmountText: maxAmountText || undefined,
            feeRatePercentText,
          }
        }).sort((left, right) => decimalCompare(left.minAmountText, right.minAmountText))
        return {
          ...deposit,
          withdrawEnabled: asset.withdraw_enabled !== false,
          withdrawFee: decimalDisplayNumber(withdrawFeeText, 'withdraw fixed fee'),
          withdrawFeeText,
          precisionScale: walletPrecisionScale(asset.precision_scale),
          withdrawFeeTiers,
        }
      })
      .filter((asset) => asset.withdrawEnabled)
  }, options)
}

export async function fetchDepositNetworks(
  assetSymbol: string,
  minimum = 0,
  options: ReferenceRequestOptions = {},
): Promise<DepositNetwork[]> {
  const normalizedAsset = assetSymbol.toUpperCase()
  const url = requestUrl('/wallet/deposit-networks')
  return referenceRequestRegistry.request(walletReferenceKey(url, {
    asset_symbol: normalizedAsset,
    minimum,
  }), 30_000, async () => {
    const response = await client.get<{ networks?: BackendDepositNetwork[] }>(url, {
      params: { asset_symbol: normalizedAsset },
    })
    return (response.data.networks || []).map((network) => ({
      network: network.network,
      displayName: network.display_name?.trim() || network.network,
      minDepositAmount: minimum,
    }))
  }, options)
}

/** 钱包目录可能受账号或地区策略影响，因此按当前内存会话隔离，不跨 token 共享。 */
function walletReferenceKey(url: string, params: Readonly<Record<string, unknown>> = {}): string {
  return createReferenceRequestKey(url, params, `wallet:${readAuthSessionSnapshot().scope || 'guest'}`)
}

export async function createDepositAddress(assetSymbol: string, network: string, minimum = 0): Promise<DepositAddress> {
  const response = await client.post<BackendDepositAddress>(requestUrl('/wallet/deposit-address'), {
    asset_symbol: assetSymbol.toUpperCase(),
    network,
  })
  return {
    assetSymbol: response.data.asset_symbol.toUpperCase(),
    network: response.data.network,
    address: response.data.address,
    memo: response.data.memo || undefined,
    minDepositAmount: minimum,
  }
}

export async function fetchWalletAccounts(): Promise<StrictWalletAccount[]> {
  const response = await client.get<{ accounts?: BackendWalletAccount[] }>(requestUrl('/wallet/accounts'))
  if (!Array.isArray(response.data.accounts)) {
    throw new WalletFinancialContractError('wallet accounts envelope')
  }
  return response.data.accounts.map((account) => mapWalletAccount(account, account.symbol))
}

export async function fetchTodayReturn(): Promise<TodayReturn> {
  const response = await client.get<BackendTodayReturn>(requestUrl('/wallet/today-return'))
  return mapTodayReturn(response.data)
}

export async function fetchReturnHistory(
  periodDays: ReturnHistoryPeriodDays,
): Promise<ReturnHistory> {
  if (!isReturnHistoryPeriod(periodDays)) throw new Error('invalid return history period')
  const response = await client.get<BackendReturnHistory>(requestUrl('/wallet/return-history'), {
    params: { days: periodDays },
  })
  return mapReturnHistory(response.data, periodDays)
}

export async function fetchWithdrawalQuote(input: {
  assetSymbol: string
  network: string
  amount: DecimalText
}): Promise<WithdrawalQuote> {
  const response = await client.post<BackendWithdrawalQuote>(requestUrl('/wallet/withdrawals/quote'), {
    asset_symbol: input.assetSymbol.toUpperCase(),
    network: input.network,
    amount: normalizeDecimalText(input.amount),
  })
  return mapWithdrawalQuote(response.data, true)
}

export async function submitWithdrawal(input: {
  quote: WithdrawalQuote
  address: string
  fundPassword?: string
  totpCode?: string
}): Promise<WithdrawalSubmission> {
  const response = await client.post<BackendWithdrawalSubmission>(requestUrl('/wallet/withdrawals'), {
    quote_id: input.quote.quoteId,
    asset_symbol: input.quote.assetSymbol,
    network: input.quote.network,
    address: input.address.trim(),
    amount: input.quote.amount,
    fee: input.quote.fee,
    idempotency_key: createWithdrawalIdempotencyKey(input.quote.quoteId),
    fund_password: input.fundPassword?.trim() || undefined,
    totp_code: input.totpCode?.trim() || undefined,
  })
  const submitted = mapWithdrawalSubmission(response.data)
  assertWithdrawalContract(input.quote, submitted)
  return submitted
}

function mapWithdrawalQuote(raw: BackendWithdrawalQuote, requireUnexpired = false): WithdrawalQuote {
  const quote: WithdrawalQuote = {
    quoteId: String(raw.quote_id || '').trim(),
    assetSymbol: String(raw.asset_symbol || '').trim().toUpperCase(),
    network: String(raw.network || '').trim(),
    amount: decimalString(raw.amount),
    fee: decimalString(raw.fee),
    net: decimalString(raw.net),
    totalReserved: decimalString(raw.total_reserved),
    feeConfigVersion: String(raw.fee_config_version || '').trim(),
    expiresAt: typeof raw.expires_at === 'number' ? raw.expires_at : Number.NaN,
  }
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(quote.quoteId)
    || !/^[A-Z0-9]{1,32}$/.test(quote.assetSymbol)
    || !/^(?:eth|base|tron|btc|solana)$/.test(quote.network)
    || !/^[0-9a-f]{64}$/.test(quote.feeConfigVersion)
    || !Number.isSafeInteger(quote.expiresAt)
    || quote.expiresAt <= 0
    || (requireUnexpired && quote.expiresAt <= Date.now())
    || !withdrawalQuoteAmountsAreConsistent(
      quote.amount,
      quote.fee,
      quote.net,
      quote.totalReserved,
    )) {
    throw new WalletFinancialContractError('withdrawal quote')
  }
  return quote
}

function mapWithdrawalSubmission(raw: BackendWithdrawalSubmission): WithdrawalSubmission {
  const quote = mapWithdrawalQuote(raw)
  const id = asNumber(raw.id)
  const status = String(raw.status || '').trim()
  const securityMethod = String(raw.security_method || '').trim()
  if (!Number.isSafeInteger(id) || id <= 0 || !status || !securityMethod) {
    throw new Error('invalid withdrawal submission response')
  }
  return { ...quote, id, status, securityMethod }
}

function assertWithdrawalContract(quote: WithdrawalQuote, submitted: WithdrawalSubmission): void {
  if (quote.quoteId !== submitted.quoteId
    || quote.assetSymbol !== submitted.assetSymbol
    || quote.network !== submitted.network
    || quote.feeConfigVersion !== submitted.feeConfigVersion
    || quote.expiresAt !== submitted.expiresAt
    || !sameDecimal(quote.amount, submitted.amount)
    || !sameDecimal(quote.fee, submitted.fee)
    || !sameDecimal(quote.net, submitted.net)
    || !sameDecimal(quote.totalReserved, submitted.totalReserved)) {
    throw new Error('withdrawal submission does not match the authorized quote')
  }
}

function decimalString(value: string | number | undefined): DecimalText {
  if (typeof value !== 'string' || !isWithdrawalDecimalString(value.trim())) {
    throw new WalletFinancialContractError('withdrawal quote decimal')
  }
  return walletDecimal(value, 'withdrawal quote decimal')
}

function sameDecimal(left: string, right: string): boolean {
  return decimalCompare(normalizeDecimalText(left), normalizeDecimalText(right)) === 0
}

interface BackendWithdrawalRecord {
  id: number
  asset_symbol: string
  network?: string | null
  address: string
  amount: string | number
  fee: string | number
  status: string
  tx_hash?: string | null
  failure_reason?: string | null
  review_reason?: string | null
  created_at: number
}

export async function fetchWithdrawalRecords(limit = 50): Promise<WithdrawalRecord[]> {
  const response = await client.get<{ withdrawals?: BackendWithdrawalRecord[] }>(requestUrl('/wallet/withdrawals'), { params: { limit } })
  return (response.data.withdrawals || []).map((record) => {
    const amountText = walletDecimal(record.amount, 'withdrawal amount')
    const feeText = walletDecimal(record.fee, 'withdrawal fee')
    return {
      id: record.id,
      assetSymbol: record.asset_symbol.toUpperCase(),
      network: record.network || undefined,
      address: record.address,
      amount: decimalDisplayNumber(amountText, 'withdrawal amount'),
      fee: decimalDisplayNumber(feeText, 'withdrawal fee'),
      amountText,
      feeText,
      status: record.status,
      txHash: record.tx_hash || undefined,
      failureReason: record.failure_reason || undefined,
      reviewReason: record.review_reason || undefined,
      createdAt: record.created_at > 0 && record.created_at < 1_000_000_000_000 ? record.created_at * 1000 : record.created_at,
    }
  })
}

function createWithdrawalIdempotencyKey(quoteId: string): string {
  return `mobile-withdraw-${quoteId}`
}

export async function fetchWalletLedger(
  options: WalletLedgerRequestOptions = {},
): Promise<WalletLedgerPage> {
  const params = createWalletLedgerRequestParams(options)
  const response = await client.get<BackendWalletLedgerResponse>(requestUrl('/wallet/ledger'), {
    params,
  })
  return mapWalletLedgerResponse(response.data)
}

export async function fetchQuickRechargeConfig(): Promise<QuickRechargeConfig> {
  const response = await client.get<{ enabled?: boolean; currency?: string; token?: string; network?: string; min_amount?: string | number; max_amount?: string | number | null }>(requestUrl('/wallet/quick-recharge/config'))
  const minAmountText = walletDecimal(response.data.min_amount, 'quick-recharge min_amount')
  const maxAmountText = nullableWalletDecimal(response.data.max_amount, 'quick-recharge max_amount')
  return {
    enabled: Boolean(response.data.enabled),
    currency: String(response.data.currency || '').toUpperCase(),
    token: String(response.data.token || '').toUpperCase(),
    network: String(response.data.network || ''),
    minAmount: decimalDisplayNumber(minAmountText, 'quick-recharge min amount'),
    maxAmount: maxAmountText ? decimalDisplayNumber(maxAmountText, 'quick-recharge max amount') : undefined,
    minAmountText,
    maxAmountText: maxAmountText || undefined,
  }
}

export async function createQuickRechargeOrder(
  amount: DecimalText,
  returnTarget: 'ios_app' | 'android_app' | 'mobile_web' | 'desktop_web',
): Promise<QuickRechargeOrder> {
  const response = await client.post<BackendQuickRechargeOrder>(requestUrl('/wallet/quick-recharge/orders'), {
    amount: normalizeDecimalText(amount),
    return_target: returnTarget,
  })
  return mapQuickRechargeOrder(response.data)
}

interface BackendQuickRechargeOrder {
  id: number
  order_id: string
  asset_symbol?: string | null
  currency: string
  token: string
  network?: string | null
  fiat_amount: string | number
  actual_amount?: string | number | null
  payment_url?: string | null
  redirect_url?: string | null
  status: string
  created_at?: number | null
}

export async function fetchQuickRechargeOrders(limit = 20): Promise<QuickRechargeOrder[]> {
  const response = await client.get<{ orders?: BackendQuickRechargeOrder[] }>(requestUrl('/wallet/quick-recharge/orders'), { params: { limit } })
  return (response.data.orders || []).map(mapQuickRechargeOrder)
}

export async function transferWalletFunds(
  assetSymbol: string,
  from: 'spot' | 'margin',
  to: 'spot' | 'margin',
  amount: DecimalText,
): Promise<WalletTransferResult> {
  const symbol = assetSymbol.toUpperCase()
  const businessIntent = {
    asset_symbol: symbol,
    from,
    to,
    amount: normalizeDecimalText(amount),
  }
  const intent = canonicalRequestIntent(businessIntent)
  const idempotencyKey = walletTransferIdempotencyKeys.acquire(intent)
  const response = await client.post<BackendWalletTransferResponse>(requestUrl('/margin/transfers'), {
    ...businessIntent,
    idempotency_key: idempotencyKey,
  })
  walletTransferIdempotencyKeys.complete(intent, idempotencyKey)
  return {
    transferId: response.data.transfer_id,
    spotWallet: mapTransferWallet(response.data.spot_wallet, symbol),
    marginWallet: mapTransferWallet(response.data.margin_wallet, symbol),
  }
}

function mapTransferWallet(wallet: BackendWalletTransferAccount, symbol: string): StrictWalletAccount {
  return mapWalletAccount(wallet, symbol)
}

function mapQuickRechargeOrder(order: BackendQuickRechargeOrder): QuickRechargeOrder {
  const createdAt = asNumber(order.created_at)
  const fiatAmountText = walletDecimal(order.fiat_amount, 'quick-recharge fiat_amount')
  const actualAmountText = nullableWalletDecimal(order.actual_amount, 'quick-recharge actual_amount')
  return {
    id: order.id,
    orderId: order.order_id,
    assetSymbol: String(order.asset_symbol || order.token || '').toUpperCase(),
    currency: order.currency.toUpperCase(),
    token: order.token.toUpperCase(),
    network: String(order.network || ''),
    fiatAmount: decimalDisplayNumber(fiatAmountText, 'quick-recharge fiat amount'),
    actualAmount: actualAmountText ? decimalDisplayNumber(actualAmountText, 'quick-recharge actual amount') : undefined,
    fiatAmountText,
    actualAmountText: actualAmountText || undefined,
    paymentUrl: order.payment_url || undefined,
    redirectUrl: order.redirect_url || undefined,
    status: order.status,
    createdAt: createdAt > 0 && createdAt < 1_000_000_000_000 ? createdAt * 1000 : createdAt || undefined,
  }
}

function mapDepositAsset(asset: BackendDepositAsset): WalletDepositAsset {
  const minDepositAmountText = walletDecimal(asset.min_deposit_amount, 'min_deposit_amount')
  const depositFeeText = walletDecimal(asset.deposit_fee, 'deposit_fee')
  return {
    symbol: asset.symbol.toUpperCase(),
    name: asset.name?.trim() || undefined,
    logoUrl: asset.logo_url?.trim() || undefined,
    depositEnabled: asset.deposit_enabled !== false,
    minDepositAmount: decimalDisplayNumber(minDepositAmountText, 'min deposit amount'),
    minDepositAmountText,
    depositFee: decimalDisplayNumber(depositFeeText, 'deposit fee'),
    depositFeeText,
  }
}

function mapWalletAccount(
  account: BackendWalletAccount | BackendWalletTransferAccount,
  symbol: string,
): StrictWalletAccount {
  const availableText = walletDecimal(account.available, 'account available')
  const frozenText = walletDecimal(account.frozen, 'account frozen')
  const lockedText = walletDecimal(account.locked, 'account locked')
  return {
    assetId: requiredWalletInteger(account.asset_id, 'account asset_id'),
    symbol: symbol.toUpperCase(),
    logoUrl: 'logo_url' in account ? account.logo_url?.trim() || undefined : undefined,
    available: decimalDisplayNumber(availableText, 'account available'),
    frozen: decimalDisplayNumber(frozenText, 'account frozen'),
    locked: decimalDisplayNumber(lockedText, 'account locked'),
    availableText,
    frozenText,
    lockedText,
  }
}

function walletDecimal(value: unknown, field: string): DecimalText {
  try {
    return requiredDecimalText(value, field, 'wallet financial response', WALLET_DECIMAL_CONSTRAINTS)
  } catch {
    throw new WalletFinancialContractError(field)
  }
}

function nullableWalletDecimal(value: unknown, field: string): DecimalText | null {
  if (value === null || value === undefined) return null
  return walletDecimal(value, field)
}

function decimalDisplayNumber(value: DecimalText, field: string): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) throw new WalletFinancialContractError(field)
  return parsed
}

function requiredWalletInteger(value: unknown, field: string): number {
  const parsed = typeof value === 'string' && value.trim() ? Number(value) : value
  if (typeof parsed !== 'number' || !Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new WalletFinancialContractError(field)
  }
  return parsed
}

function walletPrecisionScale(value: unknown): number {
  const parsed = typeof value === 'string' && value.trim() ? Number(value) : value
  if (typeof parsed !== 'number' || !Number.isSafeInteger(parsed) || parsed < 0 || parsed > 18) {
    throw new WalletFinancialContractError('precision_scale')
  }
  return parsed
}
