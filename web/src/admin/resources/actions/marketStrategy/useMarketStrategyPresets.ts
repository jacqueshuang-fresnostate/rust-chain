import { useEffect, useState } from 'react';

import { apiRequest } from '../../../../api/client';
import { errorMessage } from '../shared';
import type { MarketStrategyPreset, MarketStrategyPresetsResponse } from './types';

export function useMarketStrategyPresets(active: boolean) {
  const [presets, setPresets] = useState<MarketStrategyPreset[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [requested, setRequested] = useState(false);

  useEffect(() => {
    if (!active || requested) return;
    setRequested(true);
    setLoading(true);
    setError('');
    apiRequest<MarketStrategyPresetsResponse>('/admin/api/v1/market-strategies/presets')
      .then((result) => setPresets(Array.isArray(result.presets) ? result.presets : []))
      .catch((loadError: unknown) => setError(errorMessage(loadError)))
      .finally(() => setLoading(false));
  }, [active, requested]);

  return {
    error,
    loading,
    presets,
    reload: () => setRequested(false)
  };
}
