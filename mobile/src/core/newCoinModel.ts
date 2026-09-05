import {
  requiredDecimalText,
  type DecimalText,
} from './decimal.ts'

const FINANCIAL_DECIMAL = {
  allowNegative: false,
  maxIntegerDigits: 20,
  maxScale: 18,
} as const

export class NewCoinContractError extends Error {
  constructor(field: string) {
    super(`invalid new-coin response ${field}`)
    this.name = 'NewCoinContractError'
  }
}

export interface NewCoinProject {
  id: number
  assetId: number
  symbol: string
  name?: string
  logoUrl?: string
  lifecycleStatus: string
  totalSupplyText: DecimalText
  issuePriceText: DecimalText
  quoteAssetId?: number
  quoteAssetSymbol?: string
  quoteAssetLogoUrl?: string
  reservedSupplyText: DecimalText
  allocatedSupplyText: DecimalText
  remainingSupplyText: DecimalText
  listedAt?: number
  unlockType: string
  fixedUnlockAt?: number
  relativeUnlockSeconds?: number
  unlockFeeEnabled: boolean
  unlockFeeRateText?: DecimalText
  unlockFeeBasis?: string
  unlockFeeAssetId?: number
  postListingPurchaseEnabled: boolean
  postListingPairId?: number
  status: string
}

export interface NewCoinSubscription {
  id: number
  projectId: number
  quoteAssetId: number
  issuePriceText: DecimalText
  quoteAmountText: DecimalText
  requestedQuantityText: DecimalText
  allocatedQuantityText: DecimalText
  status: string
  idempotencyKey: string
  createdAt: number
  settlementMode?: string
  frozenQuoteAmountText?: DecimalText
  settledQuoteAmountText?: DecimalText
  refundedQuoteAmountText?: DecimalText
}

export interface NewCoinDistribution {
  id: number
  projectId: number
  subscriptionId?: number
  assetId: number
  quantityText: DecimalText
  lockPositionId?: number
  status: string
  idempotencyKey: string
  createdAt: number
}

export interface NewCoinPurchase {
  id: number
  projectId: number
  pairId: number
  baseAssetId: number
  quoteAssetId: number
  priceText: DecimalText
  quantityText: DecimalText
  quoteAmountText: DecimalText
  lockPositionId?: number
  status: string
  idempotencyKey: string
  createdAt: number
}

export interface NewCoinUnlock {
  id: number
  assetId: number
  lockPositionId: number
  unlockQuantityText: DecimalText
  unlockPriceText?: DecimalText
  unlockFeeEnabled: boolean
  unlockFeeRateText?: DecimalText
  unlockFeeBasis?: string
  unlockFeeAssetId?: number
  unlockFeeAmountText?: DecimalText
  feePaidStatus: string
  status: string
  idempotencyKey: string
  createdAt: number
}

