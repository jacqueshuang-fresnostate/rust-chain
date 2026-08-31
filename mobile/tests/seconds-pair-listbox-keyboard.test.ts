import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  isRovingListboxSelectionKey,
  moveRovingOptionId,
  stableRovingOptionId,
} from '../src/core/rovingListbox.ts'

const secondsSource = readFileSync(new URL('../src/views/SecondsView.vue', import.meta.url), 'utf8')

test('Seconds pair listbox supports deterministic wrap, boundary, and selection keys', () => {
  const ids = [11, 22, 33] as const

  assert.equal(moveRovingOptionId(ids, 11, 'ArrowDown'), 22)
  assert.equal(moveRovingOptionId(ids, 33, 'ArrowDown'), 11)
  assert.equal(moveRovingOptionId(ids, 33, 'ArrowUp'), 22)
  assert.equal(moveRovingOptionId(ids, 11, 'ArrowUp'), 33)
  assert.equal(moveRovingOptionId(ids, 22, 'Home'), 11)
  assert.equal(moveRovingOptionId(ids, 22, 'End'), 33)
  assert.equal(moveRovingOptionId(ids, null, 'ArrowDown'), 11)
  assert.equal(moveRovingOptionId(ids, null, 'ArrowUp'), 33)
  assert.equal(moveRovingOptionId([], 11, 'ArrowDown'), null)

  assert.equal(isRovingListboxSelectionKey('Enter'), true)
  assert.equal(isRovingListboxSelectionKey(' '), true)
  assert.equal(isRovingListboxSelectionKey('Spacebar'), true)
  assert.equal(isRovingListboxSelectionKey('Unidentified', 'Space'), true)
  assert.equal(isRovingListboxSelectionKey('Escape'), false)
  assert.equal(isRovingListboxSelectionKey('Tab'), false)
})

test('Seconds pair listbox keeps the active identity stable across filtering', () => {
  assert.equal(stableRovingOptionId([11, 22, 33], 22, 11), 22)
  assert.equal(stableRovingOptionId([22, 33], 22, 11), 22)
  assert.equal(stableRovingOptionId([11, 33], 22, 11), 11)
  assert.equal(stableRovingOptionId([33], 22, 11), 33)
  assert.equal(stableRovingOptionId([], 22, 11), null)
})

test('Seconds pair dialog wires roving focus while preserving shared Tab and Escape handling', () => {
  const keyHandler = sourceSlice(
    secondsSource,
    'function handlePairPickerKeydown',
    '/** Clears order baselines',
  )
  const optionTemplate = sourceSlice(
    secondsSource,
    '<div\n                v-for="product in filteredPairProducts"',
    '<p v-if="loading"',
  )

  assert.match(keyHandler, /\['ArrowDown', 'ArrowUp', 'Home', 'End'\]/)
  assert.match(keyHandler, /event\.preventDefault\(\)[\s\S]*?moveRovingOptionId/)
  assert.match(keyHandler, /activePairProductId\.value = nextId/)
  assert.match(keyHandler, /nextTick\(\(\) => \{[\s\S]*?data-seconds-pair-option-id="\$\{nextId\}"[\s\S]*?\.focus\(\)/)
  assert.match(keyHandler, /isRovingListboxSelectionKey\(event\.key, event\.code\)[\s\S]*?choosePairProduct\(product\)/)
  assert.match(keyHandler, /trapPairPickerFocus\(event, closePairPicker\)\s*\}/)
  assert.doesNotMatch(keyHandler, /event\.key === 'Escape'|event\.key === 'Tab'/)

  assert.match(optionTemplate, /role="option"/)
  assert.match(optionTemplate, /:tabindex="activePairProductId === product\.id \? 0 : -1"/)
  assert.match(optionTemplate, /@focus="activePairProductId = product\.id"/)
  assert.doesNotMatch(optionTemplate, /<button/)
  assert.match(secondsSource, /useModalDialog\([\s\S]*?pairPickerOpen,[\s\S]*?pairPickerDialog/)
})

function sourceSlice(source: string, start: string, end: string): string {
  const startIndex = source.indexOf(start)
  const endIndex = source.indexOf(end, startIndex)
  assert.ok(startIndex >= 0 && endIndex > startIndex, `missing source slice ${start} -> ${end}`)
  return source.slice(startIndex, endIndex)
}
