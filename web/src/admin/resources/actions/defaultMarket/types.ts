import type { MarketStrategyPreviewSample } from '../marketStrategy/types';

export type DefaultMarketFollow = {
  reference_pair_id: number;
  multiplier: string;
  max_move_ratio: string;
  stale_after_seconds: number;
};
export type DefaultMarketReferencePair = { id: number; symbol: string; status: string; market_type: string };
export type DefaultMarketFollowStatus = {
  mode: 'following' | 'fallback';
  reference_pair_id: number;
  reference_symbol: string;
  reference_price: string | null;
  reference_observed_at: number | null;
  fallback_reason: string | null;
  switched_at: number;
};
export type DefaultMarketParameters = {
  /** 旧版本响应可省略；写入始终显式发送。 */
  mode?: 'independent' | 'follow';
  follow?: DefaultMarketFollow | null;
  volatility: string;
  mean_reversion: string;
  price_min: string | null;
  price_max: string | null;
  volume_min: string;
  volume_max: string;
  wick_strength: string;
  depth_levels: number;
};
export type DefaultMarketResponse = {
  pair_id: number;
  symbol: string;
  market_type: string;
  configured: boolean;
  version: number;
  enabled: boolean;
  all_market_paused: boolean;
  initial_price: string | null;
  config: DefaultMarketParameters;
  seed: string;
  updated_at: number | null;
  reference_pair?: DefaultMarketReferencePair | null;
  runtime: {
    follow?: DefaultMarketFollowStatus | null;
    generation: number;
    active_source: 'none' | 'default' | 'strategy';
    strategy_id: number | null;
    strategy_version: number | null;
    default_version: number | null;
    last_price: string | null;
    last_tick_at: number | null;
    error_message: string | null;
  };
};
export type DefaultMarketDraft = Omit<DefaultMarketParameters, 'price_min' | 'price_max' | 'depth_levels' | 'mode' | 'follow'> & {
  enabled: boolean;
  mode: 'independent' | 'follow';
  reference_pair_id: string;
  follow_multiplier: string;
  follow_max_move_ratio: string;
  follow_stale_after_seconds: string;
  initial_price: string;
  price_min: string;
  price_max: string;
  depth_levels: string;
};
export type DefaultMarketFollowPreview = {
  kind: 'historical_replay' | 'independent_fallback';
  reference_pair_id: number;
  reference_symbol: string;
  range_start: number;
  range_end: number;
  reference_sample_count: number;
  warning: string | null;
};
export type DefaultMarketPreview = {
  follow_preview?: DefaultMarketFollowPreview | null;
  pair_id: number;
  version: number;
  seed: string;
  start_price: string;
  samples: MarketStrategyPreviewSample[];
};
