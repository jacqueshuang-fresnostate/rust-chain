import { Toast } from '@douyinfe/semi-ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ApiError, apiRequest } from '../../../api/client';
import {
  createDefaultConfigForm,
  createNewConfigForm,
  formFromConfig,
  legacyCompatibilityDescription,
  payloadFromForm
} from './model';
import type {
  ConfigForm,
  SmtpConfig,
  SmtpConfigListResponse,
  SmtpDeliverySettings,
  SmtpModuleTab,
  SmtpTestResult
} from './types';

function errorMessage(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

export async function submitSmtpAction(label: string, request: () => Promise<unknown>): Promise<void> {
  try {
    await request();
    Toast.success(`${label}已提交`);
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

export function useSmtpConfigWorkspace() {
  const [activeTab, setActiveTab] = useState<SmtpModuleTab>('configs');
  const [configs, setConfigs] = useState<SmtpConfig[]>([]);
  const [configForm, setConfigForm] = useState<ConfigForm>(() => createDefaultConfigForm());
  const [createConfigForm, setCreateConfigForm] = useState<ConfigForm>(() => createDefaultConfigForm());
  const [createSheetVisible, setCreateSheetVisible] = useState(false);
  const [deliveryStrategy, setDeliveryStrategy] = useState('priority');
  const [loading, setLoading] = useState(true);
  const [selectedConfigId, setSelectedConfigIdState] = useState('');
  const selectedConfigIdRef = useRef('');
  const [testConfigChoice, setTestConfigChoice] = useState('strategy');
  const [testRecipient, setTestRecipient] = useState('');
  const [lastTestResult, setLastTestResult] = useState<SmtpTestResult | null>(null);
  const [legacyConfig, setLegacyConfig] = useState<SmtpConfig | null>(null);
  const [legacyReadUnavailable, setLegacyReadUnavailable] = useState(false);

  const setSelectedConfigId = useCallback((value: string) => {
    selectedConfigIdRef.current = value;
    setSelectedConfigIdState(value);
  }, []);

  const loadConfig = useCallback(async (preferredConfigId?: number | 'new') => {
    setLoading(true);
    try {
      const legacyRead = apiRequest<SmtpConfig | null>('/admin/api/v1/smtp/config')
        .then((config) => ({ config, unavailable: false }))
        .catch(() => ({ config: null, unavailable: true }));
      const [saved, legacy] = await Promise.all([
        apiRequest<SmtpConfigListResponse>('/admin/api/v1/smtp/configs'),
        legacyRead
      ]);
      const nextConfigs = saved.configs ?? [];
      const nextStrategy = saved.delivery_settings?.strategy || 'priority';
      const preferredId =
        preferredConfigId === undefined ? selectedConfigIdRef.current : String(preferredConfigId);
      const nextSelected = nextConfigs.find((item) => String(item.id) === preferredId);
      const fallbackSelected = nextSelected ?? nextConfigs[0] ?? null;

      setConfigs(nextConfigs);
      setLegacyConfig(legacy.config);
      setLegacyReadUnavailable(legacy.unavailable);
      setDeliveryStrategy(nextStrategy);
      setSelectedConfigId(fallbackSelected ? String(fallbackSelected.id) : '');
      setConfigForm(fallbackSelected ? formFromConfig(fallbackSelected) : createNewConfigForm(nextConfigs.length));
      setTestConfigChoice((current) =>
        current === 'strategy' || nextConfigs.some((item) => String(item.id) === current)
          ? current
          : 'strategy'
      );
    } finally {
      setLoading(false);
    }
  }, [setSelectedConfigId]);

  useEffect(() => {
    void loadConfig().catch((error) => Toast.error(errorMessage(error)));
  }, [loadConfig]);

  const selectedConfig = useMemo(
    () => configs.find((config) => String(config.id) === selectedConfigId) ?? null,
    [configs, selectedConfigId]
  );

  const compatibility = useMemo(
    () => legacyCompatibilityDescription(loading, legacyReadUnavailable, legacyConfig, configs),
    [configs, legacyConfig, legacyReadUnavailable, loading]
  );

  function selectConfig(config: SmtpConfig) {
    setSelectedConfigId(String(config.id));
    setConfigForm(formFromConfig(config));
  }

  function startCreateConfig() {
    setCreateConfigForm(createNewConfigForm(configs.length));
    setCreateSheetVisible(true);
  }

  async function saveCurrentConfig(reason: string) {
    if (!selectedConfigId) throw new Error('请先选择发信配置');
    const saved = await apiRequest<SmtpConfig>(`/admin/api/v1/smtp/configs/${selectedConfigId}`, {
      method: 'PATCH',
      body: JSON.stringify(payloadFromForm(configForm, reason))
    });
    await loadConfig(saved.id);
  }

  async function createConfig(reason: string) {
    const saved = await apiRequest<SmtpConfig>('/admin/api/v1/smtp/configs', {
      method: 'POST',
      body: JSON.stringify(payloadFromForm(createConfigForm, reason))
    });
    setCreateSheetVisible(false);
    setCreateConfigForm(createNewConfigForm(configs.length + 1));
    await loadConfig(saved.id);
  }

  async function toggleConfigEnabled(config: SmtpConfig, enabled: boolean, reason: string) {
    const saved = await apiRequest<SmtpConfig>(`/admin/api/v1/smtp/configs/${config.id}`, {
      method: 'PATCH',
      body: JSON.stringify(payloadFromForm({ ...formFromConfig(config), enabled }, reason))
    });
    await loadConfig(saved.id);
  }

  async function saveDeliverySettings(reason: string) {
    const saved = await apiRequest<SmtpDeliverySettings>('/admin/api/v1/smtp/delivery-settings', {
      method: 'PATCH',
      body: JSON.stringify({ strategy: deliveryStrategy, reason })
    });
    setDeliveryStrategy(saved.strategy);
  }

  async function sendTest(reason: string) {
    const body = {
      recipient: testRecipient.trim(),
      reason,
      ...(testConfigChoice === 'strategy' ? {} : { config_id: Number(testConfigChoice) })
    };
    const response = await apiRequest<{
      config_id: number;
      config_name: string;
      recipient: string;
      sent: boolean;
    }>('/admin/api/v1/smtp/test', {
      method: 'POST',
      body: JSON.stringify(body)
    });
    setLastTestResult({ recipient: response.recipient, configName: response.config_name });
  }

  return {
    activeTab,
    compatibility,
    configForm,
    configs,
    createConfig,
    createConfigForm,
    createSheetVisible,
    deliveryStrategy,
    lastTestResult,
    loadConfig,
    loading,
    saveCurrentConfig,
    saveDeliverySettings,
    selectedConfig,
    selectedConfigId,
    selectConfig,
    sendTest,
    setActiveTab,
    setConfigForm,
    setCreateConfigForm,
    setCreateSheetVisible,
    setDeliveryStrategy,
    setTestConfigChoice,
    setTestRecipient,
    startCreateConfig,
    testConfigChoice,
    testRecipient,
    toggleConfigEnabled
  };
}
