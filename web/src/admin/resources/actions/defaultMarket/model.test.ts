import { expect, it } from 'vitest';
import { defaultMarketPayload, defaultMarketDraft, validateDefaultMarketDraft } from './model';
import { responseFixture } from './testFixtures';

it('preserves exact decimal strings, explicit enable and captured version without a pause switch', () => {
  const draft = defaultMarketDraft(responseFixture());
  draft.initial_price = '0.000000000000000017';
  draft.volume_min = '10.000000000000000017';
  const payload = defaultMarketPayload(draft, 7);
  expect(payload).toMatchObject({ expected_version: 7, enabled: false, initial_price: draft.initial_price, config: { volume_min: draft.volume_min } });
  expect(payload).not.toHaveProperty('all_market_paused');
  expect(validateDefaultMarketDraft(draft, 18, 18)).toBe('');
});

it('rejects lossy precision, nonfinite values, storage overflow and inverted bounds without rounding', () => {
  const draft = defaultMarketDraft(responseFixture());
  for (const patch of [
    { volatility: 'NaN' }, { mean_reversion: '1.01' }, { wick_strength: '-0.1' },
    { price_min: '0' }, { price_max: '1e21' }, { initial_price: '0.0001' },
    { volume_min: '1.001' }, { volume_max: '-1' }, { depth_levels: '21' },
    { depth_levels: '1.5' }, { price_min: '3', price_max: '2' },
    { volume_min: '21', volume_max: '20' }
  ]) expect(validateDefaultMarketDraft({ ...draft, ...patch }, 2, 2), JSON.stringify(patch)).not.toBe('');
  expect(validateDefaultMarketDraft(draft, NaN, 2)).toContain('精度');
});

it('allows a missing initial price only as null for authoritative server price selection', () => {
  const draft = defaultMarketDraft(responseFixture());
  expect(defaultMarketPayload(draft, 0).initial_price).toBeNull();
  expect(validateDefaultMarketDraft({ ...draft, volume_min: '0', volume_max: '0', volatility: '0' }, 0, 0)).toBe('');
});

it('normalizes legacy independent config and explicitly removes follow settings from independent saves', () => {
  const draft = defaultMarketDraft(responseFixture());
  expect(draft.mode).toBe('independent');
  expect(draft.reference_pair_id).toBe('');
  draft.reference_pair_id = '8';
  expect(defaultMarketPayload(draft, 2).config).toMatchObject({ mode: 'independent', follow: null });
});

it('validates exact following ratios and explicit reference identity without guessing BTC', () => {
  const draft = { ...defaultMarketDraft(responseFixture()), mode: 'follow' as const };
  expect(validateDefaultMarketDraft(draft, 18, 18)).toContain('参考交易对');
  const valid = { ...draft, reference_pair_id: '8', follow_multiplier: '1.000000000000000017', follow_max_move_ratio: '0.05', follow_stale_after_seconds: '60' };
  expect(validateDefaultMarketDraft(valid, 18, 18)).toBe('');
  expect(defaultMarketPayload(valid, 2).config.follow).toEqual({ reference_pair_id: 8, multiplier: '1.000000000000000017', max_move_ratio: '0.05', stale_after_seconds: 60 });
  for (const patch of [{ reference_pair_id: '0' }, { reference_pair_id: '9007199254740993' }, { follow_multiplier: '3.000000000000000001' }, { follow_multiplier: '0' }, { follow_max_move_ratio: '1.01' }, { follow_max_move_ratio: '0' }, { follow_stale_after_seconds: '14' }, { follow_stale_after_seconds: '301' }, { follow_stale_after_seconds: '15.5' }]) {
    expect(validateDefaultMarketDraft({ ...valid, ...patch }, 18, 18), JSON.stringify(patch)).not.toBe('');
  }
});
