import { client, readAccessToken, requestUrl } from './client'
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
  isWalletLedgerAccountFilter,
  isWalletLedgerCategory,
  mapWalletLedgerResponse,
  WalletLedgerContractError,
  type BackendWalletLedgerResponse,
  type WalletLedgerAccountFilter,
  type WalletLedgerCategory,
  type WalletLedgerPage,
} from '@/core/walletLedger'

import type { DepositAddress, DepositAsset, DepositNetwork, WalletAccount } from '@/core/types'
import {
  isWithdrawalDecimalString,
  withdrawalQuoteAmountsAreConsistent,
  type WithdrawalFeeTier,
} from '@/core/withdrawalQuote'

const walletTransferIdempotencyKeys = new RetryStableIdempotencyKeys('mobile-transfer')

export {
  calculateWithdrawalFee,
  maximumQuotedWithdrawalAmount,
  normalizeWithdrawalPreviewAmount,
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
  createWalletLedgerRequestLifecycle,
  formatWalletLedgerDecimal,
  formatWalletLedgerGroupHeading,
  formatWalletLedgerTime,
  groupWalletLedgerEntries,
  isWalletLedgerAccountFilter,
  isWalletLedgerAccountType,
  isWalletLedgerContractError,
  isWalletLedgerCategory,
  mapWalletLedgerResponse,
  mergeWalletLedgerEntries,
  WALLET_LEDGER_ACCOUNT_FILTERS,
  WALLET_LEDGER_ACCOUNT_TYPES,
  WALLET_LEDGER_CATEGORIES,
  WALLET_LEDGER_FILTERS,
  WALLET_LEDGER_KNOWN_CHANGE_TYPES,
  WALLET_LEDGER_MAX_FRACTION_DIGITS,
  walletLedgerAmountSign,
  walletLedgerAccountTranslationKey,
  walletLedgerCategoryTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerTypePresentation,
  WalletLedgerContractError,
} from '@/core/walletLedger'
export type {
  WalletLedgerAccountFilter,
  WalletLedgerAccountType,
  WalletLedgerCategory,
  WalletLedgerDateGroup,
  WalletLedgerEntry,
  WalletLedgerFilter,
  WalletLedgerPage,
  WalletLedgerPaginationState,
  WalletLedgerRequestLifecycle,
  WalletLedgerRequestResult,
} from '@/core/walletLedger'

export interface WithdrawalAsset extends DepositAsset {
  withdrawEnabled: boolean
  withdrawFee: number
  precisionScale: number
  withdrawFeeTiers: WithdrawalFeeTier[]
}

export interface WithdrawalQuote {
  quoteId: string
  assetSymbol: string
  network: string
  amount: string
  fee: string
  net: string
  totalReserved: string
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
  spotWallet: WalletAccount
  marginWallet: WalletAccount
}

export async function fetchDepositAssets(options: ReferenceRequestOptions = {}): Promise<DepositAsset[]> {
  const url = requestUrl('/wallet/deposit-assets')
  return referenceRequestRegistry.request(walletReferenceKey(url), 30_000, async () => {
    const response = await client.get<{ assets?: BackendDepositAsset[] }>(url)
    return (response.data.assets || [])
      .map((asset) => ({
        symbol: asset.symbol.toUpperCase(),
        name: asset.name?.trim() || undefined,
        logoUrl: asset.logo_url?.trim() || undefined,
        depositEnabled: asset.deposit_enabled !== false,
        minDepositAmount: asNumber(asset.min_deposit_amount),
      }))
      .filter((asset) => asset.depositEnabled)
  }, options)
}

