import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const navSource = read('../src/components/AppBottomNav.vue')
const parityCss = read('../src/styles/prototype-parity.css')
const zhCN = read('../src/i18n/messages/zh-CN.ts')
const en = read('../src/i18n/messages/en.ts')

test('中央交易入口打开 Teleport 模态层并复用共享生命周期', () => {
  assert.match(navSource, /const tradePickerOpen = ref\(false\)/)
  assert.match(navSource, /useModalDialog\(tradePickerOpen, tradePickerDialog, '\[data-trade-picker-close\]'\)/)
  assert.match(navSource, /<Teleport to="body">/)
  assert.match(navSource, /role="dialog"/)
  assert.match(navSource, /aria-modal="true"/)
  assert.match(navSource, /@keydown="handleTradePickerKeydown"/)
  assert.match(navSource, /setReturnFocus\(tradeTrigger\.value\)/)
  assert.match(navSource, /:aria-haspopup="item\.primary \? 'dialog' : undefined"/)
  assert.match(navSource, /:aria-expanded="item\.primary \? tradePickerOpen : undefined"/)
  assert.match(navSource, /@click="selectRoot\(item, \$event\)"/)
})

test('四行交易选择器保留真实路由且不渲染当前项 active 状态', () => {
  for (const value of ['spot', 'contract', 'seconds', 'swap']) {
    assert.match(navSource, new RegExp(`value: '${value}'`))
  }
  for (const icon of ['RefreshCw', 'CandlestickChart', 'Zap', 'ArrowDownUp', 'X']) {
    assert.match(navSource, new RegExp(`\\b${icon}\\b`))
  }
  assert.match(navSource, /createBottomNavSecondsTarget\(\)/)
  assert.match(navSource, /router\.replace\(createBottomNavSecondsTarget\(\)\)/)
  assert.match(navSource, /router\.replace\(createTradeTarget\('spot'\)\)/)
  assert.match(navSource, /router\.replace\(createTradeTarget\('contract'\)\)/)
  assert.match(navSource, /router\.push\(\{ name: 'swap' \}\)/)
  assert.doesNotMatch(navSource, /selectedTradeDestination|option\.active|:class="\{ active: option\.active \}"/)
  assert.doesNotMatch(navSource, /role="radiogroup"|role="radio"|aria-checked/)
  assert.doesNotMatch(navSource, /trade-navigation-picker__selection|<Check\b/)
  assert.doesNotMatch(navSource, /value:\s*'strategy'|value:\s*'options'|disabled[^>]*trade-navigation-picker__option/i)
  assert.doesNotMatch(navSource, /\p{Extended_Pictographic}/u)
})

test('Pencil X0ux9F 的 Dock、特殊轮廓、四行和关闭控制保持精确几何', () => {
  assert.match(navSource, /M26 0h306c14\.36 0 26 11\.64 26 26v252c0 12\.15-9\.85 22-22 22h-112/)
  assert.match(navSource, /viewBox="0 0 358 300"/)
  assert.match(navSource, /preserveAspectRatio="none"/)
  assert.match(parityCss, /\.bottom-nav \.trade-nav-action \.bottom-nav__icon\s*\{[\s\S]*?height:\s*56px;[\s\S]*?top:\s*-12px;[\s\S]*?width:\s*56px;/)
  assert.match(parityCss, /\.trade-navigation-picker\s*\{[\s\S]*?background:\s*rgb\(0 0 0 \/ 35%\);[\s\S]*?z-index:\s*var\(--layer-overlay\)/)
  assert.match(parityCss, /\.trade-navigation-picker__dialog\s*\{[\s\S]*?bottom:\s*calc\(35px \+ env\(safe-area-inset-bottom\)\);[\s\S]*?height:\s*347px;[\s\S]*?max-width:\s*358px;[\s\S]*?pointer-events:\s*auto;/)
  assert.match(parityCss, /\.trade-navigation-picker__shape\s*\{[\s\S]*?height:\s*300px;/)
  assert.match(parityCss, /\.trade-navigation-picker__options\s*\{[\s\S]*?left:\s*14px;[\s\S]*?top:\s*12px;/)
  assert.match(parityCss, /\.trade-navigation-picker__option\s*\{[\s\S]*?border-radius:\s*12px;[\s\S]*?gap:\s*16px;[\s\S]*?height:\s*58px;[\s\S]*?padding:\s*0 18px;/)
  assert.doesNotMatch(parityCss, /\.trade-navigation-picker__option\.active|--trade-picker-active/)
  assert.doesNotMatch(parityCss, /trade-navigation-picker__selection|--trade-picker-selection/)
  assert.match(parityCss, /\.trade-navigation-picker__close\s*\{[\s\S]*?border:\s*3px solid #fff;[\s\S]*?height:\s*54px;[\s\S]*?top:\s*293px;[\s\S]*?width:\s*54px;/)
})

test('交易选择器拥有双语标题、安全区、暗色与低动态适配', () => {
  assert.match(zhCN, /tradePickerTitle:\s*'选择交易方式'/)
  assert.match(en, /tradePickerTitle:\s*'Choose trading mode'/)
  assert.match(parityCss, /:root\[data-theme='dark'\] \.trade-navigation-picker/)
  assert.match(parityCss, /width:\s*min\(calc\(100vw - 32px\), 358px\)/)
  assert.match(parityCss, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.trade-picker-enter-active/)
})
