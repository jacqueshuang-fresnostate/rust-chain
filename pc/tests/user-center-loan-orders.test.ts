import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import test from 'node:test'

function readProjectFile(path: string) {
  return readFileSync(resolve(import.meta.dirname, '..', path), 'utf8')
}

test('user center sidebar exposes loan and prediction order entries', () => {
  const layoutSource = readProjectFile('src/views/User/UserLayout.vue')
  const routerSource = readProjectFile('src/router/index.ts')
  const i18nSource = readProjectFile('src/i18n/index.ts')

  assert.match(layoutSource, /to="\/user\/loan-orders"/)
  assert.match(layoutSource, /nav\.loan_orders/)
  assert.match(routerSource, /path:\s*'loan-orders'/)
  assert.match(i18nSource, /loan_orders:\s*'Loan Orders'/)
  assert.match(i18nSource, /loan_orders:\s*'贷款订单'/)

  assert.match(layoutSource, /to="\/user\/prediction-orders"/)
  assert.match(layoutSource, /nav\.prediction_orders/)
  assert.match(routerSource, /path:\s*'prediction-orders'/)
  assert.match(i18nSource, /prediction_orders:\s*'Prediction Orders'/)
  assert.match(i18nSource, /prediction_orders:\s*'竞猜订单'/)
})
