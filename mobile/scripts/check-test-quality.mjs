import { access, readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import ts from 'typescript'

export const CRITICAL_BEHAVIOR_TEST_CONTRACTS = Object.freeze([
  { category: 'race', source: 'src/core/apiRequest.ts', tests: ['tests/api-request-error.test.ts'] },
  { category: 'race', source: 'src/core/marketLifecycle.ts', tests: ['tests/market-store-lifecycle.test.ts'] },
  { category: 'race', source: 'src/core/ordersRequest.ts', tests: ['tests/finance-decimal-lifecycle.test.ts'] },
  { category: 'race', source: 'src/core/privateUserStreamManager.ts', tests: ['tests/private-user-stream.test.ts'] },
  { category: 'race', source: 'src/core/sessionOwner.ts', tests: ['tests/session-owner.test.ts'] },
  { category: 'race', source: 'src/core/sessionRequest.ts', tests: ['tests/critical-core-behavior.test.ts'] },
  { category: 'race', source: 'src/core/marginAccountReconciliation.ts', tests: ['tests/margin-account-reconciliation.test.ts'] },
  { category: 'race', source: 'src/core/supportChat.ts', tests: ['tests/support-chat.test.ts'] },
  { category: 'financial', source: 'src/core/decimal.ts', tests: ['tests/finance-decimal-lifecycle.test.ts'] },
  { category: 'financial', source: 'src/core/financialEnumPresentation.ts', tests: ['tests/finance-decimal-lifecycle.test.ts'] },
  { category: 'financial', source: 'src/core/marginClose.ts', tests: ['tests/margin-close-sheet.test.ts'] },
  { category: 'financial', source: 'src/core/marginOrder.ts', tests: ['tests/margin-order-type-sheet.test.ts'] },
  { category: 'financial', source: 'src/core/marginOrderConfirmation.ts', tests: ['tests/margin-order-confirm-dialog.test.ts'] },
  { category: 'financial', source: 'src/core/marginRiskMetrics.ts', tests: ['tests/margin-risk-metrics.test.ts'] },
  { category: 'financial', source: 'src/core/newCoinPurchase.ts', tests: ['tests/new-coin-purchase.test.ts'] },
  { category: 'financial', source: 'src/core/realizedReturn.ts', tests: ['tests/critical-core-behavior.test.ts'] },
  { category: 'financial', source: 'src/core/returnHistory.ts', tests: ['tests/return-history.test.ts'] },
  { category: 'financial', source: 'src/core/secondsOrder.ts', tests: ['tests/seconds-api-adapter.test.ts'] },
  { category: 'financial', source: 'src/core/todayReturn.ts', tests: ['tests/today-return.test.ts'] },
  { category: 'financial', source: 'src/core/tradeForm.ts', tests: ['tests/margin-product-boundaries.test.ts'] },
  { category: 'financial', source: 'src/core/walletLedger.ts', tests: ['tests/wallet-ledger-classification.test.ts'] },
  { category: 'financial', source: 'src/core/withdrawalQuote.ts', tests: ['tests/withdrawal-quote-contract.test.ts'] },
])

const SOURCE_ONLY_ASSERTIONS = new Set(['match', 'doesNotMatch'])

async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

function importedBindingNames(declaration) {
  const clause = declaration.importClause
  if (!clause) return []
  const names = []
  if (clause.name) names.push(clause.name.text)
  if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) {
    names.push(clause.namedBindings.name.text)
  }
  if (clause.namedBindings && ts.isNamedImports(clause.namedBindings)) {
    names.push(...clause.namedBindings.elements.map((element) => element.name.text))
  }
  return names
}

