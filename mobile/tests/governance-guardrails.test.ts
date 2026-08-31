import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import {
  evaluateSourceSizes,
  inspectSourceSizeBudgets,
  measureSourceSizes,
} from '../scripts/check-source-size.mjs'
import {
  evaluateCriticalBehaviorTests,
} from '../scripts/check-test-quality.mjs'

test('source budgets reject line, byte, group growth, and missing governed files', async () => {
  const root = await mkdtemp(join(tmpdir(), 'hippo-source-budget-'))
  try {
    await writeFile(join(root, 'small.ts'), 'a\nb\n')
    const budgets = [
      { path: 'small.ts', maxLines: 1, maxBytes: 3, group: 'shared' },
      { path: 'missing.ts', maxLines: 1, maxBytes: 1 },
    ]
    const measurements = await measureSourceSizes(root, budgets)
    const failures = evaluateSourceSizes(measurements, {
      shared: { maxLines: 1, maxBytes: 3 },
    })
    assert.ok(failures.some((failure) => failure.includes('small.ts has 2 lines')))
    assert.ok(failures.some((failure) => failure.includes('small.ts has 4 bytes')))
    assert.ok(failures.some((failure) => failure.includes('shared has 2 lines')))
    assert.ok(failures.some((failure) => failure.includes('missing.ts is missing')))
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})

test('critical-module quality gate counts direct executable behavior, not source-string assertions alone', async () => {
  const root = await mkdtemp(join(tmpdir(), 'hippo-test-quality-'))
  try {
    await mkdir(join(root, 'src/core'), { recursive: true })
    await mkdir(join(root, 'tests'), { recursive: true })
    await writeFile(join(root, 'src/core/race.ts'), 'export const next = (value: number) => value + 1\n')
    const contracts = [{
      category: 'race',
      source: 'src/core/race.ts',
      tests: ['tests/race.test.ts'],
    }]

    await writeFile(join(root, 'tests/race.test.ts'), [
      "import assert from 'node:assert/strict'",
      "import test from 'node:test'",
      "import { next } from '../src/core/race.ts'",
      "test('increments', () => assert.equal(next(1), 2))",
    ].join('\n'))
    assert.deepEqual((await evaluateCriticalBehaviorTests(root, contracts)).failures, [])

    await writeFile(join(root, 'tests/race.test.ts'), [
      "import assert from 'node:assert/strict'",
      "import { readFileSync } from 'node:fs'",
      "import test from 'node:test'",
      "const source = readFileSync(new URL('../src/core/race.ts', import.meta.url), 'utf8')",
      "test('contains implementation', () => assert.match(source, /next/))",
    ].join('\n'))
    const sourceOnly = await evaluateCriticalBehaviorTests(root, contracts)
    assert.ok(sourceOnly.failures.some((failure) => failure.includes('no mapped behavior test')))
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})

test('current giant-source and critical-behavior budgets are executable against the repository', async () => {
  const sourceBudget = await inspectSourceSizeBudgets(process.cwd())
  assert.deepEqual(sourceBudget.failures, [])

  const testQuality = await evaluateCriticalBehaviorTests(process.cwd())
  assert.deepEqual(testQuality.failures, [])
})
