import type { requestConvertQuote } from '@/api/swap'
import type { applyLoan } from '@/api/loan'
import type { subscribeEarnProduct } from '@/api/earn'
import type { requestPredictionQuote } from '@/api/prediction'
import type {
  createNewCoinPurchase,
  payNewCoinUnlockFee,
  subscribeNewCoin,
} from '@/api/newCoin'
import type {
  createQuickRechargeOrder,
  fetchWithdrawalQuote,
  transferWalletFunds,
} from '@/api/wallet'
import type { OpenSecondsOrderInput } from '@/api/seconds'
import type { MarginOrderInput, SpotOrderInput } from '@/api/trading'

type AssertFalse<Value extends false> = Value
type AcceptsNumber<Value> = number extends Value ? true : false

type SpotQuantityRejectsNumber = AssertFalse<AcceptsNumber<SpotOrderInput['quantity']>>
type SpotPriceRejectsNumber = AssertFalse<AcceptsNumber<NonNullable<SpotOrderInput['price']>>>
type MarginAmountRejectsNumber = AssertFalse<AcceptsNumber<MarginOrderInput['marginAmount']>>
type SecondsStakeRejectsNumber = AssertFalse<AcceptsNumber<OpenSecondsOrderInput['stakeAmount']>>
type ConvertAmountRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof requestConvertQuote>[1]>>
type LoanAmountRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof applyLoan>[0]['amount']>>
type EarnAmountRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof subscribeEarnProduct>[1]>>
type PredictionStakeRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof requestPredictionQuote>[0]['stakeAmount']>>
type SubscriptionAmountRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof subscribeNewCoin>[0]['quoteAmount']>>
type PurchasePriceRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof createNewCoinPurchase>[0]['price']>>
type UnlockFeeRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof payNewCoinUnlockFee>[0]['amount']>>
type WithdrawalAmountRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof fetchWithdrawalQuote>[0]['amount']>>
type RechargeAmountRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof createQuickRechargeOrder>[0]>>
type TransferAmountRejectsNumber = AssertFalse<AcceptsNumber<Parameters<typeof transferWalletFunds>[3]>>

export type FinanceMutationDecimalTypeChecks =
  | SpotQuantityRejectsNumber
  | SpotPriceRejectsNumber
  | MarginAmountRejectsNumber
  | SecondsStakeRejectsNumber
  | ConvertAmountRejectsNumber
  | LoanAmountRejectsNumber
  | EarnAmountRejectsNumber
  | PredictionStakeRejectsNumber
  | SubscriptionAmountRejectsNumber
  | PurchasePriceRejectsNumber
  | UnlockFeeRejectsNumber
  | WithdrawalAmountRejectsNumber
  | RechargeAmountRejectsNumber
  | TransferAmountRejectsNumber
