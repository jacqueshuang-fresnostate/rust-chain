import type { DefaultMarketResponse } from './types';
export function responseFixture(patch: Partial<DefaultMarketResponse> = {}): DefaultMarketResponse {
  return {
    pair_id: 3, symbol: 'TESTUSDT', market_type: 'strategy', configured: false, version: 0,
    enabled: false, all_market_paused: false, initial_price: null, seed: 'default-market:3', updated_at: null,
    config: { volatility: '0.0015', mean_reversion: '0.05', price_min: null, price_max: null,
      volume_min: '10', volume_max: '20', wick_strength: '0.25', depth_levels: 20 },
    runtime: { generation: 0, active_source: 'none', strategy_id: null, strategy_version: null,
      default_version: null, last_price: null, last_tick_at: null, error_message: null }, ...patch
  };
}
