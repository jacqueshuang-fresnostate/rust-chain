import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

/** @typedef {{ path: string, maxLines: number, maxBytes: number, group?: string }} SourceSizeBudget */
/** @typedef {Record<string, { maxLines: number, maxBytes: number }>} SourceGroupBudgets */

export const SOURCE_SIZE_BUDGETS = Object.freeze([
  Object.freeze({ path: 'src/views/TradeView.vue', maxLines: 6_131, maxBytes: 179_654 }),
  Object.freeze({ path: 'src/views/SecondsView.vue', maxLines: 3_464, maxBytes: 100_942 }),
  Object.freeze({ path: 'src/views/AssetsView.vue', maxLines: 2_094, maxBytes: 63_867 }),
  Object.freeze({ path: 'src/styles/base.css', maxLines: 601, maxBytes: 13_522, group: 'shared-css' }),
  Object.freeze({ path: 'src/styles/prototype-base.css', maxLines: 8_032, maxBytes: 141_686, group: 'shared-css' }),
  Object.freeze({ path: 'src/styles/prototype-parity.css', maxLines: 3_686, maxBytes: 91_152, group: 'shared-css' }),
  Object.freeze({ path: 'src/styles/pencil-selected-pages.css', maxLines: 889, maxBytes: 21_540, group: 'shared-css' }),
])

export const SOURCE_GROUP_BUDGETS = Object.freeze({
  'shared-css': Object.freeze({ maxLines: 13_208, maxBytes: 267_900 }),
})

function lineCount(text) {
  if (!text.length) return 0
  const newlines = text.match(/\n/g)?.length || 0
  return newlines + (text.endsWith('\n') ? 0 : 1)
}

/**
 * @param {string} rootDirectory
 * @param {readonly SourceSizeBudget[]} budgets
 */
export async function measureSourceSizes(rootDirectory, budgets = SOURCE_SIZE_BUDGETS) {
  return Promise.all(budgets.map(async (budget) => {
    const absolute = resolve(rootDirectory, budget.path)
    try {
      const content = await readFile(absolute)
      return {
        ...budget,
        bytes: content.byteLength,
        lines: lineCount(content.toString('utf8')),
        missing: false,
      }
    } catch (error) {
      return {
        ...budget,
        bytes: 0,
        lines: 0,
        missing: true,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }))
}

/**
 * @param {Array<SourceSizeBudget & { bytes: number, lines: number, missing: boolean, error?: string }>} measurements
 * @param {SourceGroupBudgets} groupBudgets
 */
export function evaluateSourceSizes(
  measurements,
  groupBudgets = SOURCE_GROUP_BUDGETS,
) {
  const failures = []
  for (const file of measurements) {
    if (file.missing) {
      failures.push(`${file.path} is missing (${file.error})`)
      continue
    }
    if (file.lines > file.maxLines) {
      failures.push(`${file.path} has ${file.lines} lines; budget is ${file.maxLines}`)
    }
    if (file.bytes > file.maxBytes) {
      failures.push(`${file.path} has ${file.bytes} bytes; budget is ${file.maxBytes}`)
    }
  }

  for (const [group, budget] of Object.entries(groupBudgets)) {
    const files = measurements.filter((file) => file.group === group && !file.missing)
    const actual = files.reduce((total, file) => ({
      bytes: total.bytes + file.bytes,
      lines: total.lines + file.lines,
    }), { bytes: 0, lines: 0 })
    if (actual.lines > budget.maxLines) {
      failures.push(`${group} has ${actual.lines} lines; group budget is ${budget.maxLines}`)
    }
    if (actual.bytes > budget.maxBytes) {
      failures.push(`${group} has ${actual.bytes} bytes; group budget is ${budget.maxBytes}`)
    }
  }
  return failures
}

export function formatSourceSizeDiagnostics(measurements) {
  const lines = ['Source size diagnostics:']
  for (const file of measurements) {
    lines.push(file.missing
      ? `  ${file.path}: missing`
      : `  ${file.path}: ${file.lines}/${file.maxLines} lines, ${file.bytes}/${file.maxBytes} bytes`)
  }
  return lines.join('\n')
}

export async function inspectSourceSizeBudgets(rootDirectory = process.cwd()) {
  const measurements = await measureSourceSizes(rootDirectory)
  return {
    failures: evaluateSourceSizes(measurements),
    measurements,
  }
}

async function main() {
  const result = await inspectSourceSizeBudgets(process.cwd())
  console.log(formatSourceSizeDiagnostics(result.measurements))
  if (result.failures.length) {
    console.error('\nSource size budget failures:')
    for (const failure of result.failures) console.error(`  - ${failure}`)
    process.exitCode = 1
  } else {
    console.log('\nSource size budgets passed.')
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`Source size check failed: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
