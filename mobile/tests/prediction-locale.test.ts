import assert from 'node:assert/strict'
import test from 'node:test'
import { localizePredictionMarketText } from '../src/core/predictionLocale.ts'

test('预测市场内容随语言切换并保留无法判断的原始内容', () => {
  assert.equal(localizePredictionMarketText('Crypto', 'zh-CN', 'category'), '加密货币')
  assert.equal(localizePredictionMarketText('Yes', 'zh-CN', 'outcome'), '是')
  assert.equal(
    localizePredictionMarketText('Will Bitcoin hit $100K by December 31, 2026?', 'zh-CN', 'title'),
    '比特币会在2026年12月31日前达到$100K吗？',
  )
  assert.equal(localizePredictionMarketText('Will Bitcoin hit $100K?', 'en', 'title'), 'Will Bitcoin hit $100K?')
})