export function mapNewCoinProject(raw: Record<string, unknown>): NewCoinProject {
  return {
    id: requiredId(raw.id, 'project.id'),
    assetId: requiredId(raw.asset_id, 'project.asset_id'),
    symbol: requiredText(raw.symbol, 'project.symbol').toUpperCase(),
    name: optionalText(raw.name, 'project.name'),
    logoUrl: optionalText(raw.logo_url, 'project.logo_url'),
    lifecycleStatus: requiredText(raw.lifecycle_status, 'project.lifecycle_status'),
    totalSupplyText: decimal(raw.total_supply, 'project.total_supply'),
    issuePriceText: decimal(raw.issue_price, 'project.issue_price'),
    quoteAssetId: optionalId(raw.quote_asset_id, 'project.quote_asset_id'),
    quoteAssetSymbol: optionalText(raw.quote_asset_symbol, 'project.quote_asset_symbol')?.toUpperCase(),
    quoteAssetLogoUrl: optionalText(raw.quote_asset_logo_url, 'project.quote_asset_logo_url'),
    reservedSupplyText: decimal(raw.reserved_supply, 'project.reserved_supply'),
    allocatedSupplyText: decimal(raw.allocated_supply, 'project.allocated_supply'),
    remainingSupplyText: decimal(raw.remaining_supply, 'project.remaining_supply'),
    listedAt: optionalTimestamp(raw.listed_at, 'project.listed_at'),
    unlockType: requiredText(raw.unlock_type, 'project.unlock_type'),
    fixedUnlockAt: optionalTimestamp(raw.fixed_unlock_at, 'project.fixed_unlock_at'),
    relativeUnlockSeconds: optionalInteger(raw.relative_unlock_seconds, 'project.relative_unlock_seconds'),
    unlockFeeEnabled: requiredBoolean(raw.unlock_fee_enabled, 'project.unlock_fee_enabled'),
    unlockFeeRateText: optionalDecimal(raw.unlock_fee_rate, 'project.unlock_fee_rate'),
    unlockFeeBasis: optionalText(raw.unlock_fee_basis, 'project.unlock_fee_basis'),
    unlockFeeAssetId: optionalId(raw.unlock_fee_asset, 'project.unlock_fee_asset'),
    postListingPurchaseEnabled: requiredBoolean(
      raw.post_listing_purchase_enabled,
      'project.post_listing_purchase_enabled',
    ),
    postListingPairId: optionalId(raw.post_listing_pair_id, 'project.post_listing_pair_id'),
    status: requiredText(raw.status, 'project.status'),
  }
}

export function mapNewCoinSubscription(raw: Record<string, unknown>): NewCoinSubscription {
  const settlementDecimal = raw.settlement_mode === 'manual_distribution' ? decimal : optionalDecimal
  return {
    id: requiredId(raw.id, 'subscription.id'),
    projectId: requiredId(raw.project_id, 'subscription.project_id'),
    quoteAssetId: requiredId(raw.quote_asset, 'subscription.quote_asset'),
    issuePriceText: decimal(raw.issue_price, 'subscription.issue_price'),
    quoteAmountText: decimal(raw.quote_amount, 'subscription.quote_amount'),
    requestedQuantityText: decimal(raw.requested_quantity, 'subscription.requested_quantity'),
    allocatedQuantityText: decimal(raw.allocated_quantity, 'subscription.allocated_quantity'),
    settlementMode: optionalText(raw.settlement_mode, 'subscription.settlement_mode'),
    frozenQuoteAmountText: settlementDecimal(raw.frozen_quote_amount, 'subscription.frozen_quote_amount'),
    settledQuoteAmountText: settlementDecimal(raw.settled_quote_amount, 'subscription.settled_quote_amount'),
    refundedQuoteAmountText: settlementDecimal(raw.refunded_quote_amount, 'subscription.refunded_quote_amount'),
    status: requiredText(raw.status, 'subscription.status'),
    idempotencyKey: requiredText(raw.idempotency_key, 'subscription.idempotency_key'),
    createdAt: requiredTimestamp(raw.created_at, 'subscription.created_at'),
  }
}

export function mapNewCoinDistribution(raw: Record<string, unknown>): NewCoinDistribution {
  return {
    id: requiredId(raw.id, 'distribution.id'),
    projectId: requiredId(raw.project_id, 'distribution.project_id'),
    subscriptionId: optionalId(raw.subscription_id, 'distribution.subscription_id'),
    assetId: requiredId(raw.asset_id, 'distribution.asset_id'),
    quantityText: decimal(raw.quantity, 'distribution.quantity'),
    lockPositionId: optionalId(raw.lock_position_id, 'distribution.lock_position_id'),
    status: requiredText(raw.status, 'distribution.status'),
    idempotencyKey: requiredText(raw.idempotency_key, 'distribution.idempotency_key'),
    createdAt: requiredTimestamp(raw.created_at, 'distribution.created_at'),
  }
}

