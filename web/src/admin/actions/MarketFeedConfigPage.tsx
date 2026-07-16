import { IconKey, IconList, IconPulse, IconRefresh } from '@douyinfe/semi-icons';
import { Banner, Button, Card, Descriptions, Space, Table, Tabs, Tag, Toast, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { type ComponentPropsWithoutRef, useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminCheckbox, AdminPasswordInput, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';
import { containedTableScroll, containedTableStyle } from '../../shared/tableLayout';

const { Text, Title } = Typography;

const intervalOptions = ['1m', '5m', '15m', '1h', '1d'];
const providerMetaList = [
  { label: 'Bitget 行情', value: 'bitget' },
  { label: 'HTX 行情', value: 'htx' },
  { label: 'Coinbase 行情', value: 'coinbase' }
];
const providerOptions = providerMetaList.map((provider) => provider.value);
const providerSelectOptions = providerMetaList.map(({ label, value }) => ({ label, value }));
const enabledOptions = [
  { value: 'enabled', label: '启用' },
  { value: 'disabled', label: '禁用' }
];
const authTypeOptions = [
  { value: 'api_key', label: 'API Key 鉴权' },
  { value: 'none', label: '无需鉴权' }
];

const defaultConfigForm = {
  enabled: true,
  intervals: intervalOptions,
  providers: [providerOptions[0]],
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
type MarketFeedTab = 'credentials' | 'runtime' | 'subscriptions';
type SubscriptionRow = {
  description?: string;
  enabled: boolean;
  item: string;
  key: string;
  kind: 'global' | 'interval' | 'provider' | 'symbol';
  runtimeEnabled: boolean | null;
  typeLabel: string;
};

type SubscriptionTableProps = ComponentPropsWithoutRef<'table'>;

const marketFeedTabs = [
  { itemKey: 'subscriptions', tab: '订阅配置', icon: <IconList aria-hidden="true" /> },
  { itemKey: 'runtime', tab: '运行状态', icon: <IconPulse aria-hidden="true" /> },
  { itemKey: 'credentials', tab: 'Provider 凭证', icon: <IconKey aria-hidden="true" /> }
];

function SubscriptionTable(props: SubscriptionTableProps) {
  return <table {...props} aria-label="行情订阅列表" />;
}

function CredentialTable(props: SubscriptionTableProps) {
  return <table {...props} aria-label="行情源凭证列表" />;
}

function splitCsv(value: string) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function toggleListItem(items: string[], item: string) {
  return items.includes(item) ? items.filter((current) => current !== item) : [...items, item];
}

function singleProviderSelection(providers: string[]) {
  const provider = providers.find((provider) => providerOptions.includes(provider));
  return provider ? [provider] : [];
}

function joinList(value?: string[]) {
  return value?.length ? value.join(',') : '-';
}

function uniqueItems(items: string[]) {
  return items.filter((item, index) => item.length > 0 && items.indexOf(item) === index);
}

function providerLabel(provider: string) {
  return providerMetaList.find((item) => item.value === provider)?.label ?? provider;
}

function authTypeLabel(authType: string) {
  return authTypeOptions.find((item) => item.value === authType)?.label ?? authType;
}

function includesRuntimeItem(items: string[] | undefined, item: string) {
  return items ? items.includes(item) : false;
}

function subscriptionRows(configForm: ConfigForm, config: MarketFeedConfig | null, runtime: MarketFeedRuntimeStatus | null): SubscriptionRow[] {
  const activeSymbols = splitCsv(configForm.symbols);
  const symbols = uniqueItems([...activeSymbols, ...(config?.symbols ?? [])]);

  return [
    {
      enabled: configForm.enabled,
      item: '全部订阅',
      key: 'global',
      kind: 'global',
      runtimeEnabled: runtime ? (runtime.providers.length > 0 || runtime.symbols.length > 0 || runtime.intervals.length > 0) : null,
      typeLabel: '总开关'
    },
    ...providerOptions.map((provider) => ({
      description: providerLabel(provider),
      enabled: configForm.providers.includes(provider),
      item: provider,
      key: `provider-${provider}`,
      kind: 'provider' as const,
      runtimeEnabled: runtime ? includesRuntimeItem(runtime.providers, provider) : null,
      typeLabel: '行情源'
    })),
    ...symbols.map((symbol) => ({
      enabled: activeSymbols.includes(symbol),
      item: symbol,
      key: `symbol-${symbol}`,
      kind: 'symbol' as const,
      runtimeEnabled: runtime ? includesRuntimeItem(runtime.symbols, symbol) : null,
      typeLabel: '交易对'
    })),
    ...intervalOptions.map((interval) => ({
      enabled: configForm.intervals.includes(interval),
      item: interval,
      key: `interval-${interval}`,
      kind: 'interval' as const,
      runtimeEnabled: runtime ? includesRuntimeItem(runtime.intervals, interval) : null,
      typeLabel: 'K线周期'
    }))
  ];
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
  const [activeTab, setActiveTab] = useState<MarketFeedTab>('subscriptions');
  const [loading, setLoading] = useState(true);

  const rows = useMemo(() => subscriptionRows(configForm, config, runtime), [config, configForm, runtime]);
  const overviewData = useMemo(
    () => [
      { key: '配置状态', value: <StatusTag value={configStatus(config)} /> },
      { key: '订阅开关', value: <StatusTag value={configForm.enabled} /> },
      { key: '当前行情源', value: configForm.providers[0] ? providerLabel(configForm.providers[0]) : '-' },
      { key: '交易对数量', value: splitCsv(configForm.symbols).length },
      { key: 'K线周期', value: joinList(configForm.intervals) },
      { key: '配置版本', value: config?.version ?? '-' }
    ],
    [config, configForm]
  );
  const runtimeData = useMemo(
    () => [
      { key: '运行状态', value: <StatusTag value={runtime?.last_reload_status ?? configStatus(config)} /> },
      { key: '已应用版本', value: config?.applied_version ?? runtime?.applied_version ?? '-' },
      { key: '最后重载', value: <TimestampText value={config?.last_reloaded_at ?? null} /> },
      { key: '运行行情源', value: joinList(runtime?.providers) },
      { key: '运行交易对', value: joinList(runtime?.symbols) },
      { key: '运行K线周期', value: joinList(runtime?.intervals) }
    ],
    [config, runtime]
  );

  function toggleSubscription(row: SubscriptionRow) {
    if (row.kind === 'global') {
      setConfigForm({ ...configForm, enabled: !configForm.enabled });
      return;
    }
    if (row.kind === 'provider') {
      setConfigForm({ ...configForm, providers: row.enabled ? [] : [row.item] });
      return;
    }
    if (row.kind === 'interval') {
      setConfigForm({ ...configForm, intervals: toggleListItem(configForm.intervals, row.item) });
      return;
    }

    const symbols = splitCsv(configForm.symbols);
    setConfigForm({
      ...configForm,
      symbols: row.enabled ? symbols.filter((symbol) => symbol !== row.item).join(',') : uniqueItems([...symbols, row.item]).join(',')
    });
  }

  const subscriptionColumns = useMemo<Array<ColumnProps<SubscriptionRow>>>(
    () => [
      { dataIndex: 'typeLabel', title: '类型', width: 140 },
      {
        dataIndex: 'item',
        title: '订阅项',
        width: 260,
        render: (item: string, row: SubscriptionRow) => (
          <Space spacing={8}>
            <Text strong>{item}</Text>
            {row.description ? <Text type="secondary">{row.description}</Text> : null}
          </Space>
        )
      },
      { dataIndex: 'enabled', title: '配置态', width: 120, render: (enabled: boolean) => <StatusTag value={enabled} /> },
      {
        dataIndex: 'runtimeEnabled',
        title: '运行态',
        width: 120,
        render: (runtimeEnabled: boolean | null) => (runtimeEnabled === null ? '-' : <StatusTag value={runtimeEnabled ? 'active' : 'inactive'} />)
      },
      {
        dataIndex: 'key',
        title: '操作',
        width: 140,
        render: (_value: string, row: SubscriptionRow) => (
          <Button
            aria-label={`${row.enabled ? '禁用' : '启用'} ${row.typeLabel} ${row.item}`}
            onClick={() => toggleSubscription(row)}
            size="small"
            theme="borderless"
          >
            {row.enabled ? '禁用' : '启用'}
          </Button>
        )
      }
    ],
    [configForm]
  );
  const credentialColumns = useMemo<Array<ColumnProps<MarketSourceCredential>>>(
    () => [
      { dataIndex: 'provider', title: '行情源', width: 180, render: (provider: string) => providerLabel(provider) },
      { dataIndex: 'enabled', title: '状态', width: 110, render: (enabled: boolean) => <StatusTag value={enabled} /> },
      { dataIndex: 'auth_type', title: '鉴权方式', width: 160, render: (authType: string) => authTypeLabel(authType) },
      { dataIndex: 'api_key_mask', title: 'Key 掩码', render: (apiKeyMask?: string | null) => apiKeyMask || '-' }
    ],
    []
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
          providers: singleProviderSelection(status.saved_config.providers),
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
      <PageHeader
        actions={
          <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={() => loadPage().catch((error) => Toast.error(errorMessage(error)))} theme="borderless">
            刷新状态
          </Button>
        }
        title="行情订阅配置"
      />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Space align="start" spacing={18} vertical style={{ width: '100%' }}>
          <Descriptions align="plain" column={6} data={overviewData} layout="horizontal" />
          <Tabs activeKey={activeTab} className="admin-action-tabs" onChange={(nextTab) => setActiveTab(nextTab as MarketFeedTab)} tabList={marketFeedTabs} type="button" />

          {activeTab === 'subscriptions' ? (
            <div className="admin-action-workbench-grid">
              <section className="admin-action-panel">
                <Title heading={4}>订阅配置</Title>
                <div className="admin-action-form admin-action-form-wide">
                  <label>
                    启用状态
                    <AdminSelect ariaLabel="订阅启用状态" onChange={(enabled) => setConfigForm({ ...configForm, enabled: enabled === 'enabled' })} optionList={enabledOptions} value={configForm.enabled ? 'enabled' : 'disabled'} />
                  </label>
                  <label>
                    交易对 symbols
                    <AdminTextInput ariaLabel="交易对 symbols" value={configForm.symbols} onChange={(symbols) => setConfigForm({ ...configForm, symbols })} />
                  </label>
                </div>
                <fieldset className="admin-action-choice-group">
                  <legend>
                    <Space spacing={8}>
                      <span>行情 providers</span>
                      <Tag color="light-blue">单选</Tag>
                    </Space>
                  </legend>
                  <div className="admin-action-choice-list">
                    {providerMetaList.map((provider) => (
                      <div key={provider.value} className="admin-action-checkbox">
                        <AdminCheckbox checked={configForm.providers.includes(provider.value)} onChange={(checked) => setConfigForm({ ...configForm, providers: checked ? [provider.value] : [] })}>
                          {provider.value}
                        </AdminCheckbox>
                        <Text type="secondary">{provider.label}</Text>
                      </div>
                    ))}
                  </div>
                </fieldset>
                <fieldset className="admin-action-choice-group">
                  <legend>K线 intervals</legend>
                  <div className="admin-action-choice-list">
                    {intervalOptions.map((interval) => (
                      <div key={interval} className="admin-action-checkbox">
                        <AdminCheckbox checked={configForm.intervals.includes(interval)} onChange={() => setConfigForm({ ...configForm, intervals: toggleListItem(configForm.intervals, interval) })}>
                          {interval}
                        </AdminCheckbox>
                      </div>
                    ))}
                  </div>
                </fieldset>
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
                </Space>
              </section>
              <section className="admin-action-panel">
                <Title heading={4}>订阅列表</Title>
                <Table
                  aria-label="行情订阅列表"
                  bordered
                  columns={subscriptionColumns}
                  components={{ body: { outer: SubscriptionTable } }}
                  dataSource={rows}
                  loading={loading}
                  pagination={false}
                  resizable
                  rowKey="key"
                  scroll={containedTableScroll}
                  style={containedTableStyle}
                />
              </section>
            </div>
          ) : null}

          {activeTab === 'runtime' ? (
            <section className="admin-action-panel">
              <Title heading={4}>运行状态</Title>
              <Descriptions align="plain" column={3} data={runtimeData} layout="horizontal" />
              <div data-testid="runtime-providers">
                <Space spacing={8}>
                  <Text strong>当前启动 providers</Text>
                  {runtime?.providers.length ? runtime.providers.map((provider) => <Tag key={provider}>{providerLabel(provider)}</Tag>) : <Text type="secondary">-</Text>}
                </Space>
              </div>
              {config?.last_reload_error ? <Banner fullMode={false} type="danger" description={config.last_reload_error} /> : null}
            </section>
          ) : null}

          {activeTab === 'credentials' ? (
            <div className="admin-action-workbench-grid">
              <section className="admin-action-panel">
                <Title heading={4}>Provider 凭证</Title>
                <div className="admin-action-form admin-action-form-wide">
                  <label>
                    行情源
                    <AdminSelect ariaLabel="凭证行情源" onChange={(provider) => setCredentialForm({ ...credentialForm, provider })} optionList={providerSelectOptions} value={credentialForm.provider} />
                  </label>
                  <label>
                    鉴权方式
                    <AdminSelect ariaLabel="凭证鉴权方式" onChange={(authType) => setCredentialForm({ ...credentialForm, authType })} optionList={authTypeOptions} value={credentialForm.authType} />
                  </label>
                  <label>
                    凭证状态
                    <AdminSelect ariaLabel="凭证启用状态" onChange={(enabled) => setCredentialForm({ ...credentialForm, enabled: enabled === 'enabled' })} optionList={enabledOptions} value={credentialForm.enabled ? 'enabled' : 'disabled'} />
                  </label>
                  <label>
                    API Key
                    <AdminTextInput ariaLabel="API Key" value={credentialForm.apiKey} onChange={(apiKey) => setCredentialForm({ ...credentialForm, apiKey })} />
                  </label>
                  <label>
                    API Secret
                    <AdminPasswordInput ariaLabel="API Secret" value={credentialForm.apiSecret} onChange={(apiSecret) => setCredentialForm({ ...credentialForm, apiSecret })} />
                  </label>
                  <label>
                    Passphrase
                    <AdminPasswordInput ariaLabel="Passphrase" value={credentialForm.passphrase} onChange={(passphrase) => setCredentialForm({ ...credentialForm, passphrase })} />
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
              </section>
              <section className="admin-action-panel">
                <Title heading={4}>已保存凭证</Title>
                <Table
                  aria-label="行情源凭证列表"
                  bordered
                  columns={credentialColumns}
                  components={{ body: { outer: CredentialTable } }}
                  dataSource={credentials}
                  loading={loading}
                  pagination={false}
                  rowKey="provider"
                  scroll={containedTableScroll}
                  style={containedTableStyle}
                />
              </section>
            </div>
          ) : null}
        </Space>
      </Card>
    </main>
  );
}
