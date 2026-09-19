// Run from repository root. Only in-memory HTTP mocks are used.
import assert from 'node:assert/strict'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const root = process.cwd()
const { createServer } = await import(pathToFileURL(resolve(root, 'mobile/node_modules/vite/dist/node/index.js')))
const server = await createServer({
  root: resolve(root, 'mobile'),
  configFile: resolve(root, 'mobile/vite.config.ts'),
  appType: 'custom',
  logLevel: 'silent',
  server: { middlewareMode: true },
})
try {
  const { client } = await server.ssrLoadModule('/src/api/client.ts')
  const swap = await server.ssrLoadModule('/src/api/swap.ts')
  const earn = await server.ssrLoadModule('/src/api/earn.ts')
  const loan = await server.ssrLoadModule('/src/api/loan.ts')
  const { formatAmount } = await server.ssrLoadModule('/src/core/format.ts')
  const exact = '9007199254740993.000000000000000001'
  const small = '0.00000001'
  const requests = []
  client.post = async (url, body) => {
    requests.push({ url, body })
    return { data: {
      quote_id: 'precision-review',
      convert_pair_id: 1,
      from_amount: body.from_amount,
      to_amount: exact,
      rate: '1',
      fee_amount: small,
      expires_at: Date.now() + 60_000,
    } }
  }
  const quote = await swap.requestConvertQuote({ fromAssetId: 1, toAssetId: 2 }, exact)
  assert.equal(requests[0].body.from_amount, exact)
  assert.equal(String(quote.fromAmount), '9007199254740994')
  assert.equal(formatAmount(quote.feeAmount), '0')
  await swap.confirmConvertQuote(quote.quoteId)
  assert.deepEqual(requests[1].body, { quote_id: 'precision-review' })
  client.get = async (url) => {
    if (url.endsWith('/earn/subscriptions')) {
      return { data: { subscriptions: [{ id: 1, amount: exact, apr_rate: '0.05' }] } }
    }
    if (url.endsWith('/loan/orders')) {
      return { data: { orders: [{ id: 1, amount: exact, interest_amount: small, repayment_amount: exact }] } }
    }
    throw new Error(`Unexpected mocked URL: ${url}`)
  }
  const [holding] = await earn.fetchEarnSubscriptions()
  const [debt] = await loan.fetchLoanOrders()
  assert.equal(String(holding.amount), '9007199254740994')
  assert.equal(String(debt.repaymentAmount), '9007199254740994')
  console.log(JSON.stringify({
    exactRequest: requests[0].body.from_amount,
    quoteAmount: String(quote.fromAmount),
    quoteFee: quote.feeAmount,
    displayedFee: formatAmount(quote.feeAmount),
    confirmPayload: requests[1].body,
    holdingAmount: String(holding.amount),
    repaymentAmount: String(debt.repaymentAmount),
  }, null, 2))
} finally {
  await server.close()
}
