import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  detectPerformanceTier,
  resolvePerformanceTier,
} from '../src/core/performanceTier.ts'

const mainSource = readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8')

test('设备性能档位综合节流、内存和核心数并对缺失 API 安全回退', () => {
  assert.equal(resolvePerformanceTier({}), 'standard')
  assert.equal(resolvePerformanceTier({ saveData: false, deviceMemory: 8, hardwareConcurrency: 8 }), 'standard')
  assert.equal(resolvePerformanceTier({ saveData: true, deviceMemory: 8, hardwareConcurrency: 8 }), 'constrained')
  assert.equal(resolvePerformanceTier({ deviceMemory: 4, hardwareConcurrency: 8 }), 'standard')
  assert.equal(resolvePerformanceTier({ deviceMemory: 8, hardwareConcurrency: 4 }), 'standard')
  assert.equal(resolvePerformanceTier({ deviceMemory: 4, hardwareConcurrency: 4 }), 'constrained')
  assert.equal(resolvePerformanceTier({ deviceMemory: 2, hardwareConcurrency: 8 }), 'constrained')
  assert.equal(resolvePerformanceTier({ deviceMemory: 8, hardwareConcurrency: 2 }), 'constrained')
  assert.equal(resolvePerformanceTier({ deviceMemory: 0, hardwareConcurrency: Number.NaN }), 'standard')
})

test('Navigator 可选字段读取不会依赖非标准 API 一定存在', () => {
  assert.equal(detectPerformanceTier(null), 'standard')
  assert.equal(detectPerformanceTier({}), 'standard')
  assert.equal(detectPerformanceTier({ connection: { saveData: true } }), 'constrained')
})

test('应用在 mount 前把档位写入 html dataset', () => {
  const assignment = mainSource.indexOf('document.documentElement.dataset.performanceTier = detectPerformanceTier(globalThis.navigator)')
  const mount = mainSource.indexOf("createApp(App).use(createPinia()).use(router).use(i18n).mount('#app')")
  assert.ok(assignment >= 0)
  assert.ok(mount > assignment)
})
