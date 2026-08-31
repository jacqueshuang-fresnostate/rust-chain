export type FinancialEnumTone = 'positive' | 'negative' | 'pending' | 'neutral'

export interface FinancialEnumPresentation<Known extends string> {
  known: boolean
  value?: Known
  source: string
  translationKey: string
  tone: FinancialEnumTone
}

interface KnownPresentation {
  translationKey: string
  tone: FinancialEnumTone
}

export type KycKnownStatus = 'pending' | 'approved' | 'rejected'
export type OrderKnownStatus =
  | 'submitted' | 'pending' | 'trading' | 'open' | 'partially_filled'
  | 'completed' | 'filled' | 'canceled' | 'cancelled' | 'closed'
  | 'liquidated' | 'rejected'
export type QuickRechargeKnownStatus =
  | 'created' | 'pending' | 'processing' | 'confirmed' | 'paid'
  | 'success' | 'succeeded' | 'completed' | 'failed' | 'rejected'
  | 'canceled' | 'cancelled' | 'expired'
export type ConvertKnownStatus =
  | 'quoted' | 'pending' | 'processing' | 'confirmed' | 'completed'
  | 'failed' | 'rejected' | 'canceled' | 'cancelled' | 'expired'
export type EarnKnownStatus =
  | 'pending' | 'subscribed' | 'active' | 'matured' | 'redeeming'
  | 'redeemed' | 'completed' | 'failed' | 'canceled' | 'cancelled'
export type EarnKnownCategory = 'flexible' | 'fixed' | 'staking'
export type ReferralKnownStatus =
  | 'pending' | 'registered' | 'verified' | 'active' | 'inactive'
  | 'bound' | 'completed' | 'rejected'

const KYC_STATUS = {
  pending: { translationKey: 'kyc.pending', tone: 'pending' },
  approved: { translationKey: 'kyc.approved', tone: 'positive' },
  rejected: { translationKey: 'kyc.rejected', tone: 'negative' },
} as const satisfies Record<KycKnownStatus, KnownPresentation>

const ORDER_STATUS = {
  submitted: { translationKey: 'orders.statusSubmitted', tone: 'pending' },
  pending: { translationKey: 'orders.statusPending', tone: 'pending' },
  trading: { translationKey: 'orders.statusTrading', tone: 'pending' },
  open: { translationKey: 'orders.statusTrading', tone: 'pending' },
  partially_filled: { translationKey: 'orders.statusPartiallyFilled', tone: 'pending' },
  completed: { translationKey: 'orders.statusCompleted', tone: 'positive' },
  filled: { translationKey: 'orders.statusCompleted', tone: 'positive' },
  canceled: { translationKey: 'orders.statusCanceled', tone: 'negative' },
  cancelled: { translationKey: 'orders.statusCanceled', tone: 'negative' },
  closed: { translationKey: 'orders.statusClosed', tone: 'positive' },
  liquidated: { translationKey: 'orders.statusLiquidated', tone: 'negative' },
  rejected: { translationKey: 'orders.statusRejected', tone: 'negative' },
} as const satisfies Record<OrderKnownStatus, KnownPresentation>

const QUICK_RECHARGE_STATUS = {
  created: pending('common.statusCreated'),
  pending: pending('common.statusPending'),
  processing: pending('common.statusProcessing'),
  confirmed: positive('common.statusConfirmed'),
  paid: positive('common.statusPaid'),
  success: positive('common.statusCompleted'),
  succeeded: positive('common.statusCompleted'),
  completed: positive('common.statusCompleted'),
  failed: negative('common.statusFailed'),
  rejected: negative('common.statusRejected'),
  canceled: negative('common.statusCancelled'),
  cancelled: negative('common.statusCancelled'),
  expired: negative('common.statusExpired'),
} as const satisfies Record<QuickRechargeKnownStatus, KnownPresentation>

const CONVERT_STATUS = {
  quoted: pending('common.statusQuoted'),
  pending: pending('common.statusPending'),
  processing: pending('common.statusProcessing'),
  confirmed: positive('common.statusConfirmed'),
  completed: positive('common.statusCompleted'),
  failed: negative('common.statusFailed'),
  rejected: negative('common.statusRejected'),
  canceled: negative('common.statusCancelled'),
  cancelled: negative('common.statusCancelled'),
  expired: negative('common.statusExpired'),
} as const satisfies Record<ConvertKnownStatus, KnownPresentation>

