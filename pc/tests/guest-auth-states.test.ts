import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import assert from 'node:assert/strict'
import test from 'node:test'

const repoRoot = resolve(import.meta.dirname, '..')

function readProjectFile(path: string) {
  return readFileSync(resolve(repoRoot, path), 'utf8')
}

test('PC user center renders a guest state instead of mounting private pages', () => {
  const layout = readProjectFile('src/views/User/UserLayout.vue')
  const authState = readProjectFile('src/components/common/AuthRequiredState.vue')
  const authComposable = readProjectFile('src/composables/useAuthRequired.ts')
  const i18n = readProjectFile('src/i18n/index.ts')

  assert.match(layout, /<AuthRequiredState v-if="!isLoggedIn"/)
  assert.match(layout, /<router-view v-else/)
  assert.match(layout, /goToLogin\(\)/)
  assert.match(authState, /common\.login_required_title/)
  assert.match(authState, /common\.login_now/)
  assert.match(authComposable, /redirect:\s*route\.fullPath/)
  assert.match(authComposable, /localStorage\.getItem\('token'\)/)
  assert.match(i18n, /login_required_title:\s*'Please log in'/)
  assert.match(i18n, /login_required_title:\s*'请先登录'/)
})

test('PC swap loads public pairs without fetching private balances for guests', () => {
  const swap = readProjectFile('src/views/Swap.vue')

  assert.match(swap, /const \{ isLoggedIn, goToLogin \} = useAuthRequired\(\)/)
  assert.match(swap, /const pairResponse = await fetchSwapPairs\(\)/)
  assert.doesNotMatch(swap, /Promise\.all\(\[\s*fetchSwapPairs\(\),\s*fetchSwapBalances\(\),\s*\]\)/)
  assert.match(swap, /if \(isLoggedIn\.value\) \{\s*await refreshBalances\(\)/)
  assert.match(swap, /async function refreshBalances\(\) \{\s*if \(!isLoggedIn\.value\)/)
  assert.match(swap, /async function refreshQuote\(\) \{[\s\S]*if \(!isLoggedIn\.value\) return/)
  assert.match(swap, /async function handleSwap\(\) \{[\s\S]*goToLogin\(\)[\s\S]*return/)
  assert.match(swap, /<AuthRequiredState v-if="!isLoggedIn" compact/)
})

test('PC spot and margin trading panels guard private order and wallet APIs for guests', () => {
  const orderForm = readProjectFile('src/components/trade/OrderForm.vue')
  const orderHistory = readProjectFile('src/components/trade/OrderHistory.vue')
  const trade = readProjectFile('src/views/Trade.vue')
  const contractForm = readProjectFile('src/components/trade/ContractOrderForm.vue')
  const contractOrders = readProjectFile('src/components/trade/ContractOrders.vue')
  const contract = readProjectFile('src/views/Contract.vue')

  assert.match(orderForm, /<AuthRequiredState v-if="!isLoggedIn" compact/)
  assert.match(orderForm, /const getWallet = async \(\) => \{[\s\S]*!isLoggedIn\.value/)
  assert.match(orderForm, /const submitOrder = async \(\) => \{[\s\S]*goToLogin\(\)/)
  assert.match(orderHistory, /<AuthRequiredState v-if="!isLoggedIn" compact/)
  assert.match(orderHistory, /const loadOrders = async \(\) => \{[\s\S]*!isLoggedIn\.value/)
  assert.match(trade, /if \(isLoggedIn\.value\) \{\s*privateSub = await stompService\.subscribePrivate/)

  assert.match(contractForm, /<AuthRequiredState v-if="!isLoggedIn" compact/)
  assert.match(contractForm, /onMounted\(async \(\) => \{[\s\S]*if \(!isLoggedIn\.value\) return/)
  assert.match(contractOrders, /<AuthRequiredState v-if="!isLoggedIn" compact/)
  assert.match(contractOrders, /const loadData = async \(\) => \{[\s\S]*if \(!isLoggedIn\.value\) return/)
  assert.match(contract, /<div v-if="!isLoggedIn"[\s\S]*<AuthRequiredState/)
  assert.match(contract, /onMounted\(async \(\) => \{[\s\S]*if \(!isLoggedIn\.value\) return[\s\S]*stompService\.connect\('margin'\)/)
  assert.match(contract, /if \(isLoggedIn\.value\) \{[\s\S]*await contractStore\.loadWallets\(\)/)
  assert.match(contract, /if \(isLoggedIn\.value\) \{[\s\S]*privateSub = await stompService\.subscribePrivate/)
})

test('PC seconds page keeps public market data while guarding private orders for guests', () => {
  const seconds = readProjectFile('src/views/SecondOptions.vue')

  assert.match(seconds, /import AuthRequiredState/)
  assert.match(seconds, /const \{ isLoggedIn, goToLogin \} = useAuthRequired\(\)/)
  assert.match(seconds, /const loadOrders = async \(\) => \{[\s\S]*if \(!isLoggedIn\.value\)/)
  assert.match(seconds, /const handleOrder = async \(direction: 0 \| 1\) => \{[\s\S]*goToLogin\(\)/)
  assert.match(seconds, /<div v-if="!isLoggedIn"[\s\S]*<AuthRequiredState/)
  assert.match(seconds, /onMounted\(async \(\) => \{[\s\S]*if \(!isLoggedIn\.value\) return[\s\S]*stompService\.connect\('seconds'\)/)
  assert.match(seconds, /if \(isLoggedIn\.value\) \{[\s\S]*store\.loadBalance\(\)/)
  assert.match(seconds, /if \(isLoggedIn\.value\) \{[\s\S]*privateSub = await stompService\.subscribePrivate/)
  assert.match(seconds, /<AuthRequiredState v-if="!isLoggedIn" compact/)
})