function inspectSyntax(sourceText, testAbsolute, sourceAbsolute) {
  const syntax = ts.createSourceFile(testAbsolute, sourceText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS)
  const importedNames = new Set()
  let behaviorTestCases = 0
  let testCases = 0
  let behaviorAssertions = 0
  let importedBindingReferences = 0

  for (const statement of syntax.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) continue
    const importAbsolute = resolve(dirname(testAbsolute), statement.moduleSpecifier.text)
    if (importAbsolute !== sourceAbsolute) continue
    for (const name of importedBindingNames(statement)) importedNames.add(name)
  }

  function testBodyEvidence(node) {
    let assertions = 0
    let references = 0
    function inspect(current) {
      if (
        ts.isCallExpression(current)
        && ts.isPropertyAccessExpression(current.expression)
        && ts.isIdentifier(current.expression.expression)
        && current.expression.expression.text === 'assert'
        && !SOURCE_ONLY_ASSERTIONS.has(current.expression.name.text)
      ) {
        assertions += 1
      }
      if (ts.isIdentifier(current) && importedNames.has(current.text)) references += 1
      ts.forEachChild(current, inspect)
    }
    inspect(node)
    return { assertions, references }
  }

  function visit(node) {
    if (ts.isCallExpression(node)) {
      if (ts.isIdentifier(node.expression) && ['test', 'it'].includes(node.expression.text)) {
        testCases += 1
        const callback = node.arguments.find((argument) => (
          ts.isArrowFunction(argument) || ts.isFunctionExpression(argument)
        ))
        if (callback) {
          const evidence = testBodyEvidence(callback.body)
          if (evidence.assertions > 0 && evidence.references > 0) behaviorTestCases += 1
        }
      }
      if (
        ts.isPropertyAccessExpression(node.expression)
        && ts.isIdentifier(node.expression.expression)
        && node.expression.expression.text === 'assert'
        && !SOURCE_ONLY_ASSERTIONS.has(node.expression.name.text)
      ) {
        behaviorAssertions += 1
      }
    }
    if (
      ts.isIdentifier(node)
      && importedNames.has(node.text)
      && !ts.isImportSpecifier(node.parent)
      && !ts.isImportClause(node.parent)
      && !ts.isNamespaceImport(node.parent)
    ) {
      importedBindingReferences += 1
    }
    ts.forEachChild(node, visit)
  }
  ts.forEachChild(syntax, visit)

  return {
    behaviorTestCases,
    behaviorAssertions,
    directlyImportsSource: importedNames.size > 0,
    importedBindingReferences,
    testCases,
  }
}

export async function inspectBehaviorTestFile(rootDirectory, sourcePath, testPath) {
  const sourceAbsolute = resolve(rootDirectory, sourcePath)
  const testAbsolute = resolve(rootDirectory, testPath)
  if (!await exists(testAbsolute)) {
    return { test: testPath, missing: true }
  }
  const sourceText = await readFile(testAbsolute, 'utf8')
  return {
    test: testPath,
    missing: false,
    ...inspectSyntax(sourceText, testAbsolute, sourceAbsolute),
  }
}

export async function evaluateCriticalBehaviorTests(
  rootDirectory = process.cwd(),
  contracts = CRITICAL_BEHAVIOR_TEST_CONTRACTS,
) {
  const failures = []
  const diagnostics = []

  for (const contract of contracts) {
    const sourceAbsolute = resolve(rootDirectory, contract.source)
    if (!await exists(sourceAbsolute)) {
      failures.push(`${contract.source} is missing`)
      continue
    }
    const evidence = await Promise.all(contract.tests.map((testPath) => (
      inspectBehaviorTestFile(rootDirectory, contract.source, testPath)
    )))
    const behaviorEvidence = evidence.find((item) => (
      !item.missing
      && item.directlyImportsSource
      && item.behaviorTestCases > 0
      && item.importedBindingReferences > 0
      && item.testCases > 0
      && item.behaviorAssertions > 0
    ))
    diagnostics.push({ ...contract, evidence })
    if (!behaviorEvidence) {
      failures.push(`${contract.source} has no mapped behavior test with a direct import, executable test case, and non-source assertion`)
    }
  }
  return { diagnostics, failures }
}

export function formatTestQualityDiagnostics(diagnostics) {
  const lines = ['Critical behavior test diagnostics:']
  for (const contract of diagnostics) {
    const evidence = contract.evidence.map((item) => item.missing
      ? `${item.test} missing`
      : `${item.test} tests=${item.testCases} behavior=${item.behaviorTestCases} assertions=${item.behaviorAssertions} refs=${item.importedBindingReferences}`)
    lines.push(`  [${contract.category}] ${contract.source}: ${evidence.join('; ')}`)
  }
  return lines.join('\n')
}

async function main() {
  const result = await evaluateCriticalBehaviorTests(process.cwd())
  console.log(formatTestQualityDiagnostics(result.diagnostics))
  if (result.failures.length) {
    console.error('\nCritical behavior test failures:')
    for (const failure of result.failures) console.error(`  - ${failure}`)
    process.exitCode = 1
  } else {
    console.log('\nCritical behavior test budgets passed. Source-reading assertions remain supplemental and do not count as the sole evidence.')
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`Test quality check failed: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
