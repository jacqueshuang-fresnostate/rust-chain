import {
  decimalMultiply,
  decimalSign,
  normalizeDecimalText,
} from './decimal.ts'
import { formatFinancialAmount } from './financialDisplay.ts'
import { isCompleteTodayReturn, type TodayReturn } from './todayReturn.ts'

export type TodayReturnViewState = 'idle' | 'loading' | 'complete' | 'partial' | 'error'
export type TodayReturnTone = 'positive' | 'negative' | ''

export interface TodayReturnPresentation {
  amount: string
  detail: string
  tone: TodayReturnTone
}

export function resolveTodayReturnPresentation(input: {
  visible: boolean
  state: TodayReturnViewState
  value: TodayReturn | null
  amountMask: string
  detailMask: string
  locale?: string
  messages: {
    loading: string
    partial: (assets: string) => string
    partialUnknown: string
    error: string
  }
}): TodayReturnPresentation {
  if (!input.visible) {
    return { amount: input.amountMask, detail: input.detailMask, tone: '' }
  }

  if (input.state === 'complete' && isCompleteTodayReturn(input.value)) {
    const amountSign = decimalSign(input.value.amount)
    const locale = input.locale || 'en-US'
    const amount = formatFinancialAmount(input.value.amount, locale, {
      assetSymbol: input.value.reportingAsset,
    })
    return {
      amount: `${amountSign > 0 && !amount.startsWith('<') ? '+' : ''}${amount} ${input.value.reportingAsset}`,
      detail: `${formatFinancialAmount(
        decimalMultiply(input.value.rate, normalizeDecimalText('100')),
        locale,
        { maximumFractionDigits: 2, useGrouping: false },
      )}%`,
      tone: amountSign > 0 ? 'positive' : amountSign < 0 ? 'negative' : '',
    }
  }

  if (input.state === 'loading') {
    return { amount: '--', detail: input.messages.loading, tone: '' }
  }
  if (input.state === 'partial') {
    const assets = input.value?.missingPriceAssets.join(', ')
    return {
      amount: '--',
      detail: assets ? input.messages.partial(assets) : input.messages.partialUnknown,
      tone: '',
    }
  }
  if (input.state === 'error') {
    return { amount: '--', detail: input.messages.error, tone: '' }
  }
  return { amount: '--', detail: '--', tone: '' }
}
