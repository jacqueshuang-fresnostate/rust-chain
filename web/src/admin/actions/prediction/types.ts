export type PredictionSettings = {
  sync_enabled: boolean;
  sync_interval_seconds: number;
  sync_tags: string[];
  allowed_asset_ids: number[];
  default_fee_rate: string;
  default_settlement_mode: string;
  default_invalid_refund_policy: string;
  quote_ttl_seconds: number;
  revision: number;
  last_sync_status?: string | null;
  last_sync_error?: string | null;
  last_sync_started_at?: number | null;
  last_sync_finished_at?: number | null;
  last_successful_sync_at?: number | null;
  last_sync_imported_count: number;
  last_sync_updated_count: number;
};

export type PredictionSettingsValues = {
  syncEnabled: boolean;
  syncIntervalSeconds: string;
  syncTags: string;
  allowedAssetIds: string[];
  defaultFeeRate: string;
  defaultSettlementMode: string;
  defaultInvalidRefundPolicy: string;
  quoteTtlSeconds: string;
};

export type PredictionAssetConfig = {
  asset_id: number;
  asset_symbol: string;
  enabled: boolean;
  max_payout_amount: string;
  revision: number;
  updated_at: number;
};

export type PredictionAssetDraft = {
  enabled: boolean;
  maxPayoutAmount: string;
};

export type PredictionSyncLog = {
  id: number;
  trigger_type: string;
  status: string;
  imported_count: number;
  updated_count: number;
  error_message?: string | null;
  started_at: number;
  finished_at?: number | null;
};

export type AssetConfigsResponse = {
  configs: PredictionAssetConfig[];
};

export type SyncLogsResponse = {
  logs: PredictionSyncLog[];
};

export type PredictionTab = 'assets' | 'settings';
