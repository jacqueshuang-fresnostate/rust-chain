// Mechanical AST-guided field annotation. Requires the compiled dto_inventory tool.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import { spawnSync } from 'node:child_process'

const result = spawnSync('/private/tmp/rust-chain-numeric-dto-inventory', [], { encoding: 'utf8' })
assert.equal(result.status, 0, result.stderr)
const nested = new Set([
  'WithdrawFeeTier', 'WithdrawalAllowance', 'WithdrawalReviewTier',
  'DefaultMarketParameters', 'DefaultMarketFollowParameters',
])
const edits = new Map()
for (const row of result.stdout.trim().split('\n')) {
  const [file, structure, field, depthText, declarationText, attrsText] = row.split('|')
  const presentationInput = file.includes('presentation')
    && !structure.endsWith('Response') && !structure.includes('Cache')
  if (!presentationInput && !nested.has(structure)) continue
  const depth = Number(depthText)
  assert.ok(depth <= 2, `${structure}.${field}`)
  const helper = ['deserialize_decimal', 'deserialize_optional_decimal', 'deserialize_patch_decimal'][depth]
  const lines = fs.readFileSync(file, 'utf8').split('\n')
  const attrs = attrsText ? attrsText.split(',').map((range) => range.split('-').map(Number)) : []
  assert.ok(attrs.length <= 1, `Multiple serde attributes: ${structure}.${field}`)
  const declaration = Number(declarationText) - 1
  const indent = lines[declaration].match(/^\s*/)[0]
  let start = declaration
  let length = 0
  let options = []
  if (attrs.length) {
    const [from, to] = attrs[0]
    start = from - 1
    length = to - from + 1
    const source = lines.slice(start, start + length).join('\n')
    if (source.includes(`crate::numeric::${helper}`)) continue
    const match = source.match(/#\[serde\(([\s\S]*)\)\]/)
    assert.ok(match, source)
    const args = match[1]
    assert.ok(!args.includes('deserialize_with') || args.includes('"double_option"'), source)
    options = args.split(',').map((item) => item.trim()).filter(Boolean)
      .filter((item) => !item.startsWith('deserialize_with'))
  }
  if (depth > 0 && !options.some((item) => item === 'default' || item.startsWith('default ='))) {
    options.unshift('default')
  }
  options.push(`deserialize_with = "crate::numeric::${helper}"`)
  const replacement = `${indent}#[serde(${options.join(', ')})]`
  const list = edits.get(file) || []
  list.push({ start, length, replacement, structure, field })
  edits.set(file, list)
}
let total = 0
for (const [file, changes] of edits) {
  const lines = fs.readFileSync(file, 'utf8').split('\n')
  for (const change of changes.sort((left, right) => right.start - left.start)) {
    lines.splice(change.start, change.length, change.replacement)
    total++
  }
  fs.writeFileSync(file, lines.join('\n'))
  console.log(`${file}: ${changes.length} fields`)
}
console.log(`Annotated ${total} request/config fields without altering field types or response serialization.`)
