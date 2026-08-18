import { Toast } from '@douyinfe/semi-ui';
import { useCallback, useEffect, useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { PredictionSettings, PredictionSyncLog, SyncLogsResponse } from './types';

export function usePredictionSync() {
  const [loading, setLoading] = useState(true);
  const [settings, setSettings] = useState<PredictionSettings | null>(null);
  const [syncLogs, setSyncLogs] = useState<PredictionSyncLog[]>([]);
  const [syncing, setSyncing] = useState(false);

  const loadSync = useCallback(async () => {
    setLoading(true);
    try {
      const [settingsResponse, logsResponse] = await Promise.all([
        apiRequest<PredictionSettings>('/admin/api/v1/prediction/settings'),
        apiRequest<SyncLogsResponse>('/admin/api/v1/prediction/sync/logs?limit=20')
      ]);
      setSettings(settingsResponse);
      setSyncLogs(Array.isArray(logsResponse.logs) ? logsResponse.logs : []);
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '加载竞猜同步运行信息失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadSync();
  }, [loadSync]);

  async function triggerSync() {
    setSyncing(true);
    try {
      await apiRequest('/admin/api/v1/prediction/sync', { method: 'POST' });
      Toast.success('已触发 Polymarket 同步');
      await loadSync();
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '同步失败');
    } finally {
      setSyncing(false);
    }
  }

  return { loadSync, loading, settings, syncing, syncLogs, triggerSync };
}
