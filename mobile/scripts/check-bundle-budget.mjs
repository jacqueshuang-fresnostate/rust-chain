import { gzipSync } from 'node:zlib'
import { readdir, readFile } from 'node:fs/promises'
import { basename, relative, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const KIB = 1024

export const DEFAULT_BUNDLE_BUDGET = Object.freeze({
  entry: {
    js: { raw: 480 * KIB, gzip: 165 * KIB },
    css: { raw: 250 * KIB, gzip: 46 * KIB },
  },
  largest: {
    js: { raw: 480 * KIB, gzip: 165 * KIB },
    css: { raw: 250 * KIB, gzip: 46 * KIB },
  },
  totals: {
    js: { raw: 1_500 * KIB, gzip: 510 * KIB, count: 105 },
    css: { raw: 640 * KIB, gzip: 128 * KIB, count: 50 },
  },
})

async function collectFiles(directory, root = directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(entries.map(async (entry) => {
    const absolute = resolve(directory, entry.name)
    if (entry.isDirectory()) return collectFiles(absolute, root)
    return [{ absolute, relative: relative(root, absolute).replaceAll('\\', '/') }]
  }))
  return nested.flat()
}

function attribute(tag, name) {
  return tag.match(new RegExp(`\\b${name}=["']([^"']+)["']`, 'i'))?.[1]
}

function entryReferences(indexHtml) {
  const scripts = [...indexHtml.matchAll(/<script\b[^>]*>/gi)]
    .map(([tag]) => attribute(tag, 'src'))
    .filter(Boolean)
  const styles = [...indexHtml.matchAll(/<link\b[^>]*>/gi)]
    .filter(([tag]) => String(attribute(tag, 'rel') || '').toLowerCase() === 'stylesheet')
    .map(([tag]) => attribute(tag, 'href'))
    .filter(Boolean)
  return { scripts, styles }
}

function findReferencedFile(files, reference) {
  const clean = decodeURIComponent(String(reference).split(/[?#]/, 1)[0]).replace(/^\/+/, '')
  return files.find((file) => file.relative === clean || clean.endsWith(`/${file.relative}`))
    || files.find((file) => basename(file.relative) === basename(clean))
}

function summarize(rows) {
  return rows.reduce((summary, row) => ({
    raw: summary.raw + row.raw,
    gzip: summary.gzip + row.gzip,
    count: summary.count + 1,
  }), { raw: 0, gzip: 0, count: 0 })
}

export async function measureBundle(distDirectory) {
  const dist = resolve(distDirectory)
  const assetsDirectory = resolve(dist, 'assets')
  const indexHtml = await readFile(resolve(dist, 'index.html'), 'utf8')
  const sourceFiles = (await collectFiles(assetsDirectory))
    .filter((file) => /\.(?:css|js)$/.test(file.relative))
  const rows = await Promise.all(sourceFiles.map(async (file) => {
    const content = await readFile(file.absolute)
    return {
      ...file,
      type: file.relative.endsWith('.css') ? 'css' : 'js',
      raw: content.byteLength,
      gzip: gzipSync(content, { level: 9 }).byteLength,
    }
  }))
  rows.sort((left, right) => right.raw - left.raw || left.relative.localeCompare(right.relative))

  const references = entryReferences(indexHtml)
  const entryJs = references.scripts.map((item) => findReferencedFile(rows, item)).filter(Boolean)
  const entryCss = references.styles.map((item) => findReferencedFile(rows, item)).filter(Boolean)
  const js = rows.filter((row) => row.type === 'js')
  const css = rows.filter((row) => row.type === 'css')

  return {
    dist,
    files: rows,
    entries: { js: entryJs, css: entryCss },
    totals: { js: summarize(js), css: summarize(css) },
  }
}

function sizeFailures(label, actual, limit) {
  const failures = []
  for (const encoding of ['raw', 'gzip']) {
    if (actual[encoding] > limit[encoding]) {
      failures.push(`${label} ${encoding} ${formatBytes(actual[encoding])} exceeds ${formatBytes(limit[encoding])}`)
    }
  }
  return failures
}

export function evaluateBundleBudget(report, budget = DEFAULT_BUNDLE_BUDGET) {
  const failures = []
  for (const type of ['js', 'css']) {
    const total = report.totals[type]
    failures.push(...sizeFailures(`total ${type.toUpperCase()}`, total, budget.totals[type]))
    if (total.count > budget.totals[type].count) {
      failures.push(`total ${type.toUpperCase()} chunk count ${total.count} exceeds ${budget.totals[type].count}`)
    }

    const entry = summarize(report.entries[type])
    if (entry.count === 0) {
      failures.push(`entry ${type.toUpperCase()} was not resolved from index.html`)
    } else {
      failures.push(...sizeFailures(`entry ${type.toUpperCase()}`, entry, budget.entry[type]))
    }

    for (const file of report.files.filter((row) => row.type === type)) {
      failures.push(...sizeFailures(`${type.toUpperCase()} chunk ${file.relative}`, file, budget.largest[type]))
    }
  }
  return failures
}

export function formatBytes(value) {
  return `${(value / KIB).toFixed(1)} KiB`
}

export function formatBundleReport(report, budget = DEFAULT_BUNDLE_BUDGET) {
  const lines = ['Bundle budget diagnostics:']
  for (const type of ['js', 'css']) {
    const total = report.totals[type]
    const entry = summarize(report.entries[type])
    lines.push(
      `  ${type.toUpperCase()} total: ${formatBytes(total.raw)} raw / ${formatBytes(total.gzip)} gzip / ${total.count} chunks`,
      `  ${type.toUpperCase()} entry: ${formatBytes(entry.raw)} raw / ${formatBytes(entry.gzip)} gzip / ${entry.count} files`,
      `  ${type.toUpperCase()} limits: total ${formatBytes(budget.totals[type].raw)} raw / ${formatBytes(budget.totals[type].gzip)} gzip; entry ${formatBytes(budget.entry[type].raw)} raw / ${formatBytes(budget.entry[type].gzip)} gzip`,
    )
    for (const file of report.files.filter((row) => row.type === type).slice(0, 8)) {
      lines.push(`    ${file.relative}: ${formatBytes(file.raw)} raw / ${formatBytes(file.gzip)} gzip`)
    }
  }
  return lines.join('\n')
}

async function main() {
  const dist = resolve(process.cwd(), process.argv[2] || 'dist')
  const report = await measureBundle(dist)
  const failures = evaluateBundleBudget(report)
  console.log(formatBundleReport(report))
  if (failures.length > 0) {
    console.error('\nBundle budget failures:')
    for (const failure of failures) console.error(`  - ${failure}`)
    process.exitCode = 1
  } else {
    console.log('\nBundle budgets passed.')
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`Bundle budget check failed: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
