import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('transaction history uses backend wallet ledger change_type strings', () => {
  const apiSource = readProjectFile('src/api/transaction.ts')
  const viewSource = readProjectFile('src/views/User/Transaction.vue')
  const adapterSource = readProjectFile('src/api/backendAdapters.ts')
  const i18nSource = readProjectFile('src/i18n/index.ts')

  assert.match(apiSource, /WALLET_LEDGER_TRANSACTION_TYPES/)
  assert.doesNotMatch(apiSource, /RECHARGE\s*=\s*0/)
  assert.doesNotMatch(viewSource, /TransactionType\.(?:OTC_BUY|OTC_SELL|TRANSFER)/)
  assert.match(viewSource, /getAmountColor\(item\.amount\)/)
  assert.match(adapterSource, /type:\s*entry\.change_type/)
  assert.doesNotMatch(adapterSource, /transactionTypeForRef/)
  assert.match(apiSource, /offset:\s*\(pageNo - 1\) \* pageSize/)
  assert.match(apiSource, /change_type:\s*params\.type/)
  assert.doesNotMatch(apiSource, /limit:\s*100/)
  assert.doesNotMatch(apiSource, /\.filter\(/)
  assert.doesNotMatch(apiSource, /\.slice\(/)
  assert.match(i18nSource, /type_quick_recharge/)
  assert.match(i18nSource, /type_convert_settlement/)
  assert.match(i18nSource, /type_spot_trade_settlement/)
  assert.match(i18nSource, /type_loan_disbursement/)
  assert.match(apiSource, /loan_repayment/)
})
