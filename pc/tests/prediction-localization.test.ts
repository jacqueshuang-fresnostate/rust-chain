import test from 'node:test'
import assert from 'node:assert/strict'

import { localizePredictionMarketText } from '../src/utils/predictionLocale.ts'

test('prediction market text uses configured locale document before fallback translation', () => {
  const title = localizePredictionMarketText(
    'Will Bitcoin hit $120k by December 31?',
    'zh-CN',
    'title',
    {
      default_locale: 'en',
      items: [
        { locale: 'en', title: 'Will Bitcoin hit $120k by December 31?' },
        { locale: 'zh-CN', title: '比特币会在12月31日前达到 12 万美元吗？' },
      ],
    },
  )

  assert.equal(title, '比特币会在12月31日前达到 12 万美元吗？')
})

test('prediction market text localizes common polymarket english fallback', () => {
  assert.equal(
    localizePredictionMarketText('Will Bitcoin hit $120k by December 31?', 'zh-CN', 'title'),
    '比特币会在12月31日前达到$120k吗？',
  )
  assert.equal(
    localizePredictionMarketText('Will no Fed rate cuts happen in 2026?', 'zh-CN', 'title'),
    '2026年美联储会不降息吗？',
  )
  assert.equal(
    localizePredictionMarketText('Will 12 or more Fed rate cuts happen in 2026?', 'zh-CN', 'title'),
    '2026年美联储会12 次或更多次降息吗？',
  )
  assert.equal(
    localizePredictionMarketText('Opensea FDV above $500M one day after launch?', 'zh-CN', 'title'),
    'Opensea 代币上线后一天 FDV 会高于$500M吗？',
  )
  assert.equal(localizePredictionMarketText('Politics', 'zh-CN', 'category'), '政治')
  assert.equal(localizePredictionMarketText('Yes', 'zh-CN', 'outcome'), '是')
  assert.equal(localizePredictionMarketText('No', 'zh-CN', 'outcome'), '否')
})

test('prediction market text keeps english locale unchanged', () => {
  assert.equal(
    localizePredictionMarketText('Will Ethereum be above $5,000 on June 30?', 'en', 'title'),
    'Will Ethereum be above $5,000 on June 30?',
  )
})
