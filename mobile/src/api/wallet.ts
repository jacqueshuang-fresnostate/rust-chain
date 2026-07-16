import { client, requestUrl } from './client'
import { asNumber } from '@/core/format'
import type { DepositAddress, DepositAsset, DepositNetwork, WalletAccount } from '@/core/types'

export interface WithdrawalAsset extends DepositAsset {
  withdrawEnabled: boolean
  withdrawFee: number
  name?: string
}

export interface WalletLedgerEntry {
  id: number
  symbol: string
  changeType: string
  amount: number
  fee: number
  balanceAfter: number
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

interface BackendLedgerEntry {
  id: number
  symbol: string
  change_type: string
  amount: string | number
  fee?: string | number | null
  balance_after?: string | number
  created_at: number
}

export async function fetchDepositAssets(): Promise<DepositAsset[]> {
  const response = await client.get<{ assets?: BackendDepositAsset[] }>(requestUrl('/wallet/deposit-assets'))
  return (response.data.assets || [])
    .map((asset) => ({
      symbol: asset.symbol.toUpperCase(),
      logoUrl: asset.logo_url?.trim() || undefined,
      depositEnabled: asset.deposit_enabled !== false,
      minDepositAmount: asNumber(asset.min_deposit_amount),
    }))
    .filter((asset) => asset.depositEnabled)
}

export async function fetchWithdrawalAssets(): Promise<WithdrawalAsset[]> {
  const response = await client.get<{ assets?: BackendDepositAsset[] }>(requestUrl('/wallet/withdraw-assets'))
  return (response.data.assets || [])
    .map((asset) => ({
      symbol: asset.symbol.toUpperCase(),
      logoUrl: asset.logo_url?.trim() || undefined,
      depositEnabled: asset.deposit_enabled !== false,
      withdrawEnabled: asset.withdraw_enabled !== false,
      minDepositAmount: asNumber(asset.min_deposit_amount),
      withdrawFee: asNumber(asset.withdraw_fee),
      name: asset.name?.trim() || undefined,
    }))
    .filter((asset) => asset.withdrawEnabled)
}

export async function fetchDepositNetworks(assetSymbol: string, minimum = 0): Promise<DepositNetwork[]> {
  const response = await client.get<{ networks?: BackendDepositNetwork[] }>(requestUrl('/wallet/deposit-networks'), {
    params: { asset_symbol: assetSymbol.toUpperCase() },
  })
  return (response.data.networks || []).map((network) => ({
    network: network.network,
    displayName: network.display_name?.trim() || network.network,
    estimatedMinutes: networkMinutes(network.network),
    minDepositAmount: minimum,
  }))
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

export async function submitWithdrawal(input: { assetSymbol: string; network?: string; address: string; amount: number; fee: number; fundPassword?: string; totpCode?: string }): Promise<void> {
  await client.post(requestUrl('/wallet/withdrawals'), {
    asset_symbol: input.assetSymbol.toUpperCase(),
    network: input.network,
    address: input.address.trim(),
    amount: String(input.amount),
    fee: String(input.fee),
    fund_password: input.fundPassword?.trim() || undefined,
    totp_code: input.totpCode?.trim() || undefined,
  })
}

export async function fetchWalletLedger(limit = 30, offset = 0, changeType?: string): Promise<WalletLedgerEntry[]> {
  const response = await client.get<{ entries?: BackendLedgerEntry[] }>(requestUrl('/wallet/ledger'), {
    params: { limit, offset, change_type: changeType || undefined },
  })
  return (response.data.entries || []).map((entry) => ({
    id: entry.id,
    symbol: entry.symbol.toUpperCase(),
    changeType: entry.change_type,
    amount: asNumber(entry.amount),
    fee: asNumber(entry.fee),
    balanceAfter: asNumber(entry.balance_after),
    createdAt: entry.created_at > 0 && entry.created_at < 1_000_000_000_000 ? entry.created_at * 1000 : entry.created_at,
  }))
}

export async function fetchQuickRechargeConfig(): Promise<QuickRechargeConfig> {
  const response = await client.get<{ enabled?: boolean; currency?: string; token?: string; network?: string; min_amount?: string | number; max_amount?: string | number | null }>(requestUrl('/wallet/quick-recharge/config'))
  return {
    enabled: Boolean(response.data.enabled),
    currency: String(response.data.currency || 'USD').toUpperCase(),
    token: String(response.data.token || 'USDT').toUpperCase(),
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

export async function transferWalletFunds(assetSymbol: string, from: 'spot' | 'margin', to: 'spot' | 'margin', amount: number): Promise<void> {
  await client.post(requestUrl('/margin/transfers'), {
    asset_symbol: assetSymbol.toUpperCase(),
    from,
    to,
    amount: String(amount),
  })
}

function networkMinutes(network: string): number {
  const normalized = network.toLowerCase()
  if (normalized.includes('ethereum') || normalized.includes('erc20')) return 7
  if (normalized.includes('arbitrum')) return 19
  return 1
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
