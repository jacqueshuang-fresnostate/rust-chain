import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('auth cards render the brand logo without the platform name span', () => {
  for (const path of [
    'src/views/auth/Login.vue',
    'src/views/auth/Register.vue',
    'src/views/auth/ForgotPassword.vue',
  ]) {
    const source = readProjectFile(path)
    assert.match(source, /<BrandLogo\b/)
    assert.doesNotMatch(source, /<BrandLogo[^>]*\bshow-name\b/)
    assert.doesNotMatch(source, /<BrandLogo[^>]*\bname-class=/)
  }

  const brandLogoSource = readProjectFile('src/components/common/BrandLogo.vue')
  assert.match(brandLogoSource, /<span v-if="showName"/)
})

test('header brand logo does not render the platform name span', () => {
  const source = readProjectFile('src/components/layout/Header.vue')

  assert.match(source, /<BrandLogo\b/)
  assert.doesNotMatch(source, /<BrandLogo[^>]*\bshow-name\b/)
  assert.doesNotMatch(source, /<BrandLogo[^>]*\bname-class=/)
})

test('header uses a compact exchange navigation structure', () => {
  const source = readProjectFile('src/components/layout/Header.vue')
  const i18nSource = readProjectFile('src/i18n/index.ts')

  assert.match(source, /data-pc-header/)
  assert.match(source, /data-pc-header-trade-menu/)
  assert.match(source, /primaryNavItems/)
  assert.match(source, /dropdownTickers/)
  assert.match(source, /productItemClass\('spot'\)/)
  assert.match(source, /router\.push\('\/spot'\)/)
  assert.match(source, /goToTrade\(ticker\.symbol\)/)
  assert.doesNotMatch(source, /drop-shadow-neon|text-glow|box-glow/)

  for (const key of ['spot_desc', 'swap_desc', 'binary_desc', 'contract_desc', 'top_pairs', 'logout']) {
    assert.match(i18nSource, new RegExp(`${key}:`))
  }
})
