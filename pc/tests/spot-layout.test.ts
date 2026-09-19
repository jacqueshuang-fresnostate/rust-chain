import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { parse as parseSfc } from '@vue/compiler-sfc'
import { parse, NodeTypes, type ElementNode, type RootNode } from '@vue/compiler-dom'

const source = readFileSync(new URL('../src/views/Trade.vue', import.meta.url), 'utf8')
const { descriptor } = parseSfc(source)
const template = parse(descriptor.template!.content)

function ancestorsOf(tag: string, node: RootNode | ElementNode = template, parents: ElementNode[] = []): ElementNode[] {
  for (const child of node.children) {
    if (child.type !== NodeTypes.ELEMENT) continue
    if (child.tag === tag) return parents
    const result = ancestorsOf(tag, child, [...parents, child])
    if (result.length) return result
  }
  return []
}

function classes(node: ElementNode) {
  const attribute = node.props.find(prop => prop.type === NodeTypes.ATTRIBUTE && prop.name === 'class')
  return new Set(attribute?.type === NodeTypes.ATTRIBUTE ? attribute.value?.content.split(/\s+/) : [])
}

test('spot chart column can shrink on desktop and cannot collapse over the stacked form', () => {
  const ancestors = ancestorsOf('MarketChart')
  assert.ok(ancestors.length)
  const chartPanel = classes(ancestors.at(-1)!)
  const column = classes(ancestors.at(-2)!)
  for (const value of ['min-w-0', 'min-h-0', 'flex-1']) assert.ok(chartPanel.has(value), value)
  for (const value of ['min-w-0', 'min-h-0', 'h-[660px]', 'lg:h-full', 'lg:flex-1', 'shrink-0', 'overflow-hidden']) {
    assert.ok(column.has(value), value)
  }
  const historyPanel = classes(ancestorsOf('OrderHistory').at(-1)!)
  for (const value of ['h-[260px]', 'shrink-0', 'min-w-0', 'min-h-0']) assert.ok(historyPanel.has(value), value)
})

test('spot form remains reachable through stacked scrolling and independent desktop scrolling', () => {
  const ancestors = ancestorsOf('OrderForm')
  assert.ok(ancestors.length)
  const column = classes(ancestors.at(-2)!)
  const workspace = classes(ancestors.at(-3)!)
  for (const value of ['min-w-0', 'lg:min-h-0', 'lg:w-[340px]', 'lg:overflow-y-auto', 'shrink-0']) {
    assert.ok(column.has(value), value)
  }
  for (const value of ['min-w-0', 'min-h-0', 'overflow-y-auto', 'lg:flex-row']) assert.ok(workspace.has(value), value)
  assert.ok(!column.has('overflow-hidden'))
})
