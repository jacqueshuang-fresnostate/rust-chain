import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('transaction history date range uses a localized datetime popup', () => {
  const viewSource = readProjectFile('src/views/User/Transaction.vue')
  const apiSource = readProjectFile('src/api/transaction.ts')
  const i18nSource = readProjectFile('src/i18n/index.ts')

  assert.doesNotMatch(viewSource, /type="date"/)
  assert.match(viewSource, /dateRangePickerOpen/)
  assert.match(viewSource, /role="dialog"/)
  assert.match(viewSource, /type="datetime-local"/)
  assert.match(viewSource, /draftStartTime/)
  assert.match(viewSource, /draftEndTime/)
  assert.match(viewSource, /date_range_invalid/)
  assert.match(viewSource, /window\.addEventListener\('click', handleDateRangeOutsideClick\)/)
  assert.match(apiSource, /normalizeTransactionDateTimeFilter/)
  assert.match(apiSource, /trimmed\.replace\('T', ' '\)/)
  assert.match(apiSource, /23:59:59/)
  assert.match(i18nSource, /select_date_time_range/)
  assert.match(i18nSource, /start_time/)
  assert.match(i18nSource, /end_time/)
  assert.match(i18nSource, /date_range_invalid/)
})
