import assert from 'node:assert/strict'
import test from 'node:test'
import {
  decimalDivide,
  decimalMultiply,
  normalizeDecimalText,
  positiveDecimalInput,
} from '../src/core/decimal.ts'
import {
  baseQuantityFromQuoteAmount,
  createSpotOrderReviewSnapshot,
  createTradeFinancialPresentation,
  quoteAmountFromBaseQuantity,
  resolveTradeEffectivePrice,
  resolveTradeLimitPriceFromBook,
} from '../src/core/tradeFinancial.ts'
import { quantityForBalancePercentagePoints } from '../src/core/tradeForm.ts'
import {
  createSecondsFinancialPresentation,
  createSecondsOrderReviewSnapshot,
  deriveSecondsEstimatedProfit,
  deriveSecondsProfitLoss,
  deriveSecondsReturnRatePercent,
  secondsFinancialOrderValues,
  validateSecondsStake,
} from '../src/core/secondsFinancial.ts'

test('DecimalText rejects incomplete trailing-decimal drafts at every financial boundary', () => {
  assert.throws(() => normalizeDecimalText('1.'), /invalid decimal text/)
  assert.equal(positiveDecimalInput('1.'), null)
  assert.equal(resolveTradeEffectivePrice({
    orderType: 'limit',
    limitPrice: '12.',
    marketPrice: '13',
  }), null)
  assert.equal(validateSecondsStake('1.', {
    minimum: '0.000000000000000001',
    maximum: '10',
    available: '10',
  }).isValid, false)
})

test('DecimalText precision limits ignore insignificant trailing zeros', () => {
  assert.equal(
    positiveDecimalInput('1.0000000000000000000'),
    '1',
  )
  assert.equal(
    positiveDecimalInput('1.0000000000000000001'),
    null,
  )
  assert.equal(validateSecondsStake('1.0000000000000000000', {
    minimum: '1.0000000000000000000',
    maximum: '1.0000000000000000000',
    available: '1.0000000000000000000',
  }).isValid, true)
})

test('Trade derivations preserve 1e-18 and values beyond Number safe range', () => {
  const atom = normalizeDecimalText('0.000000000000000001')
  const large = normalizeDecimalText('9007199254740993.000000000000000001')
  const price = normalizeDecimalText('2')

  assert.equal(quoteAmountFromBaseQuantity(atom, price), '0.000000000000000002')
  assert.equal(quoteAmountFromBaseQuantity(large, price), '18014398509481986.000000000000000002')
  assert.equal(baseQuantityFromQuoteAmount('18014398509481986.000000000000000002', price), large)
})

test('Trade multiply/divide and spot review payloads never round through IEEE-754', () => {
  assert.equal(
    decimalMultiply(normalizeDecimalText('0.1'), normalizeDecimalText('0.2')),
    '0.02',
  )
  assert.equal(
    decimalDivide(normalizeDecimalText('1'), normalizeDecimalText('8'), 18),
    '0.125',
  )
  assert.equal(quantityForBalancePercentagePoints({
    available: '9007199254740993.000000000000000001',
    mode: 'contract',
    percentagePoints: 37,
    price: '1',
    side: 'buy',
  }), '3332663724254167.41')
  assert.equal(quantityForBalancePercentagePoints({
    available: '0.000000000000000004',
    mode: 'contract',
    percentagePoints: 25,
    price: '1',
    side: 'buy',
  }), '0.000000000000000001')

  const draft = {
    symbol: 'BTC/USDT',
    side: 'buy' as const,
    orderType: 'limit' as const,
    quantity: '9007199254740993.000000000000000001',
    limitPrice: '0.000000000000000001',
    marketPrice: '123.45',
  }
  const review = createSpotOrderReviewSnapshot(draft)
  assert.ok(review)

  draft.quantity = '1'
  draft.limitPrice = '2'
  assert.equal(review.quantity, '9007199254740993.000000000000000001')
  assert.equal(review.price, '0.000000000000000001')
  assert.equal(review.quoteAmount, '0.009007199254740993000000000000000001')
  assert.equal(Object.isFrozen(review), true)
})

test('Trade execution helpers reject numeric fallbacks and select only exact book text', () => {
  assert.equal(resolveTradeEffectivePrice({
    orderType: 'market',
    limitPrice: '',
    marketPrice: 13 as unknown as string,
  }), null)
  assert.equal(
    quoteAmountFromBaseQuantity(0.1 as unknown as string, '2'),
    null,
  )

  const asks = [
    { price: 100.1, priceText: '100.100000000000000001' },
    { price: 100.09, priceText: '100.099999999999999999' },
  ]
  const bids = [
    { price: 99.9, priceText: '99.900000000000000001' },
    { price: 99.91, priceText: '99.909999999999999999' },
  ]
  assert.equal(resolveTradeLimitPriceFromBook({
    side: 'buy',
    bids,
    asks,
    latestPrice: '101.000000000000000001',
  }), '100.099999999999999999')
  assert.equal(resolveTradeLimitPriceFromBook({
    side: 'sell',
    bids,
    asks,
    latestPrice: '98.000000000000000001',
  }), '99.909999999999999999')
  assert.equal(resolveTradeLimitPriceFromBook({
    side: 'buy',
    bids: [{ price: 99.12345678901235 }],
    asks: [{ price: 100.12345678901235 }],
    latestPrice: '101.000000000000000001',
  }), '101.000000000000000001')
  assert.equal(resolveTradeLimitPriceFromBook({
    side: 'buy',
    bids: [],
    asks: [{ price: 100.12345678901235 }],
    latestPrice: null,
  }), null)
})

