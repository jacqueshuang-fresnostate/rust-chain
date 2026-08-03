import assert from 'node:assert/strict'
import test from 'node:test'
import {
  observeMarketChartTheme,
  type MarketChartThemeObserver,
} from '../src/core/marketChartTheme.ts'

class FakeMutationObserver implements MarketChartThemeObserver {
  private readonly observations = new Map<Node, Set<string>>()
  private readonly callback: MutationCallback
  private disconnected = false
  disconnectCalls = 0

  constructor(callback: MutationCallback) {
    this.callback = callback
  }

  observe(target: Node, options: MutationObserverInit = {}): void {
    this.observations.set(target, new Set(options.attributeFilter || []))
  }

  disconnect(): void {
    this.disconnectCalls += 1
    this.disconnected = true
    this.observations.clear()
  }

  attributeFilterFor(target: Node): string[] {
    return [...(this.observations.get(target) || [])]
  }

  notify(target: Node, attributeName: string): void {
    if (this.disconnected || !this.observations.get(target)?.has(attributeName)) return
    this.callback(
      [{ attributeName, target, type: 'attributes' } as MutationRecord],
      this as unknown as MutationObserver,
    )
  }
}

test('图表在根级主题先变、app-stage class 后变时最终应用真实主题并在卸载后停止响应', () => {
  const stageState = { className: 'app-stage theme-light' }
  const rootState = { theme: 'light' }
  const stage = stageState as unknown as Element
  const documentRoot = rootState as unknown as Element
  const container = {
    closest(selector: string) {
      assert.equal(selector, '.app-stage')
      return stage
    },
  } as unknown as Element

  let observer: FakeMutationObserver | undefined
  const appliedStageClasses: string[] = []
  const stop = observeMarketChartTheme(
    container,
    documentRoot,
    () => appliedStageClasses.push(stageState.className),
    (callback) => {
      observer = new FakeMutationObserver(callback)
      return observer
    },
  )

  assert.ok(observer)
  assert.deepEqual(observer.attributeFilterFor(stage), ['class'])
  assert.deepEqual(observer.attributeFilterFor(documentRoot), ['data-theme'])

  rootState.theme = 'dark'
  observer.notify(documentRoot, 'data-theme')
  assert.deepEqual(appliedStageClasses, ['app-stage theme-light'])

  stageState.className = 'app-stage theme-dark'
  observer.notify(stage, 'class')
  assert.deepEqual(appliedStageClasses, [
    'app-stage theme-light',
    'app-stage theme-dark',
  ])

  stop()
  stageState.className = 'app-stage theme-light'
  observer.notify(stage, 'class')
  assert.equal(observer.disconnectCalls, 1)
  assert.deepEqual(appliedStageClasses, [
    'app-stage theme-light',
    'app-stage theme-dark',
  ])
})
