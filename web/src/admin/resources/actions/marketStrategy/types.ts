import type { MarketStrategyNodeDraft } from '../../../components/MarketStrategyNodeEditor';

export type MarketStrategyValues = {
  endTime: string;
  meanReversionStrength: string;
  nodes: MarketStrategyNodeDraft[];
  noiseScale: string;
  pairId: string;
  regenerateSeed: boolean;
  scenario: string;
  seed: string;
  seedMode: string;
  startPrice: string;
  startTime: string;
  status: string;
  strategyType: string;
  targetPrice: string;
  volatility: string;
  volumeMax: string;
  volumeMin: string;
  volumeShape: string;
  wickScale: string;
};

export type MarketStrategyNodeRecord = {
  execution_mode?: unknown;
  sequence_no?: unknown;
  target_time?: unknown;
  target_type?: unknown;
  target_value?: unknown;
  tolerance?: unknown;
  volatility?: unknown;
  volume_max?: unknown;
  volume_min?: unknown;
};

export type MarketStrategyGeneratorRecord = {
  mean_reversion_strength?: unknown;
  noise_scale?: unknown;
  scenario?: unknown;
  seed?: unknown;
  seed_mode?: unknown;
  volume_shape?: unknown;
  wick_scale?: unknown;
};

export type MarketStrategyPresetNode = {
  execution_mode: string;
  progress_percent: number;
  target_type: string;
  target_value: string;
  tolerance: string;
  volatility: string;
  volume_max: string | null;
  volume_min: string | null;
};

export type MarketStrategyPreset = {
  code: string;
  description: string;
  generator: Omit<MarketStrategyGeneratorRecord, 'seed'>;
  name: string;
  nodes: MarketStrategyPresetNode[];
  target_price_change_percent: string;
};

export type MarketStrategyPresetsResponse = {
  presets: MarketStrategyPreset[];
};

export type MarketStrategyPreviewSample = {
  close: string;
  high: string;
  low: string;
  open: string;
  open_time: number;
  volume: string;
};

export type MarketStrategyPreviewResponse = {
  one_minute_count: number;
  preview_seed: string;
  preview_version: number;
  sample_count: number;
  samples: MarketStrategyPreviewSample[];
};
