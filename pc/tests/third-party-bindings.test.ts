import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('PC security center wires third-party binding policy and actions', () => {
  const apiSource = readProjectFile('src/api/user.ts')
  const securitySource = readProjectFile('src/views/User/Security.vue')
  const i18nSource = readProjectFile('src/i18n/index.ts')

  assert.match(apiSource, /ThirdPartyBindingPolicy/)
  assert.match(apiSource, /getThirdPartyBindings/)
  assert.match(apiSource, /bindThirdPartyAccount/)
  assert.match(apiSource, /backendApiUrl\('\/user\/third-party-bindings'\)/)
  assert.match(apiSource, /third_party_bindings: ThirdPartyBindingPolicy/)

  assert.match(securitySource, /getThirdPartyBindings/)
  assert.match(securitySource, /bindThirdPartyAccount/)
  assert.match(securitySource, /coinbase_wallet/)
  assert.match(securitySource, /telegram_account/)
  assert.match(securitySource, /showThirdPartyModal/)
  assert.match(securitySource, /thirdPartyPolicy/)
  assert.match(securitySource, /v-if="coinbaseEnabled"/)
  assert.match(securitySource, /v-if="telegramEnabled"/)
  assert.doesNotMatch(securitySource, /security\.not_supported/)
  assert.doesNotMatch(securitySource, /wallet_bind_unavailable/)
  assert.doesNotMatch(securitySource, /withdraw_verification/)
  assert.doesNotMatch(securitySource, /paymentPolicyLabel/)

  for (const key of [
    'telegram',
    'not_supported',
    'bind_coinbase_wallet',
    'bind_telegram_account',
    'coinbase_identifier_placeholder',
    'telegram_identifier_placeholder',
    'third_party_disabled',
    'third_party_bind_success',
  ]) {
    assert.match(i18nSource, new RegExp(`${key}:`))
  }
})
