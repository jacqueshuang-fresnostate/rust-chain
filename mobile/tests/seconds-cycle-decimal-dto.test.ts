import assert from 'node:assert/strict'
import test from 'node:test'
import { createServer } from 'vite'

interface MappedSecondsCycle {
  payoutRate: number
  payoutRateText: string
  minStake: number
  minStakeText: string
  maxStake?: number
  maxStakeText: string | null
}

test('SecondsCycle keeps exact string authority and rejects JSON-number finance fields', async (context) => {
  const server = await createServer({
    appType: 'custom',
    logLevel: 'silent',
    server: { middlewareMode: true },
  })
  context.after(async () => server.close())
  const seconds = await server.ssrLoadModule('/src/api/seconds.ts') as {
    mapSecondsCycle: (value: Record<string, unknown>) => MappedSecondsCycle
  }

  const cycle = seconds.mapSecondsCycle({
    id: 1,
    duration_seconds: 30,
    payout_rate: ' 0.000000000000000001 ',
    min_stake: '9007199254740993.000000000000000001',
    max_stake: '9007199254740993.000000000000000002',
  })
  assert.equal(cycle.payoutRateText, '0.000000000000000001')
  assert.equal(cycle.minStakeText, '9007199254740993.000000000000000001')
  assert.equal(cycle.maxStakeText, '9007199254740993.000000000000000002')
  assert.equal(
    seconds.mapSecondsCycle({
      id: 2,
      duration_seconds: 60,
      payout_rate: '0.8',
      min_stake: '1',
      max_stake: null,
    }).maxStakeText,
    null,
  )

  const valid = {
    id: 3,
    duration_seconds: 60,
    payout_rate: '0.8',
    min_stake: '1',
    max_stake: '10',
  }
  assert.throws(() => seconds.mapSecondsCycle({ ...valid, payout_rate: 0.8 }), /payout_rate/)
  assert.throws(() => seconds.mapSecondsCycle({ ...valid, min_stake: 1 }), /min_stake/)
  assert.throws(() => seconds.mapSecondsCycle({ ...valid, max_stake: 10 }), /max_stake/)
  assert.throws(() => seconds.mapSecondsCycle({ ...valid, payout_rate: '1e-18' }), /payout_rate/)
})
