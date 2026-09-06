import { Toast } from '@douyinfe/semi-ui';
import { useEffect, useRef, useState } from 'react';

import { apiRequest } from '../../../../api/client';
import type { ApiRecord } from '../../../../api/types';
import { errorMessage } from '../shared';
import { marketStrategyFromRecord } from './model';
import { useBeforeUnloadGuard } from '../../../settings/UnsavedChangesGuard';

export function useMarketStrategyEditor(record: ApiRecord, strategyId: string) {
  const [config, setConfig] = useState(() => marketStrategyFromRecord(record));
  const [loading, setLoading] = useState(false);
  const [visible, setVisible] = useState(false);
  const [baseline, setBaseline] = useState(config);
  const [submitting, setSubmitting] = useState(false);
  const generation = useRef(0);
  const dirty = JSON.stringify(config) !== JSON.stringify(baseline);
  useBeforeUnloadGuard(visible && (dirty || submitting));
  useEffect(() => () => { generation.current += 1; }, []);

  async function openEditor() {
    const requestId = ++generation.current;
    setLoading(true);
    try {
      // 列表不含节点和生成参数；先读详情，避免空节点覆盖已有配置。
      const detail = await apiRequest<ApiRecord>(`/admin/api/v1/market-strategies/${strategyId}`);
      if (requestId !== generation.current) return;
      const latest = marketStrategyFromRecord(detail);
      setConfig(latest);
      setBaseline(latest);
      setVisible(true);
    } catch (error) {
      if (requestId !== generation.current) return;
      Toast.error(errorMessage(error));
    } finally {
      if (requestId === generation.current) setLoading(false);
    }
  }

  return { config, dirty, loading, openEditor, reset: () => setConfig(baseline), setConfig, setVisible, visible, submitting, setSubmitting };
}