test('Seconds stake, payout, profit/loss and return ratio remain exact DecimalText', () => {
  const stake = normalizeDecimalText('9007199254740993.000000000000000001')
  const payoutRate = normalizeDecimalText('0.8')
  const profit = deriveSecondsEstimatedProfit(stake, payoutRate)

  assert.equal(profit, '7205759403792794.4000000000000000008')
  assert.deepEqual(deriveSecondsProfitLoss({ result: 'win', stake, payoutRate }), {
    kind: 'profit',
    amount: profit,
  })
  assert.deepEqual(deriveSecondsProfitLoss({ result: 'loss', stake, payoutRate }), {
    kind: 'loss',
    amount: '-9007199254740993.000000000000000001',
  })
  assert.equal(deriveSecondsReturnRatePercent(profit, stake), '80')
  assert.equal(
    deriveSecondsEstimatedProfit('0.000000000000000001', '0.5'),
    '0.0000000000000000005',
  )
})

test('Seconds review freezes exact financial text and reuses it as the request payload', () => {
  const source = {
    productId: 7,
    cycleId: 30,
    symbol: 'BTCUSDT',
    stakeAssetId: 1,
    stakeAssetSymbol: 'USDT',
    durationSeconds: 30,
    direction: 'up' as const,
    stakeAmount: '9007199254740993.000000000000000001',
    minimumStake: '0.000000000000000001',
    maximumStake: '9007199254740993.000000000000000001',
    available: '9007199254740993.000000000000000001',
    payoutRate: '0.875',
    referencePrice: '60000.000000000000000001',
    idempotencyKey: 'seconds-decimal-review',
  }
  const review = createSecondsOrderReviewSnapshot(source)
  assert.ok(review)

  source.stakeAmount = '1'
  source.payoutRate = '0.1'
  source.referencePrice = '2'
  assert.equal(review.stakeAmount, '9007199254740993.000000000000000001')
  assert.equal(review.payoutRate, '0.875')
  assert.equal(review.referencePrice, '60000.000000000000000001')
  assert.equal(review.estimatedProfit, '7881299347898368.875000000000000000875')
  assert.deepEqual(review.request, {
    productId: 7,
    durationSeconds: 30,
    direction: 'up',
    stakeAmount: '9007199254740993.000000000000000001',
    idempotencyKey: 'seconds-decimal-review',
  })
  assert.equal(Object.isFrozen(review), true)
  assert.equal(Object.isFrozen(review.request), true)
})

test('Seconds execution and PnL derivations fail closed on legacy numeric financial fields', () => {
  assert.equal(createSecondsOrderReviewSnapshot({
    productId: 7,
    cycleId: 30,
    symbol: 'BTCUSDT',
    stakeAssetId: 1,
    stakeAssetSymbol: 'USDT',
    durationSeconds: 30,
    direction: 'up',
    stakeAmount: '1',
    minimumStake: '1',
    maximumStake: '10',
    available: '10',
    payoutRate: 0.875 as unknown as string,
    referencePrice: '60000',
    idempotencyKey: 'seconds-numeric-rate',
  }), null)
  assert.deepEqual(secondsFinancialOrderValues({
    stakeAmount: 9007199254740994,
    payoutRate: 0.875,
    entryPrice: 60000,
    settlementPrice: 61000,
  }), {
    stakeAmount: null,
    payoutRate: null,
    entryPrice: null,
    settlementPrice: null,
  })
})

test('extracted Trade and Seconds presentation adapters preserve exact execution text', () => {
  const tradePresentation = createTradeFinancialPresentation({
    locale: () => 'en-US',
    translate: (key, params) => [key, params.minimum, params.maximum, params.asset]
      .filter(Boolean)
      .join('|'),
  })
  assert.equal(tradePresentation.formatHourlyInterest({
    hourlyInterestRate: 0.5,
    hourlyInterestRateText: '0.00125',
  }), '0.1250% / 1h')
  assert.equal(tradePresentation.formatMarginRange({
    minMarginText: '0.000000000000000001',
    maxMarginText: '9007199254740993.000000000000000001',
  }, 'USDT'), 'trade.marginRangeWithMaximum|0.000000000000000001|9,007,199,254,740,993|USDT')

  const liveTickers = new Map<string, {
    lastPrice?: number
    lastPriceText?: string
    changePercent?: number
  }>([['BTCUSDT', {
    lastPrice: 9007199254740994,
    lastPriceText: '9007199254740993.000000000000000001',
    changePercent: 1.25,
  }]])
  const secondsPresentation = createSecondsFinancialPresentation({
    locale: () => 'en-US',
    exactByOrderId: new Map(),
    normalizeSymbol: (symbol) => symbol.replace(/[-_/\s]/g, '').toUpperCase(),
    liveTickerFor: (symbol) => liveTickers.get(symbol.replace(/[-_/\s]/g, '').toUpperCase()),
    marketTickerFor: () => undefined,
    selectedSymbol: () => 'BTCUSDT',
    selectedCandleClose: () => 60000.25,
    translate: (key) => key,
  })
  assert.equal(secondsPresentation.exactPriceForSymbol('BTC/USDT'), '9007199254740993.000000000000000001')
  assert.equal(secondsPresentation.priceFor('BTC/USDT'), '9007199254740993.000000000000000001')
  assert.equal(secondsPresentation.exactCyclePayoutRate({
    minStake: 1,
    payoutRate: 0.875,
    payoutRateText: '0.875000000000000001',
  }), '0.875000000000000001')
  assert.equal(secondsPresentation.exactCyclePayoutRate({
    minStake: 1,
    payoutRate: 0.875,
  }), null)

  liveTickers.set('BTCUSDT', { lastPrice: 9007199254740994 })
  assert.equal(secondsPresentation.exactPriceForSymbol('BTC/USDT'), null)
  assert.equal(secondsPresentation.priceFor('BTC/USDT'), 9007199254740994)
})