const EARN_STATUS = {
  pending: pending('common.statusPending'),
  subscribed: pending('common.statusSubscribed'),
  active: positive('common.statusActive'),
  matured: positive('common.statusMatured'),
  redeeming: pending('common.statusRedeeming'),
  redeemed: positive('common.statusRedeemed'),
  completed: positive('common.statusCompleted'),
  failed: negative('common.statusFailed'),
  canceled: negative('common.statusCancelled'),
  cancelled: negative('common.statusCancelled'),
} as const satisfies Record<EarnKnownStatus, KnownPresentation>

const EARN_CATEGORY = {
  flexible: { translationKey: 'common.categoryFlexible', tone: 'neutral' },
  fixed: { translationKey: 'common.categoryFixed', tone: 'neutral' },
  staking: { translationKey: 'common.categoryStaking', tone: 'neutral' },
} as const satisfies Record<EarnKnownCategory, KnownPresentation>

const REFERRAL_STATUS = {
  pending: pending('common.statusPending'),
  registered: pending('common.statusRegistered'),
  verified: positive('common.statusVerified'),
  active: positive('common.statusActive'),
  inactive: neutral('common.statusInactive'),
  bound: positive('common.statusBound'),
  completed: positive('common.statusCompleted'),
  rejected: negative('common.statusRejected'),
} as const satisfies Record<ReferralKnownStatus, KnownPresentation>

export function kycStatusPresentation(source: string): FinancialEnumPresentation<KycKnownStatus> {
  return present(source, KYC_STATUS, 'common.unknownStatusWithSource')
}

export function orderStatusPresentation(source: string): FinancialEnumPresentation<OrderKnownStatus> {
  return present(source, ORDER_STATUS, 'common.unknownStatusWithSource')
}

export function quickRechargeStatusPresentation(
  source: string,
): FinancialEnumPresentation<QuickRechargeKnownStatus> {
  return present(source, QUICK_RECHARGE_STATUS, 'common.unknownStatusWithSource')
}

export function convertStatusPresentation(source: string): FinancialEnumPresentation<ConvertKnownStatus> {
  return present(source, CONVERT_STATUS, 'common.unknownStatusWithSource')
}

export function earnStatusPresentation(source: string): FinancialEnumPresentation<EarnKnownStatus> {
  return present(source, EARN_STATUS, 'common.unknownStatusWithSource')
}

export function earnCategoryPresentation(source: string): FinancialEnumPresentation<EarnKnownCategory> {
  return present(source, EARN_CATEGORY, 'common.unknownCategoryWithSource')
}

export function referralStatusPresentation(source: string): FinancialEnumPresentation<ReferralKnownStatus> {
  return present(source, REFERRAL_STATUS, 'common.unknownStatusWithSource')
}

export function isEarnRedeemableStatus(source: string): boolean {
  const parsed = earnStatusPresentation(source)
  return parsed.known && (parsed.value === 'subscribed' || parsed.value === 'active' || parsed.value === 'matured')
}

export function isKycSubmissionLocked(source: string): boolean {
  const parsed = kycStatusPresentation(source)
  // Unknown workflow states are conservatively locked without presenting them as pending.
  return !parsed.known || parsed.value === 'pending' || parsed.value === 'approved'
}

function present<Known extends string>(
  sourceValue: string,
  knownValues: Record<Known, KnownPresentation>,
  unknownTranslationKey: string,
): FinancialEnumPresentation<Known> {
  const source = sourceValue.trim()
  const normalized = source.toLowerCase() as Known
  const known = knownValues[normalized]
  return known
    ? { known: true, value: normalized, source, ...known }
    : {
        known: false,
        source,
        translationKey: unknownTranslationKey,
        tone: 'neutral',
      }
}

function positive(translationKey: string): KnownPresentation {
  return { translationKey, tone: 'positive' }
}

function negative(translationKey: string): KnownPresentation {
  return { translationKey, tone: 'negative' }
}

function pending(translationKey: string): KnownPresentation {
  return { translationKey, tone: 'pending' }
}

function neutral(translationKey: string): KnownPresentation {
  return { translationKey, tone: 'neutral' }
}
