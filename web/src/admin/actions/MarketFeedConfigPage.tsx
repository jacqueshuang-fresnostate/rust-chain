import { Banner, Button, Card, Space, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';

const { Text, Title } = Typography;

const intervalOptions = ['1m', '5m', '15m', '1h', '1d'];
const providerOptions = ['bitget', 'htx'];

const defaultConfigForm = {
  enabled: true,
  intervals: intervalOptions,
  providers: providerOptions,
  symbols: 'BTC-USDT,ETH-USDT'
};

const defaultCredentialForm = {
  apiKey: '',
  apiSecret: '',
  authType: 'api_key',
  enabled: true,
  passphrase: '',
  provider: 'bitget'
};

type MarketFeedConfig = {
  applied_version: number | null;
  enabled: boolean;
  id: number;
  intervals: string[];
  last_reloaded_at?: number | null;
  last_reload_error?: string | null;
  last_reload_status?: string | null;
  name: string;
  needs_reload: boolean;
  providers: string[];
  symbols: string[];
  version: number;
};

type MarketFeedRuntimeStatus = {
  applied_version?: number | null;
  intervals: string[];
  last_reload_error?: string | null;
  last_reload_status?: string | null;
  providers: string[];
  symbols: string[];
};

type MarketFeedStatusResponse = {
  runtime: MarketFeedRuntimeStatus;
  saved_config: MarketFeedConfig | null;
};

type MarketSourceCredential = {
  api_key_mask?: string | null;
  auth_type: string;
  enabled: boolean;
  provider: string;
};

type MarketSourceCredentialsResponse = {
  credentials: MarketSourceCredential[];
};

type ConfigForm = typeof defaultConfigForm;
type CredentialForm = typeof defaultCredentialForm;

function splitCsv(value: string) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function toggleListItem(items: string[], item: string) {
  return items.includes(item) ? items.filter((current) => current !== item) : [...items, item];
}

function joinList(value?: string[]) {
  return value?.length ? value.join(',') : '-';
}

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function configStatus(config: MarketFeedConfig | null) {
  if (!config) {
    return 'pending';
  }
  if (config.needs_reload) {
    return 'needs_reload';
  }
  return config.last_reload_status ?? (config.enabled ? 'active' : 'skipped');
}

async function submitAction(label: string, request: () => Promise<unknown>) {
  try {
    await request();
    Toast.success(`${label}已提交`);
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

export function MarketFeedConfigPage() {
  const [config, setConfig] = useState<MarketFeedConfig | null>(null);
  const [runtime, setRuntime] = useState<MarketFeedRuntimeStatus | null>(null);
  const [credentials, setCredentials] = useState<MarketSourceCredential[]>([]);
  const [configForm, setConfigForm] = useState<ConfigForm>(defaultConfigForm);
  const [credentialForm, setCredentialForm] = useState<CredentialForm>(defaultCredentialForm);
  const [loading, setLoading] = useState(true);

  const credentialSummary = useMemo(
    () => credentials.map((credential) => `${credential.provider}:${credential.api_key_mask ?? credential.auth_type}`).join('，') || '暂无凭证',
    [credentials]
  );

  async function loadPage() {
    setLoading(true);
    try {
      const [status, credentialList] = await Promise.all([
        apiRequest<MarketFeedStatusResponse>('/admin/api/v1/market-feed/status'),
        apiRequest<MarketSourceCredentialsResponse>('/admin/api/v1/market-feed/credentials')
      ]);
      setConfig(status.saved_config);
      setRuntime(status.runtime);
      setCredentials(credentialList.credentials);
      if (status.saved_config) {
        setConfigForm({
          enabled: status.saved_config.enabled,
          intervals: status.saved_config.intervals,
          providers: status.saved_config.providers,
          symbols: status.saved_config.symbols.join(',')
        });
      }
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadPage().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="行情订阅配置" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>订阅配置</Title>
            <div className="admin-action-form">
              <label>
                交易对 symbols
                <input aria-label="交易对 symbols" value={configForm.symbols} onChange={(event) => setConfigForm({ ...configForm, symbols: event.currentTarget.value })} />
              </label>
              <fieldset className="admin-action-choice-group">
                <legend>K线 intervals</legend>
                <div className="admin-action-choice-list">
                  {intervalOptions.map((interval) => (
                    <label key={interval} className="admin-action-checkbox">
                      <input
                        aria-label={interval}
                        checked={configForm.intervals.includes(interval)}
                        type="checkbox"
                        onChange={() => setConfigForm({ ...configForm, intervals: toggleListItem(configForm.intervals, interval) })}
                      />
                      {interval}
                    </label>
                  ))}
                </div>
              </fieldset>
              <fieldset className="admin-action-choice-group">
                <legend>行情 providers</legend>
                <div className="admin-action-choice-list">
                  {providerOptions.map((provider) => (
                    <label key={provider} className="admin-action-checkbox">
                      <input
                        aria-label={provider}
                        checked={configForm.providers.includes(provider)}
                        type="checkbox"
                        onChange={() => setConfigForm({ ...configForm, providers: toggleListItem(configForm.providers, provider) })}
                      />
                      {provider}
                    </label>
                  ))}
                </div>
              </fieldset>
              <label>
                启用状态
                <select aria-label="订阅启用状态" value={configForm.enabled ? 'enabled' : 'disabled'} onChange={(event) => setConfigForm({ ...configForm, enabled: event.currentTarget.value === 'enabled' })}>
                  <option value="enabled">启用</option>
                  <option value="disabled">禁用</option>
                </select>
              </label>
            </div>
            <Space>
              <ConfirmAction
                actionText="保存配置"
                title="确认保存行情订阅配置"
                onConfirm={(reason) =>
                  submitAction('保存行情订阅配置', async () => {
                    const saved = await apiRequest<MarketFeedConfig>('/admin/api/v1/market-feed/config', {
                      method: 'PATCH',
                      body: JSON.stringify({
                        enabled: configForm.enabled,
                        intervals: configForm.intervals,
                        providers: configForm.providers,
                        reason,
                        symbols: splitCsv(configForm.symbols)
                      })
                    });
                    setConfig(saved);
                  })
                }
              />
              <ConfirmAction
                actionText="重载行情订阅"
                disabled={!config}
                title="确认重载第三方行情订阅"
                onConfirm={(reason) =>
                  submitAction('重载行情订阅', async () => {
                    const response = await apiRequest<{ config: MarketFeedConfig; runtime: MarketFeedRuntimeStatus }>('/admin/api/v1/market-feed/reload', {
                      method: 'POST',
                      body: JSON.stringify({ reason })
                    });
                    setConfig(response.config);
                    setRuntime(response.runtime);
                  })
                }
              />
              <Button loading={loading} onClick={() => loadPage().catch((error) => Toast.error(errorMessage(error)))}>
                刷新状态
              </Button>
            </Space>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>运行状态</Title>
            <div className="admin-action-summary">
              <span>配置状态 <StatusTag value={configStatus(config)} /></span>
              <span>配置版本：{config?.version ?? '-'}</span>
              <span>已应用版本：{config?.applied_version ?? runtime?.applied_version ?? '-'}</span>
              <span>最后重载：<TimestampText value={config?.last_reloaded_at ?? null} /></span>
              <span>运行 symbols：{joinList(runtime?.symbols)}</span>
              <span>运行 intervals：{joinList(runtime?.intervals)}</span>
              <span data-testid="runtime-providers">当前启动 providers：{joinList(runtime?.providers)}</span>
            </div>
            {config?.last_reload_error ? <Banner fullMode={false} type="danger" description={config.last_reload_error} /> : null}
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>Provider 凭证</Title>
            <div className="admin-action-form">
              <label>
                Provider
                <select aria-label="凭证 provider" value={credentialForm.provider} onChange={(event) => setCredentialForm({ ...credentialForm, provider: event.currentTarget.value })}>
                  <option value="bitget">bitget</option>
                  <option value="htx">htx</option>
                </select>
              </label>
              <label>
                Auth Type
                <select aria-label="凭证 auth type" value={credentialForm.authType} onChange={(event) => setCredentialForm({ ...credentialForm, authType: event.currentTarget.value })}>
                  <option value="api_key">api_key</option>
                  <option value="none">none</option>
                </select>
              </label>
              <label>
                API Key
                <input aria-label="API Key" value={credentialForm.apiKey} onChange={(event) => setCredentialForm({ ...credentialForm, apiKey: event.currentTarget.value })} />
              </label>
              <label>
                API Secret
                <input aria-label="API Secret" type="password" value={credentialForm.apiSecret} onChange={(event) => setCredentialForm({ ...credentialForm, apiSecret: event.currentTarget.value })} />
              </label>
              <label>
                Passphrase
                <input aria-label="Passphrase" type="password" value={credentialForm.passphrase} onChange={(event) => setCredentialForm({ ...credentialForm, passphrase: event.currentTarget.value })} />
              </label>
              <label>
                凭证状态
                <select aria-label="凭证启用状态" value={credentialForm.enabled ? 'enabled' : 'disabled'} onChange={(event) => setCredentialForm({ ...credentialForm, enabled: event.currentTarget.value === 'enabled' })}>
                  <option value="enabled">启用</option>
                  <option value="disabled">禁用</option>
                </select>
              </label>
            </div>
            <ConfirmAction
              actionText="保存凭证"
              title="确认保存行情源凭证"
              onConfirm={(reason) =>
                submitAction('保存行情源凭证', async () => {
                  const saved = await apiRequest<MarketSourceCredential>(`/admin/api/v1/market-feed/credentials/${credentialForm.provider}`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      api_key: credentialForm.apiKey.trim() || undefined,
                      api_secret: credentialForm.apiSecret.trim() || undefined,
                      auth_type: credentialForm.authType,
                      enabled: credentialForm.enabled,
                      passphrase: credentialForm.passphrase.trim() || undefined,
                      reason
                    })
                  });
                  setCredentials((items) => [saved, ...items.filter((item) => item.provider !== saved.provider)]);
                  setCredentialForm({ ...credentialForm, apiKey: '', apiSecret: '', passphrase: '' });
                })
              }
            />
            <Text type="secondary">当前凭证：{credentialSummary}</Text>
          </Space>
        </Card>
      </div>
    </main>
  );
}
