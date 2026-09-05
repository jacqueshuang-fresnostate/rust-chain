import { describe, expect, it } from 'vitest';

import {
  applyPreset,
  inputDateTimeFromUnixMillis,
  isMarketStrategySubmittable,
  marketStrategyBasePayload,
  marketStrategyFromRecord,
  marketStrategyValidationError
} from './model';
import type { MarketStrategyPreset } from './types';

function validStrategy() {
  return marketStrategyFromRecord({
    pair_id: 21, strategy_type: 'price_path', start_price: '1', target_price: '2',
    start_time: new Date('2026-08-12T10:00').getTime(),
    end_time: new Date('2026-08-12T11:00').getTime(),
    volatility: '0.01', volume_min: '0', volume_max: '20', status: 'paused',
    nodes: [{ sequence_no: 0, target_time: new Date('2026-08-12T10:30').getTime(),
      target_type: 'absolute_price', target_value: '1.5', execution_mode: 'hard', tolerance: '0', volatility: '0' }]
  });
}

const twoNodePreset: MarketStrategyPreset = {
  code: 'crash_recovery', name: '急跌修复', description: '', target_price_change_percent: '10',
  generator: { scenario: 'crash_recovery', seed_mode: 'auto', mean_reversion_strength: '0.55', noise_scale: '1', wick_scale: '0.75', volume_shape: 'uniform' },
  nodes: [25, 75].map((progress) => ({ progress_percent: progress, target_type: 'percent_from_previous', target_value: '10', execution_mode: 'hard', tolerance: '0', volatility: '0', volume_min: null, volume_max: null }))
};

