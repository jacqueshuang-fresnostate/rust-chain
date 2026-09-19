import test from 'node:test'
import assert from 'node:assert/strict'
import { parseSpotTriggerSnapshot } from '../src/core/spotTrigger.ts'

test('spot trigger readback preserves direction, exact decimals and server activation', () => {
  for (const direction of ['rising', 'falling']) {
    assert.deepEqual(parseSpotTriggerSnapshot({
      order_type: 'stop_limit',
      trigger_price: '1.234567890123456789',
      trigger_direction: direction,
      triggered_at: 1_789_718_400_000,
    }), {
      triggerPriceText: '1.234567890123456789',
      triggerDirection: direction,
      triggeredAt: 1_789_718_400_000,
    })
  }
  assert.deepEqual(parseSpotTriggerSnapshot({
    order_type: 'stop_limit', trigger_price: '10', trigger_direction: null, triggered_at: null,
  }), { triggerPriceText: '10', triggerDirection: null, triggeredAt: null })
  assert.deepEqual(parseSpotTriggerSnapshot({ order_type: 'limit' }), {
    triggerPriceText: null, triggerDirection: null, triggeredAt: null,
  })
})

test('malformed trigger metadata is not silently presented as legacy or untriggered', () => {
  const valid = { order_type: 'stop_limit', trigger_price: '10', trigger_direction: 'rising' }
  for (const invalid of [
    { ...valid, trigger_direction: 'unknown' },
    { ...valid, triggered_at: '1789718400000' },
    { ...valid, triggered_at: -1 },
    { ...valid, trigger_price: null },
    { ...valid, trigger_price: '0' },
    { ...valid, trigger_direction: null, triggered_at: 1_789_718_400_000 },
  ]) assert.throws(() => parseSpotTriggerSnapshot(invalid))
})
