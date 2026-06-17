import { IconRefresh, IconSave, IconSync } from '@douyinfe/semi-icons';
import { Button, Card, Col, Input, Row, Space, Switch, Table, Tabs, Toast, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { apiRequest } from '../../api/client';
import { TimestampText } from '../../shared/TimestampText';
import { AdminMultiSelect, AdminSelect, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';

const { Title, Text } = Typography;

type PredictionSettings = {
  sync_enabled: boolean;
  sync_interval_seconds: number;
  sync_tags: string[];
  allowed_asset_ids: number[];
  default_fee_rate: string;
  default_settlement_mode: string;
  default_invalid_refund_policy: string;
  quote_ttl_seconds: number;
  last_sync_status?: string | null;
  last_sync_error?: string | null;
  last_sync_started_at?: number | null;
  last_sync_finished_at?: number | null;
  last_successful_sync_at?: number | null;
  last_sync_imported_count: number;
  last_sync_updated_count: number;
};

type PredictionSettingsValues = {
  syncEnabled: boolean;
  syncIntervalSeconds: string;
  syncTags: string;
  allowedAssetIds: string[];
  defaultFeeRate: string;
  defaultSettlementMode: string;
  defaultInvalidRefundPolicy: string;
  quoteTtlSeconds: string;
};

type PredictionAssetConfig = {
  asset_id: number;
  asset_symbol: string;
  enabled: boolean;
  max_payout_amount: string;
  updated_at: number;
};

type PredictionAssetDraft = {
  enabled: boolean;
  maxPayoutAmount: string;
};

type PredictionSyncLog = {
  id: number;
  trigger_type: string;
  status: string;
  imported_count: number;
  updated_count: number;
  error_message?: string | null;
  started_at: number;
  finished_at?: number | null;
};

type AssetConfigsResponse = {
  configs: PredictionAssetConfig[];
};

type SettingsResponse = PredictionSettings;

type SyncLogsResponse = {
  logs: PredictionSyncLog[];
};

const settlementModeOptions: SemiSelectOption[] = [
  { value: 'manual_confirm', label: '外部结果 + 人工确认' },
  { value: 'auto', label: '外部结果 + 自动结算' }
];

const invalidRefundPolicyOptions: SemiSelectOption[] = [
  { value: 'refund_stake_and_fee', label: '退本金 + 退手续费' },
  { value: 'refund_stake_only', label: '只退本金' },
  { value: 'manual', label: '无效结算时人工选择' }
];

function settingsToValues(settings: PredictionSettings): PredictionSettingsValues {
  return {
    syncEnabled: settings.sync_enabled,
    syncIntervalSeconds: String(settings.sync_interval_seconds),
    syncTags: settings.sync_tags.join('\n'),
    allowedAssetIds: settings.allowed_asset_ids.map(String),
    defaultFeeRate: String(settings.default_fee_rate ?? '0'),
    defaultSettlementMode: settings.default_settlement_mode,
    defaultInvalidRefundPolicy: settings.default_invalid_refund_policy,
    quoteTtlSeconds: String(settings.quote_ttl_seconds)
  };
}

function parseTags(value: string): string[] {
  return value
    .split(/[\n,，]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function positiveInteger(value: string, label: string) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
  }
  return parsed;
}

function nonNegativeAmount(value: string, label: string) {
  const trimmed = value.trim();
  if (!trimmed || Number(trimmed) < 0 || Number.isNaN(Number(trimmed))) {
    throw new Error(`${label}必须为非负数字`);
  }
  return trimmed;
}

export function PredictionConfigPage() {
  const [assetConfigs, setAssetConfigs] = useState<PredictionAssetConfig[]>([]);
  const [assetDrafts, setAssetDrafts] = useState<Record<string, PredictionAssetDraft>>({});
  const [loading, setLoading] = useState(true);
  const [settings, setSettings] = useState<PredictionSettings | null>(null);
  const [settingsValues, setSettingsValues] = useState<PredictionSettingsValues | null>(null);
  const [syncLogs, setSyncLogs] = useState<PredictionSyncLog[]>([]);
  const [savingAssetId, setSavingAssetId] = useState<number | null>(null);
  const [savingSettings, setSavingSettings] = useState(false);
  const [syncing, setSyncing] = useState(false);

  const assetOptions = useMemo(
    () => assetConfigs.map((asset) => ({ label: asset.asset_symbol, value: String(asset.asset_id) })),
    [assetConfigs]
  );

  const loadPage = useCallback(async () => {
    setLoading(true);
    try {
      const [settingsResponse, assetResponse, logsResponse] = await Promise.all([
        apiRequest<SettingsResponse>('/admin/api/v1/prediction/settings'),
        apiRequest<AssetConfigsResponse>('/admin/api/v1/prediction/asset-configs'),
        apiRequest<SyncLogsResponse>('/admin/api/v1/prediction/sync/logs?limit=20')
      ]);
      setSettings(settingsResponse);
      setSettingsValues(settingsToValues(settingsResponse));
      setAssetConfigs(assetResponse.configs);
      setAssetDrafts(
        Object.fromEntries(
          assetResponse.configs.map((asset) => [
            String(asset.asset_id),
            { enabled: asset.enabled, maxPayoutAmount: String(asset.max_payout_amount ?? '0') }
          ])
        )
      );
      setSyncLogs(logsResponse.logs);
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '加载竞猜配置失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadPage();
  }, [loadPage]);

  async function saveSettings() {
    if (!settingsValues) return;
    setSavingSettings(true);
    try {
      const response = await apiRequest<SettingsResponse>('/admin/api/v1/prediction/settings', {
        method: 'PATCH',
        body: JSON.stringify({
          sync_enabled: settingsValues.syncEnabled,
          sync_interval_seconds: positiveInteger(settingsValues.syncIntervalSeconds, '同步间隔'),
          sync_tags: parseTags(settingsValues.syncTags),
          allowed_asset_ids: settingsValues.allowedAssetIds.map(Number),
          default_fee_rate: nonNegativeAmount(settingsValues.defaultFeeRate, '默认手续费率'),
          default_settlement_mode: settingsValues.defaultSettlementMode,
          default_invalid_refund_policy: settingsValues.defaultInvalidRefundPolicy,
          quote_ttl_seconds: positiveInteger(settingsValues.quoteTtlSeconds, '报价有效秒数')
        })
      });
      setSettings(response);
      setSettingsValues(settingsToValues(response));
      Toast.success('竞猜配置已保存');
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '保存竞猜配置失败');
    } finally {
      setSavingSettings(false);
    }
  }

  async function saveAssetConfig(asset: PredictionAssetConfig) {
    const draft = assetDrafts[String(asset.asset_id)];
    if (!draft) return;
    setSavingAssetId(asset.asset_id);
    try {
      const updated = await apiRequest<PredictionAssetConfig>('/admin/api/v1/prediction/asset-configs', {
        method: 'POST',
        body: JSON.stringify({
          asset_id: asset.asset_id,
          enabled: draft.enabled,
          max_payout_amount: nonNegativeAmount(draft.maxPayoutAmount, '最大赔付')
        })
      });
      setAssetConfigs((current) => current.map((item) => (item.asset_id === updated.asset_id ? updated : item)));
      Toast.success(`${asset.asset_symbol} 下注配置已保存`);
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '保存资产配置失败');
    } finally {
      setSavingAssetId(null);
    }
  }

  async function triggerSync() {
    setSyncing(true);
    try {
      await apiRequest('/admin/api/v1/prediction/sync', { method: 'POST' });
      Toast.success('已触发 Polymarket 同步');
      await loadPage();
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '同步失败');
    } finally {
      setSyncing(false);
    }
  }

  const assetColumns = useMemo<Array<ColumnProps<PredictionAssetConfig>>>(
    () => [
      { dataIndex: 'asset_symbol', title: '资产' },
      {
        dataIndex: 'enabled',
        title: '允许下注',
        render: (_value, record) => {
          const draft = assetDrafts[String(record.asset_id)];
          return (
            <Switch
              checked={Boolean(draft?.enabled)}
              checkedText="开"
              onChange={(checked) =>
                setAssetDrafts((current) => ({
                  ...current,
                  [String(record.asset_id)]: { enabled: checked, maxPayoutAmount: current[String(record.asset_id)]?.maxPayoutAmount ?? '0' }
                }))
              }
              uncheckedText="关"
            />
          );
        }
      },
      {
        dataIndex: 'max_payout_amount',
        title: '默认最大赔付',
        render: (_value, record) => {
          const draft = assetDrafts[String(record.asset_id)];
          return (
            <Input
              aria-label={`${record.asset_symbol} 默认最大赔付`}
              onChange={(value) =>
                setAssetDrafts((current) => ({
                  ...current,
                  [String(record.asset_id)]: { enabled: current[String(record.asset_id)]?.enabled ?? false, maxPayoutAmount: String(value) }
                }))
              }
              style={{ width: 180 }}
              value={draft?.maxPayoutAmount ?? '0'}
            />
          );
        }
      },
      { dataIndex: 'updated_at', title: '更新时间', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> },
      {
        dataIndex: 'asset_id',
        title: '操作',
        render: (_value, record) => (
          <Button icon={<IconSave />} loading={savingAssetId === record.asset_id} onClick={() => saveAssetConfig(record)} theme="light" type="primary">
            保存
          </Button>
        )
      }
    ],
    [assetDrafts, savingAssetId]
  );

  const syncLogColumns = useMemo<Array<ColumnProps<PredictionSyncLog>>>(
    () => [
      { dataIndex: 'trigger_type', title: '触发方式' },
      { dataIndex: 'status', title: '状态' },
      { dataIndex: 'imported_count', title: '新增' },
      { dataIndex: 'updated_count', title: '更新' },
      { dataIndex: 'error_message', title: '错误信息', render: (value) => <span>{typeof value === 'string' && value ? value : '-'}</span> },
      { dataIndex: 'started_at', title: '开始时间', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> },
      { dataIndex: 'finished_at', title: '结束时间', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> }
    ],
    []
  );

  return (
    <main className="exchange-page">
      <Card bordered={false} className="admin-resource-shell">
        <div className="admin-resource-head">
          <div>
            <Title heading={4} style={{ marginBottom: 6 }}>
              竞猜配置
            </Title>
            <Text type="tertiary">管理 Polymarket 同步、本地下注资产、手续费、封顶和结算策略</Text>
          </div>
          <Space>
            <Button icon={<IconRefresh />} loading={loading} onClick={loadPage} theme="borderless">
              刷新
            </Button>
          </Space>
        </div>
        <Tabs lazyRender type="line">
          <Tabs.TabPane itemKey="settings" tab="全局策略">
            {settingsValues ? (
              <Space align="start" style={{ width: '100%' }} vertical>
                <Switch
                  checked={settingsValues.syncEnabled}
                  checkedText="启用同步"
                  onChange={(checked) => setSettingsValues({ ...settingsValues, syncEnabled: checked })}
                  uncheckedText="关闭同步"
                />
                <Row gutter={[16, 16]}>
                  <Col span={8}>
                    <AdminTextInput ariaLabel="同步间隔秒数" onChange={(value) => setSettingsValues({ ...settingsValues, syncIntervalSeconds: value })} value={settingsValues.syncIntervalSeconds} />
                  </Col>
                  <Col span={8}>
                    <AdminTextInput ariaLabel="默认手续费率" onChange={(value) => setSettingsValues({ ...settingsValues, defaultFeeRate: value })} value={settingsValues.defaultFeeRate} />
                  </Col>
                  <Col span={8}>
                    <AdminTextInput ariaLabel="报价有效秒数" onChange={(value) => setSettingsValues({ ...settingsValues, quoteTtlSeconds: value })} value={settingsValues.quoteTtlSeconds} />
                  </Col>
                  <Col span={12}>
                    <AdminSelect
                      ariaLabel="默认结算模式"
                      onChange={(value) => setSettingsValues({ ...settingsValues, defaultSettlementMode: value })}
                      optionList={settlementModeOptions}
                      value={settingsValues.defaultSettlementMode}
                    />
                  </Col>
                  <Col span={12}>
                    <AdminSelect
                      ariaLabel="无效市场退款策略"
                      onChange={(value) => setSettingsValues({ ...settingsValues, defaultInvalidRefundPolicy: value })}
                      optionList={invalidRefundPolicyOptions}
                      value={settingsValues.defaultInvalidRefundPolicy}
                    />
                  </Col>
                  <Col span={12}>
                    <AdminMultiSelect
                      ariaLabel="全局允许下注资产"
                      optionList={assetOptions}
                      placeholder="选择允许下注的虚拟资产"
                      value={settingsValues.allowedAssetIds}
                      onChange={(value) => setSettingsValues({ ...settingsValues, allowedAssetIds: value })}
                    />
                  </Col>
                  <Col span={12}>
                    <AdminTextArea
                      ariaLabel="Polymarket 标签或分类"
                      autosize
                      onChange={(value) => setSettingsValues({ ...settingsValues, syncTags: value })}
                      placeholder="每行一个 tag_id 或 tag_slug；留空同步全部活跃市场"
                      value={settingsValues.syncTags}
                    />
                  </Col>
                </Row>
                <Button icon={<IconSave />} loading={savingSettings} onClick={saveSettings} theme="solid" type="primary">
                  保存全局策略
                </Button>
              </Space>
            ) : null}
          </Tabs.TabPane>
          <Tabs.TabPane itemKey="assets" tab="下注资产">
            <Table columns={assetColumns} dataSource={assetConfigs} loading={loading} pagination={false} rowKey="asset_id" />
          </Tabs.TabPane>
          <Tabs.TabPane itemKey="sync" tab="同步状态">
            <Space style={{ width: '100%' }} vertical>
              <Row gutter={[16, 16]}>
                <Col span={6}>
                  <Text type="tertiary">最近状态</Text>
                  <Title heading={6}>{settings?.last_sync_status ?? '-'}</Title>
                </Col>
                <Col span={6}>
                  <Text type="tertiary">最近成功</Text>
                  <div><TimestampText value={settings?.last_successful_sync_at ?? null} /></div>
                </Col>
                <Col span={6}>
                  <Text type="tertiary">新增 / 更新</Text>
                  <Title heading={6}>{settings ? `${settings.last_sync_imported_count} / ${settings.last_sync_updated_count}` : '-'}</Title>
                </Col>
                <Col span={6}>
                  <Button icon={<IconSync />} loading={syncing} onClick={triggerSync} theme="solid" type="primary">
                    立即同步
                  </Button>
                </Col>
              </Row>
              {settings?.last_sync_error ? <Text type="danger">{settings.last_sync_error}</Text> : null}
              <Table columns={syncLogColumns} dataSource={syncLogs} loading={loading} pagination={false} rowKey="id" />
            </Space>
          </Tabs.TabPane>
        </Tabs>
      </Card>
    </main>
  );
}