describe('market strategy model', () => {
  it.each([
    { targetValue: 'not-a-number' },
    { targetValue: '0' },
    { targetValue: '-1' },
    { targetType: 'percent_from_start', targetValue: '-100' },
    { targetType: 'percent_from_previous', targetValue: '-100.000000000000000001' },
    { targetType: 'unsupported' },
    { executionMode: 'unsupported' }
  ])('rejects invalid node settings before preview or save: %j', (patch) => {
    const values = validStrategy();
    Object.assign(values.nodes[0], patch);
    expect(isMarketStrategySubmittable(values, true)).toBe(false);
    expect(() => marketStrategyBasePayload(values)).toThrow();
  });

  it.each(['percent_from_start', 'percent_from_previous'] as const)('accepts precise negative percentages above -100: %s', (targetType) => {
    const values = validStrategy();
    Object.assign(values.nodes[0], { targetType, targetValue: '-99.999999999999999999' });
    expect(isMarketStrategySubmittable(values, true)).toBe(true);
    expect(marketStrategyBasePayload(values).nodes[0].target_value).toBe('-99.999999999999999999');
  });

  it('rejects an impossible calendar date instead of rolling it forward', () => {
    const values = { ...validStrategy(), nodes: [], startTime: '2026-02-30T10:00', endTime: '2026-03-03T10:00' };
    expect(isMarketStrategySubmittable(values, true)).toBe(false);
    expect(() => marketStrategyBasePayload(values)).toThrow();
  });

  it('keeps invalid historical seconds visible instead of silently rounding the persisted configuration', () => {
    const values = validStrategy();
    values.startTime = inputDateTimeFromUnixMillis(new Date('2026-08-12T10:00:30.123').getTime());
    expect(values.startTime).toBe('2026-08-12T10:00:30.123');
    expect(marketStrategyValidationError(values, true)).toBe('开始和结束时间必须对齐到整分钟');
    expect(() => marketStrategyBasePayload(values)).toThrow('整分钟');
    expect(inputDateTimeFromUnixMillis(Number.MAX_VALUE)).toBe('');
  });

  it('converts offset-bearing detail timestamps into local fields without shifting the instant', () => {
    const values = marketStrategyFromRecord({
      ...marketStrategyBasePayload(validStrategy()), pair_id: 21,
      start_time: '2026-08-12T10:00:00+05:30', end_time: '2026-08-12T11:00:00+05:30', nodes: []
    });
    expect(marketStrategyBasePayload(values).start_time).toBe(Date.UTC(2026, 7, 12, 4, 30));
    expect(marketStrategyBasePayload(values).end_time).toBe(Date.UTC(2026, 7, 12, 5, 30));
  });

  it.each([
    [{ volumeMin: '10', volumeMax: '' }, '最小和最大成交量须同时填写或同时留空'],
    [{ volumeMin: '10', volumeMax: '9.999999999999999999' }, '最大成交量不得小于最小成交量'],
    [{ tolerance: '-0.00000001' }, '容差必须为非负数'],
    [{ volatility: 'NaN' }, '局部波动率必须为非负数']
  ] as const)('reports the exact invalid node field %j', (patch, message) => {
    const values = validStrategy();
    Object.assign(values.nodes[0], patch);
    expect(marketStrategyValidationError(values, true)).toBe(`节点1${message}`);
    expect(isMarketStrategySubmittable(values, true)).toBe(false);
  });

  it('rejects a preset whose nodes would collide rather than silently discarding a target', () => {
    const values = { ...validStrategy(), endTime: '2026-08-12T10:02' };
    const before = structuredClone(values);
    expect(applyPreset(values, twoNodePreset)).toBeNull();
    expect(values).toEqual(before);
  });

  it('applies every relative preset node without changing order or existing input values', () => {
    const values = validStrategy();
    const result = applyPreset(values, twoNodePreset);
    expect(result?.nodes.map(({ targetTime, targetType, targetValue }) => ({ targetTime, targetType, targetValue }))).toEqual([
      { targetTime: '2026-08-12T10:15', targetType: 'percent_from_previous', targetValue: '10' },
      { targetTime: '2026-08-12T10:45', targetType: 'percent_from_previous', targetValue: '10' }
    ]);
    expect(result?.targetPrice).toBe('1.1');
    expect(values.nodes).toHaveLength(1);
  });
  it('hydrates sorted detail nodes and serializes the existing API contract in milliseconds', () => {
    const startTime = new Date('2026-08-12T10:00').getTime();
    const firstNodeTime = new Date('2026-08-12T10:20').getTime();
    const secondNodeTime = new Date('2026-08-12T10:40').getTime();
    const endTime = new Date('2026-08-12T11:00').getTime();
    const values = marketStrategyFromRecord({
      pair_id: 21,
      strategy_type: 'price_path',
      start_price: '1',
      target_price: '2',
      start_time: startTime,
      end_time: endTime,
      volatility: '0.01',
      volume_min: '10',
      volume_max: '20',
      status: 'paused',
      generator: {
        scenario: 'high_volatility',
        seed_mode: 'fixed',
        seed: 'stable-seed',
        mean_reversion_strength: '1.2',
        noise_scale: '2.4',
        wick_scale: '1.8',
        volume_shape: 'bell'
      },
      nodes: [
        {
          sequence_no: 1,
          target_time: secondNodeTime,
          target_type: 'absolute_price',
          target_value: '1.8',
          execution_mode: 'soft',
          tolerance: '0.1',
          volatility: '0.02',
          volume_min: null,
          volume_max: null
        },
        {
          sequence_no: 0,
          target_time: firstNodeTime,
          target_type: 'absolute_price',
          target_value: '1.4',
          execution_mode: 'hard',
          tolerance: '0',
          volatility: '0.01',
          volume_min: '10',
          volume_max: '20'
        }
      ]
    });

    expect(values.nodes.map((node) => node.targetValue)).toEqual(['1.4', '1.8']);
    expect(isMarketStrategySubmittable(values, false)).toBe(true);
    expect(marketStrategyBasePayload(values)).toMatchObject({
      strategy_type: 'price_path',
      start_time: startTime,
      end_time: endTime,
      nodes: [
        { target_time: firstNodeTime, target_value: '1.4', volume_min: '10', volume_max: '20' },
        { target_time: secondNodeTime, target_value: '1.8', volume_min: null, volume_max: null }
      ],
      generator: {
        scenario: 'high_volatility',
        seed_mode: 'fixed',
        seed: 'stable-seed',
        regenerate_seed: false,
        mean_reversion_strength: '1.2',
        noise_scale: '2.4',
        wick_scale: '1.8',
        volume_shape: 'bell'
      }
    });
  });

  it('rejects node times on either strategy boundary', () => {
    const startTime = new Date('2026-08-12T10:00').getTime();
    const endTime = new Date('2026-08-12T11:00').getTime();
    const values = marketStrategyFromRecord({
      pair_id: 21,
      strategy_type: 'price_path',
      start_price: '1',
      target_price: '2',
      start_time: startTime,
      end_time: endTime,
      volatility: '0.01',
      volume_min: '0',
      volume_max: '20',
      status: 'paused',
      nodes: [
        {
          sequence_no: 0,
          target_time: startTime,
          target_type: 'absolute_price',
          target_value: '1.4',
          execution_mode: 'hard',
          tolerance: '0',
          volatility: '0.01'
        }
      ]
    });

    expect(isMarketStrategySubmittable(values, false)).toBe(false);
    expect(
      isMarketStrategySubmittable(
        { ...values, nodes: [{ ...values.nodes[0], targetTime: values.endTime }] },
        false
      )
    ).toBe(false);
  });
});
