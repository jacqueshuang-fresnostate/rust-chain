import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('register country selector is searchable and keeps the country code contract', () => {
  const registerSource = readProjectFile('src/views/auth/Register.vue')
  const i18nSource = readProjectFile('src/i18n/index.ts')

  assert.doesNotMatch(registerSource, /<select[\s\S]*form\.countryCode/)
  assert.match(registerSource, /type="search"/)
  assert.match(registerSource, /v-model="countrySearch"/)
  assert.match(registerSource, /filteredCountryOptions/)
  assert.match(registerSource, /country\.name\.toLowerCase\(\)\.includes\(keyword\)/)
  assert.match(registerSource, /country\.code\.toLowerCase\(\)\.includes\(keyword\)/)
  assert.match(registerSource, /selectCountry\(country\.code\)/)
  assert.match(registerSource, /countryCode:\s*form\.value\.countryCode/)
  assert.match(i18nSource, /register_search_country/)
  assert.match(i18nSource, /register_no_country_matches/)
})
