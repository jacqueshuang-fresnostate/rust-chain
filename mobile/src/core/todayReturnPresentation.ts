import { formatAmount, formatPercent } from './format.ts'
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
    const sign = input.value.amount > 0 ? '+' : ''
    return {
      amount: `${sign}${formatAmount(input.value.amount)} ${input.value.reportingAsset}`,
      detail: formatPercent(input.value.rate * 100),
      tone: input.value.amount > 0 ? 'positive' : input.value.amount < 0 ? 'negative' : '',
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
