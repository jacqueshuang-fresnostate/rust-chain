import assert from 'node:assert/strict'
import test from 'node:test'
import { secondsOrderStatusPresentation, secondsOrderProfitLossPresentation } from '../src/core/secondsOrder.ts'
import { walletLedgerTypePresentation } from '../src/core/walletLedger.ts'
import { requiredDecimalText } from '../src/core/decimal.ts'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'

test('principal refunds have a neutral terminal label and no invented win or loss', () => {
  assert.deepEqual(secondsOrderStatusPresentation({ status: 'refunded' }), {
    translationKey: 'seconds.statusRefunded', source: 'refunded', tone: 'pending',
  })
  const pnl = secondsOrderProfitLossPresentation({
    result: undefined, stakeAmountText: requiredDecimalText('10', 'stake', 'refund test'), payoutRateText: requiredDecimalText('0.8', 'rate', 'refund test'),
  })
  assert.equal(pnl.amountText, null)
  assert.equal(pnl.amount, undefined)
  assert.equal(zhCN.seconds.statusRefunded, '已退还本金')
  assert.equal(en.seconds.statusRefunded, 'Principal refunded')
})

test('wallet refund classification preserves principal-only bilingual meaning', () => {
  assert.equal(walletLedgerTypePresentation('seconds_contract_principal_refund').translationKey, 'ledger.typeSecondsPrincipalRefund')
  assert.equal(zhCN.ledger.typeSecondsPrincipalRefund, '秒合约本金退还')
  assert.equal(en.ledger.typeSecondsPrincipalRefund, 'Seconds principal refund')
})