export async function fetchWithdrawalAssets(options: ReferenceRequestOptions = {}): Promise<WithdrawalAsset[]> {
  const url = requestUrl('/wallet/withdraw-assets')
  return referenceRequestRegistry.request(walletReferenceKey(url), 30_000, async () => {
    const response = await client.get<{ assets?: BackendDepositAsset[] }>(url)
    return (response.data.assets || [])
      .map((asset) => ({
        symbol: asset.symbol.toUpperCase(),
        logoUrl: asset.logo_url?.trim() || undefined,
        depositEnabled: asset.deposit_enabled !== false,
        withdrawEnabled: asset.withdraw_enabled !== false,
        minDepositAmount: asNumber(asset.min_deposit_amount),
        withdrawFee: asNumber(asset.withdraw_fee),
        precisionScale: Math.min(18, Math.max(0, Math.trunc(asNumber(asset.precision_scale)))),
        withdrawFeeTiers: (asset.withdraw_fee_tiers || []).map((tier) => ({
          minAmount: asNumber(tier.min_amount),
          maxAmount: tier.max_amount == null ? undefined : asNumber(tier.max_amount),
          feeRatePercent: asNumber(tier.fee_rate_percent),
        })).sort((left, right) => left.minAmount - right.minAmount),
        name: asset.name?.trim() || undefined,
      }))
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
  return createReferenceRequestKey(url, params, `wallet:${readAccessToken() || 'guest'}`)
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

export async function fetchWalletAccounts(): Promise<WalletAccount[]> {
  const response = await client.get<{ accounts?: BackendWalletAccount[] }>(requestUrl('/wallet/accounts'))
  return (response.data.accounts || []).map((account) => ({
    assetId: asNumber(account.asset_id),
    symbol: account.symbol.toUpperCase(),
    logoUrl: account.logo_url?.trim() || undefined,
    available: asNumber(account.available),
    frozen: asNumber(account.frozen),
    locked: asNumber(account.locked),
  }))
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
  amount: string | number
}): Promise<WithdrawalQuote> {
  const response = await client.post<BackendWithdrawalQuote>(requestUrl('/wallet/withdrawals/quote'), {
    asset_symbol: input.assetSymbol.toUpperCase(),
    network: input.network,
    amount: String(input.amount),
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
    throw new Error('invalid withdrawal quote response')
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

function decimalString(value: string | number | undefined): string {
  if (typeof value !== 'string') {
    throw new Error('invalid withdrawal decimal response')
  }
  const normalized = value.trim()
  if (!isWithdrawalDecimalString(normalized)) {
    throw new Error('invalid withdrawal decimal response')
  }
  return normalized
}

function sameDecimal(left: string, right: string): boolean {
  return canonicalDecimal(left) === canonicalDecimal(right)
}

function canonicalDecimal(value: string): string {
  const match = value.trim().match(/^([+-]?)(\d+)(?:\.(\d*))?$/)
  if (!match) return value.trim()
  const whole = (match[2] || '0').replace(/^0+(?=\d)/, '')
  const fraction = (match[3] || '').replace(/0+$/, '')
  const zero = whole === '0' && !fraction
  return `${match[1] === '-' && !zero ? '-' : ''}${whole}${fraction ? `.${fraction}` : ''}`
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
  return (response.data.withdrawals || []).map((record) => ({
    id: record.id,
    assetSymbol: record.asset_symbol.toUpperCase(),
    network: record.network || undefined,
    address: record.address,
    amount: asNumber(record.amount),
    fee: asNumber(record.fee),
    status: record.status,
    txHash: record.tx_hash || undefined,
    failureReason: record.failure_reason || undefined,
    reviewReason: record.review_reason || undefined,
    createdAt: record.created_at > 0 && record.created_at < 1_000_000_000_000 ? record.created_at * 1000 : record.created_at,
  }))
}

function createWithdrawalIdempotencyKey(quoteId: string): string {
  return `mobile-withdraw-${quoteId}`
}

export async function fetchWalletLedger(options: {
  limit?: number
  offset?: number
  category?: WalletLedgerCategory
  accountType?: WalletLedgerAccountFilter
  changeType?: string
} = {}): Promise<WalletLedgerPage> {
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
  const changeType = options.changeType?.trim()
  const response = await client.get<BackendWalletLedgerResponse>(requestUrl('/wallet/ledger'), {
    params: {
      limit,
      offset,
      category: options.category,
      account_type: accountType,
      change_type: changeType || undefined,
    },
  })
  return mapWalletLedgerResponse(response.data)
}

export async function fetchQuickRechargeConfig(): Promise<QuickRechargeConfig> {
  const response = await client.get<{ enabled?: boolean; currency?: string; token?: string; network?: string; min_amount?: string | number; max_amount?: string | number | null }>(requestUrl('/wallet/quick-recharge/config'))
  return {
    enabled: Boolean(response.data.enabled),
    currency: String(response.data.currency || '').toUpperCase(),
    token: String(response.data.token || '').toUpperCase(),
    network: String(response.data.network || ''),
    minAmount: asNumber(response.data.min_amount),
    maxAmount: response.data.max_amount === null || response.data.max_amount === undefined ? undefined : asNumber(response.data.max_amount),
  }
}

export async function createQuickRechargeOrder(amount: number, returnTarget: 'ios_app' | 'android_app' | 'mobile_web' | 'desktop_web'): Promise<QuickRechargeOrder> {
  const response = await client.post<BackendQuickRechargeOrder>(requestUrl('/wallet/quick-recharge/orders'), {
    amount: String(amount),
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

export async function transferWalletFunds(assetSymbol: string, from: 'spot' | 'margin', to: 'spot' | 'margin', amount: number): Promise<WalletTransferResult> {
  const symbol = assetSymbol.toUpperCase()
  const businessIntent = {
    asset_symbol: symbol,
    from,
    to,
    amount: String(amount),
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

function mapTransferWallet(wallet: BackendWalletTransferAccount, symbol: string): WalletAccount {
  return {
    assetId: asNumber(wallet.asset_id),
    symbol,
    available: asNumber(wallet.available),
    frozen: asNumber(wallet.frozen),
    locked: asNumber(wallet.locked),
  }
}

function mapQuickRechargeOrder(order: BackendQuickRechargeOrder): QuickRechargeOrder {
  const createdAt = asNumber(order.created_at)
  return {
    id: order.id,
    orderId: order.order_id,
    assetSymbol: String(order.asset_symbol || order.token || '').toUpperCase(),
    currency: order.currency.toUpperCase(),
    token: order.token.toUpperCase(),
    network: String(order.network || ''),
    fiatAmount: asNumber(order.fiat_amount),
    actualAmount: order.actual_amount === null || order.actual_amount === undefined ? undefined : asNumber(order.actual_amount),
    paymentUrl: order.payment_url || undefined,
    redirectUrl: order.redirect_url || undefined,
    status: order.status,
    createdAt: createdAt > 0 && createdAt < 1_000_000_000_000 ? createdAt * 1000 : createdAt || undefined,
  }
}
