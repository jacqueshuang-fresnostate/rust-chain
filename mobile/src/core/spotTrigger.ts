import { requiredDecimalText, type DecimalText } from './decimal.ts'

export interface SpotTriggerSnapshot {
  triggerPriceText: DecimalText | null
  triggerDirection: 'rising' | 'falling' | null
  triggeredAt: number | null
}

export function parseSpotTriggerSnapshot(order: Record<string, unknown>): SpotTriggerSnapshot {
  const direction = order.trigger_direction ?? null
  if (direction !== null && direction !== 'rising' && direction !== 'falling') {
    throw new TypeError('Invalid spot trigger direction')
  }
  const triggeredAt = order.triggered_at ?? null
  if (triggeredAt !== null && (
    direction === null || typeof triggeredAt !== 'number'
    || !Number.isSafeInteger(triggeredAt) || triggeredAt <= 0
  )) {
    throw new TypeError('Invalid spot trigger timestamp')
  }
  const triggerPriceText = order.trigger_price == null ? null : requiredDecimalText(
    order.trigger_price, 'trigger_price', 'spot order',
    { allowNegative: false, allowZero: false, maxIntegerDigits: 20, maxScale: 18 },
  )
  if (order.order_type === 'stop_limit' && triggerPriceText === null) {
    throw new TypeError('Missing spot trigger price')
  }
  return { triggerPriceText, triggerDirection: direction, triggeredAt }
}
