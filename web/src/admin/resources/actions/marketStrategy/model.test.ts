import { describe, expect, it } from 'vitest';

import {
  isMarketStrategySubmittable,
  marketStrategyBasePayload,
  marketStrategyFromRecord
} from './model';

describe('market strategy model', () => {
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
