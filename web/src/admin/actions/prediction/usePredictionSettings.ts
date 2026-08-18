import { Toast } from '@douyinfe/semi-ui';
import { useCallback, useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../../api/client';
import {
  assetConfigPayload,
  assetDraftsFromConfigs,
  settingsPayload,
  settingsToValues
} from './model';
import type {
  AssetConfigsResponse,
  PredictionAssetConfig,
  PredictionAssetDraft,
  PredictionSettings,
  PredictionSettingsValues
} from './types';

export function usePredictionSettings() {
  const [assetConfigs, setAssetConfigs] = useState<PredictionAssetConfig[]>([]);
  const [assetDrafts, setAssetDrafts] = useState<Record<string, PredictionAssetDraft>>({});
  const [loading, setLoading] = useState(true);
  const [conflict, setConflict] = useState<string | null>(null);
  const [settings, setSettings] = useState<PredictionSettings | null>(null);
  const [settingsValues, setSettingsValues] = useState<PredictionSettingsValues | null>(null);

  const loadSettings = useCallback(async () => {
    setLoading(true);
    try {
      const [settingsResponse, assetResponse] = await Promise.all([
        apiRequest<PredictionSettings>('/admin/api/v1/prediction/settings'),
        apiRequest<AssetConfigsResponse>('/admin/api/v1/prediction/asset-configs')
      ]);
      const configs = Array.isArray(assetResponse.configs) ? assetResponse.configs : [];
      setSettings(settingsResponse);
      setSettingsValues(settingsToValues(settingsResponse));
      setAssetConfigs(configs);
      setAssetDrafts(assetDraftsFromConfigs(configs));
      setConflict(null);
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '加载竞猜配置失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  function updateSettingsValues(patch: Partial<PredictionSettingsValues>) {
    setSettingsValues((current) => (current ? { ...current, ...patch } : current));
  }

  function updateAssetDraft(assetId: number, patch: Partial<PredictionAssetDraft>) {
    const key = String(assetId);
    setAssetDrafts((current) => ({
      ...current,
      [key]: {
        enabled: current[key]?.enabled ?? false,
        maxPayoutAmount: current[key]?.maxPayoutAmount ?? '0',
        ...patch
      }
    }));
  }

  async function saveSettings(reason: string) {
    if (!settingsValues || !settings) return;
    try {
      const response = await apiRequest<PredictionSettings>('/admin/api/v1/prediction/settings', {
        method: 'PATCH',
        body: JSON.stringify(settingsPayload(settingsValues, settings.revision, reason))
      });
      setSettings(response);
      setSettingsValues(settingsToValues(response));
      setConflict(null);
      Toast.success('竞猜配置已保存');
    } catch (error) {
      if (error instanceof ApiError && error.status === 409) {
        const message = '全局策略已被其他管理员更新；当前草稿已保留，请重新加载最新配置后再修改。';
        setConflict(message);
        Toast.error(message);
        return;
      }
      Toast.error(error instanceof Error ? error.message : '保存竞猜配置失败');
    }
  }

  async function saveAssetConfig(asset: PredictionAssetConfig, reason: string) {
    const draft = assetDrafts[String(asset.asset_id)];
    if (!draft) return;
    try {
      const updated = await apiRequest<PredictionAssetConfig>('/admin/api/v1/prediction/asset-configs', {
        method: 'POST',
        body: JSON.stringify(assetConfigPayload(asset, draft, reason))
      });
      setAssetConfigs((current) =>
        current.map((item) => (item.asset_id === updated.asset_id ? updated : item))
      );
      setAssetDrafts((current) => ({
        ...current,
        [String(updated.asset_id)]: {
          enabled: updated.enabled,
          maxPayoutAmount: String(updated.max_payout_amount ?? '0')
        }
      }));
      setConflict(null);
      Toast.success(`${asset.asset_symbol} 下注配置已保存`);
    } catch (error) {
      if (error instanceof ApiError && error.status === 409) {
        const message = `${asset.asset_symbol} 配置已被其他管理员更新；当前草稿已保留，请重新加载最新配置后再修改。`;
        setConflict(message);
        Toast.error(message);
        return;
      }
      Toast.error(error instanceof Error ? error.message : '保存资产配置失败');
    }
  }

  return {
    assetConfigs,
    assetDrafts,
    conflict,
    loadSettings,
    loading,
    saveAssetConfig,
    saveSettings,
    settings,
    settingsValues,
    updateAssetDraft,
    updateSettingsValues
  };
}