export function mapNewCoinPurchase(raw: Record<string, unknown>): NewCoinPurchase {
  return {
    id: requiredId(raw.id, 'purchase.id'),
    projectId: requiredId(raw.project_id, 'purchase.project_id'),
    pairId: requiredId(raw.pair_id, 'purchase.pair_id'),
    baseAssetId: requiredId(raw.base_asset, 'purchase.base_asset'),
    quoteAssetId: requiredId(raw.quote_asset, 'purchase.quote_asset'),
    priceText: decimal(raw.price, 'purchase.price'),
    quantityText: decimal(raw.quantity, 'purchase.quantity'),
    quoteAmountText: decimal(raw.quote_amount, 'purchase.quote_amount'),
    lockPositionId: optionalId(raw.lock_position_id, 'purchase.lock_position_id'),
    status: requiredText(raw.status, 'purchase.status'),
    idempotencyKey: requiredText(raw.idempotency_key, 'purchase.idempotency_key'),
    createdAt: requiredTimestamp(raw.created_at, 'purchase.created_at'),
  }
}

export function mapNewCoinUnlock(raw: Record<string, unknown>): NewCoinUnlock {
  return {
    id: requiredId(raw.id, 'unlock.id'),
    assetId: requiredId(raw.asset_id, 'unlock.asset_id'),
    lockPositionId: requiredId(raw.lock_position_id, 'unlock.lock_position_id'),
    unlockQuantityText: decimal(raw.unlock_quantity, 'unlock.unlock_quantity'),
    unlockPriceText: optionalDecimal(raw.unlock_price, 'unlock.unlock_price'),
    unlockFeeEnabled: requiredBoolean(raw.unlock_fee_enabled, 'unlock.unlock_fee_enabled'),
    unlockFeeRateText: optionalDecimal(raw.unlock_fee_rate, 'unlock.unlock_fee_rate'),
    unlockFeeBasis: optionalText(raw.unlock_fee_basis, 'unlock.unlock_fee_basis'),
    unlockFeeAssetId: optionalId(raw.unlock_fee_asset, 'unlock.unlock_fee_asset'),
    unlockFeeAmountText: optionalDecimal(raw.unlock_fee_amount, 'unlock.unlock_fee_amount'),
    feePaidStatus: requiredText(raw.fee_paid_status, 'unlock.fee_paid_status'),
    status: requiredText(raw.status, 'unlock.status'),
    idempotencyKey: requiredText(raw.idempotency_key, 'unlock.idempotency_key'),
    createdAt: requiredTimestamp(raw.created_at, 'unlock.created_at'),
  }
}

function decimal(value: unknown, field: string): DecimalText {
  try {
    return requiredDecimalText(value, field, 'new-coin response', FINANCIAL_DECIMAL)
  } catch {
    throw new NewCoinContractError(field)
  }
}

function optionalDecimal(value: unknown, field: string): DecimalText | undefined {
  return value === null || value === undefined ? undefined : decimal(value, field)
}

function requiredText(value: unknown, field: string): string {
  if (typeof value !== 'string' || !value.trim()) throw new NewCoinContractError(field)
  return value.trim()
}

function optionalText(value: unknown, field: string): string | undefined {
  if (value === null || value === undefined) return undefined
  if (typeof value !== 'string') throw new NewCoinContractError(field)
  return value.trim() || undefined
}

function requiredBoolean(value: unknown, field: string): boolean {
  if (typeof value !== 'boolean') throw new NewCoinContractError(field)
  return value
}

function requiredId(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
    throw new NewCoinContractError(field)
  }
  return value
}

function optionalId(value: unknown, field: string): number | undefined {
  return value === null || value === undefined ? undefined : requiredId(value, field)
}

function optionalInteger(value: unknown, field: string): number | undefined {
  if (value === null || value === undefined) return undefined
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new NewCoinContractError(field)
  }
  return value
}

function requiredTimestamp(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
    throw new NewCoinContractError(field)
  }
  return value < 1_000_000_000_000 ? value * 1000 : value
}

function optionalTimestamp(value: unknown, field: string): number | undefined {
  return value === null || value === undefined ? undefined : requiredTimestamp(value, field)
}
