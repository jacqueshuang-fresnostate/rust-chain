import {
  decimalMultiply,
  decimalSign,
  formatDecimalText,
  normalizeDecimalText,
} from './decimal.ts'
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
    return {
      amount: `${amountSign > 0 ? '+' : ''}${formatDecimalText(input.value.amount, input.locale || 'en-US', {
        maximumFractionDigits: 18,
      })} ${input.value.reportingAsset}`,
      detail: `${formatDecimalText(
        decimalMultiply(input.value.rate, normalizeDecimalText('100')),
        input.locale || 'en-US',
        { maximumFractionDigits: 8 },
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
