import { Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../../api/client';
import type { ApiRecord } from '../../../../api/types';
import { errorMessage } from '../shared';
import { marketStrategyFromRecord } from './model';

export function useMarketStrategyEditor(record: ApiRecord, strategyId: string) {
  const [config, setConfig] = useState(() => marketStrategyFromRecord(record));
  const [loading, setLoading] = useState(false);
  const [visible, setVisible] = useState(false);

  async function openEditor() {
    setLoading(true);
    try {
      // 列表不含节点和生成参数；先读详情，避免空节点覆盖已有配置。
      const detail = await apiRequest<ApiRecord>(`/admin/api/v1/market-strategies/${strategyId}`);
      setConfig(marketStrategyFromRecord(detail));
      setVisible(true);
    } catch (error) {
      Toast.error(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  return { config, loading, openEditor, setConfig, setVisible, visible };
}
