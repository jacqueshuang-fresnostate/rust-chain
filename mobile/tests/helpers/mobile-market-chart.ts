import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import ts from 'typescript'
import { compileScript, parse } from '@vue/compiler-sfc'
import * as Vue from 'vue'
import { createI18n, useI18n } from 'vue-i18n'
import en from '../../src/i18n/messages/en.ts'
import zhCN from '../../src/i18n/messages/zh-CN.ts'
import { createMarketChartHistorySession } from '../../src/core/marketChartHistory.ts'
import { calculateMarketMovingAverages } from '../../src/core/marketIndicators.ts'
import type { KlinePoint, MarketTicker } from '../../src/core/types.ts'

interface HostNode {
  type: string
  text: string
  props: Record<string, unknown>
  children: HostNode[]
  parent: HostNode | null
}

function hostNode(type: string, text = ''): HostNode {
  return { type, text, props: {}, children: [], parent: null }
}

/** Compile/render the actual wrapper SFC; fake only host DOM, icons, renderer and HTTP I/O. */
export function mountMobileMarketChart(loadOlder: (symbol: string, interval: string, before: number) => Promise<KlinePoint[]>, points: KlinePoint[]) {
  const renderer = Vue.createRenderer<HostNode, HostNode>({
    createElement: (type) => hostNode(type),
    createText: (text) => hostNode('#text', text),
    createComment: (text) => hostNode('#comment', text),
    setText: (node, text) => { node.text = text },
    setElementText: (node, text) => { node.text = text; node.children = [] },
    parentNode: (node) => node.parent,
    nextSibling: (node) => node.parent?.children[(node.parent?.children.indexOf(node) ?? -1) + 1] ?? null,
    patchProp: (node, key, _previous, next) => { node.props[key] = next },
    remove(node) {
      if (node.parent) node.parent.children = node.parent.children.filter((child) => child !== node)
      node.parent = null
    },
    insert(node, parent, anchor = null) {
      if (node.parent) node.parent.children = node.parent.children.filter((child) => child !== node)
      node.parent = parent
      const index = anchor ? parent.children.indexOf(anchor) : -1
      if (index < 0) parent.children.push(node)
      else parent.children.splice(index, 0, node)
    },
  })
  const icon = Vue.defineComponent({ setup: () => () => Vue.h('icon') })
  const chartStub = Vue.defineComponent({
    props: ['points', 'movingAverages', 'symbol', 'interval', 'historyLoading', 'locale', 'label'],
    emits: ['load-history'],
    setup: (props, { emit }) => () => Vue.h('chart-engine', { ...props, onDemand: () => emit('load-history') }),
  })
  const modules: Record<string, Record<string, unknown>> = {
    vue: Vue,
    'vue-i18n': { useI18n },
    'lucide-vue-next': { ChartNoAxesCombined: icon, LoaderCircle: icon },
    '@/components/LightweightMarketChart.vue': { default: chartStub },
    '@/api/market': { fetchOlderKlines: loadOlder },
    '@/core/marketChartHistory': { createMarketChartHistorySession },
    '@/core/marketIndicators': { calculateMarketMovingAverages },
  }
  const source = readFileSync(new URL('../../src/components/MobileMarketChart.vue', import.meta.url), 'utf8')
  const { descriptor } = parse(source)
  const compiled = compileScript(descriptor, { id: 'mobile-chart-test', inlineTemplate: true })
  const syntax = ts.createSourceFile('mobile-chart.ts', compiled.content, ts.ScriptTarget.Latest, true)
  const bindings: Record<string, unknown> = {}
  for (const statement of syntax.statements) {
    if (!ts.isImportDeclaration(statement) || !statement.importClause || statement.importClause.isTypeOnly) continue
    const moduleName = (statement.moduleSpecifier as ts.StringLiteral).text
    const values = modules[moduleName]
    assert.ok(values, `Missing test I/O binding: ${moduleName}`)
    if (statement.importClause.name) bindings[statement.importClause.name.text] = values.default
    const named = statement.importClause.namedBindings
    if (named && ts.isNamedImports(named)) {
      for (const item of named.elements) {
        if (!item.isTypeOnly) bindings[item.name.text] = values[(item.propertyName ?? item.name).text]
      }
    }
  }
  const body = syntax.statements.filter((node) => !ts.isImportDeclaration(node)).map((node) => node.getText(syntax)).join('\n')
  const output = ts.transpileModule(body.replace('export default ', 'return '), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
  }).outputText
  const component = new Function(...Object.keys(bindings), output)(...Object.values(bindings)) as Vue.Component
  const props = Vue.reactive({ marketType: undefined as MarketTicker['marketType'], symbol: 'BTCUSDT', interval: '1m', points, loading: false })
  const i18n = createI18n({ legacy: false, locale: 'zh-CN', messages: { en, 'zh-CN': zhCN } })
  const root = hostNode('root')
  const app = renderer.createApp({ render: () => Vue.h(component, props) })
  app.use(i18n)
  app.mount(root)

  function all(node: HostNode = root): HostNode[] {
    return [node, ...node.children.flatMap((child) => all(child))]
  }
  return {
    root,
    props,
    i18n,
    all,
    find: (type: string) => all().find((node) => node.type === type)!,
    chart: () => all().find((node) => node.type === 'chart-engine')!,
    text: () => all().filter((node) => node.type !== '#comment').map((node) => node.text).join(' '),
    async flush() { await Promise.resolve(); await Vue.nextTick() },
    async update(next: Partial<typeof props>) { Object.assign(props, next); await Vue.nextTick() },
    unmount: () => app.unmount(),
  }
}
