import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')

function read(relativePath: string) {
  return readFileSync(resolve(root, relativePath), 'utf8')
}

test('authenticated backend requests refresh access token once before forcing relogin', () => {
  const requestSource = read('src/api/request.ts')
  const authStorageSource = read('src/utils/authStorage.ts')

  assert.match(requestSource, /axios\.post<BackendAuthTokenResponse>\(\s*`\$\{backendBaseUrl\(\)\}\/auth\/refresh`/)
  assert.match(requestSource, /const refreshToken = readRefreshToken\(\)/)
  assert.match(requestSource, /writeAuthTokens\(nextToken, nextRefreshToken\)/)
  assert.match(requestSource, /const token = readAuthToken\(\)/)
  assert.match(requestSource, /function refreshAccessTokenOnce\(\)/)
  assert.match(requestSource, /tokenRefreshPromise = refreshAccessToken\(\)\.finally/)
  assert.match(requestSource, /_authRetry/)
  assert.match(requestSource, /setHeader\(retryConfig, 'Authorization', createAuthorizationHeader\(nextToken\)\)/)
  assert.match(requestSource, /return this\.instance\.request\(retryConfig\)/)
  assert.match(authStorageSource, /const USER_STORE_KEY = 'user'/)
  assert.match(authStorageSource, /JSON\.parse\(raw\)/)
  assert.match(authStorageSource, /readPersistedUserStore\(\)\?\.token/)
  assert.match(authStorageSource, /readPersistedUserStore\(\)\?\.refreshToken/)
})

test('auth bootstrap routes do not recursively use refresh retry on 401', () => {
  const requestSource = read('src/api/request.ts')

  assert.match(requestSource, /function isAuthSessionRoute\(url\?: string\)/)
  assert.match(requestSource, /'\/auth\/login'/)
  assert.match(requestSource, /'\/auth\/login\/2fa'/)
  assert.match(requestSource, /'\/auth\/register'/)
  assert.match(requestSource, /'\/auth\/refresh'/)
  assert.match(requestSource, /isAuthSessionRoute\(config\.url\)/)
})
