import { IconList, IconPulse, IconRefresh, IconSave, IconSetting, IconSync } from '@douyinfe/semi-icons';
import { Banner, Button, Card, Col, Descriptions, Input, Row, Space, Switch, Table, Tabs, Tag, Toast, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { type ComponentPropsWithoutRef, type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';

import { apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { TimestampText } from '../../shared/TimestampText';
import { AdminMultiSelect, AdminSelect, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';
import { StatusTag } from '../../shared/StatusTag';
import { containedTableScroll, containedTableStyle } from '../../shared/tableLayout';

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

type PredictionTab = 'assets' | 'settings' | 'sync';
type PredictionTableProps = ComponentPropsWithoutRef<'table'>;

const settlementModeOptions: SemiSelectOption[] = [
  { value: 'manual_confirm', label: '外部结果 + 人工确认' },
  { value: 'auto', label: '外部结果 + 自动结算' }
];

const invalidRefundPolicyOptions: SemiSelectOption[] = [
  { value: 'refund_stake_and_fee', label: '退本金 + 退手续费' },
  { value: 'refund_stake_only', label: '只退本金' },
  { value: 'manual', label: '无效结算时人工选择' }
];

const predictionTabs = [
  { itemKey: 'settings', tab: '全局策略', icon: <IconSetting aria-hidden="true" /> },
  { itemKey: 'assets', tab: '下注资产', icon: <IconList aria-hidden="true" /> },
  { itemKey: 'sync', tab: '同步任务', icon: <IconPulse aria-hidden="true" /> }
];

const triggerTypeLabels: Record<string, string> = {
  manual: '手动触发',
  scheduled: '定时同步',
  system: '系统触发'
};

const syncStatusMeta: Record<string, { color: 'green' | 'grey' | 'light-blue' | 'orange' | 'red'; label: string }> = {
  failed: { color: 'red', label: '失败' },
  running: { color: 'light-blue', label: '同步中' },
  skipped: { color: 'grey', label: '已跳过' },
  success: { color: 'green', label: '成功' },
  pending: { color: 'orange', label: '待执行' }
};

type FieldColumnSize = 'full' | 'half' | 'third';

const fieldColumnProps: Record<FieldColumnSize, { md?: number; xl?: number; xs: number }> = {
  full: { xs: 24 },
  half: { xs: 24, md: 12 },
  third: { xs: 24, md: 12, xl: 8 }
};

function AssetConfigTable(props: PredictionTableProps) {
  return <table {...props} aria-label="竞猜下注资产配置表" />;
}

function SyncLogTable(props: PredictionTableProps) {
  return <table {...props} aria-label="竞猜同步日志表" />;
}

function FieldLabel({ children, label }: { children: ReactNode; label: string }) {
  return (
    <label style={{ display: 'grid', gap: 6, width: '100%' }}>
      {label}
      {children}
    </label>
  );
}

function FieldColumn({ children, size = 'half' }: { children: ReactNode; size?: FieldColumnSize }) {
  return <Col {...fieldColumnProps[size]}>{children}</Col>;
}

function ConfigGrid({ children }: { children: ReactNode }) {
  return (
    <Row gutter={[24, 18]} style={{ width: '100%' }}>
      {children}
    </Row>
  );
}

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

function optionLabel(options: SemiSelectOption[], value?: string | null) {
  if (!value) return '-';
  return options.find((option) => option.value === value)?.label ?? value;
}

function triggerTypeLabel(value?: string | null) {
  if (!value) return '-';
  return triggerTypeLabels[value] ?? value;
}

function syncStatusTag(value?: string | null) {
  if (!value) return <span>-</span>;
  const meta = syncStatusMeta[value] ?? { color: 'light-blue' as const, label: value };
  return <Tag color={meta.color}>{meta.label}</Tag>;
}

function joinText(items: string[]) {
  return items.length ? items.join('、') : '-';
}

export function PredictionConfigPage() {
  const [activeTab, setActiveTab] = useState<PredictionTab>('settings');
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
      setAssetDrafts((current) => ({
        ...current,
        [String(updated.asset_id)]: { enabled: updated.enabled, maxPayoutAmount: String(updated.max_payout_amount ?? '0') }
      }));
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
        title: '状态',
        width: 150,
        render: (_value, record) => {
          const draft = assetDrafts[String(record.asset_id)];
          return (
            <Switch
              aria-label={`${record.asset_symbol} 允许下注`}
              checked={Boolean(draft?.enabled)}
              checkedText="启用"
              onChange={(checked) =>
                setAssetDrafts((current) => ({
                  ...current,
                  [String(record.asset_id)]: { enabled: checked, maxPayoutAmount: current[String(record.asset_id)]?.maxPayoutAmount ?? '0' }
                }))
              }
              uncheckedText="停用"
            />
          );
        }
      },
      {
        dataIndex: 'max_payout_amount',
        title: '默认最大赔付',
        width: 240,
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
              type="number"
              value={draft?.maxPayoutAmount ?? '0'}
            />
          );
        }
      },
      { dataIndex: 'updated_at', title: '更新时间', width: 180, render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> },
      {
        dataIndex: 'asset_id',
        title: '操作',
        width: 120,
        render: (_value, record) => (
          <Button icon={<IconSave aria-hidden="true" />} loading={savingAssetId === record.asset_id} onClick={() => saveAssetConfig(record)} theme="light" type="primary">
            保存
          </Button>
        )
      }
    ],
    [assetDrafts, savingAssetId]
  );

  const syncLogColumns = useMemo<Array<ColumnProps<PredictionSyncLog>>>(
    () => [
      { dataIndex: 'trigger_type', title: '触发方式', width: 140, render: (value) => <span>{triggerTypeLabel(typeof value === 'string' ? value : null)}</span> },
      { dataIndex: 'status', title: '状态', width: 120, render: (value) => syncStatusTag(typeof value === 'string' ? value : null) },
      { dataIndex: 'imported_count', title: '新增', width: 100 },
      { dataIndex: 'updated_count', title: '更新', width: 100 },
      { dataIndex: 'error_message', title: '错误信息', ellipsis: true, render: (value) => <span>{typeof value === 'string' && value ? value : '-'}</span> },
      { dataIndex: 'started_at', title: '开始时间', width: 180, render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> },
      { dataIndex: 'finished_at', title: '结束时间', width: 180, render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> }
    ],
    []
  );

  const allowedAssetLabels = useMemo(() => {
    const selectedIds = new Set(settingsValues?.allowedAssetIds ?? []);
    return assetConfigs.filter((asset) => selectedIds.has(String(asset.asset_id))).map((asset) => asset.asset_symbol);
  }, [assetConfigs, settingsValues]);

  const enabledAssetCount = useMemo(() => assetConfigs.filter((asset) => asset.enabled).length, [assetConfigs]);

  const overviewData = useMemo(
    () => [
      { key: '同步开关', value: <StatusTag value={settingsValues?.syncEnabled ?? settings?.sync_enabled ?? false} /> },
      { key: '允许资产', value: `${allowedAssetLabels.length} 个` },
      { key: '已启用资产', value: `${enabledAssetCount} / ${assetConfigs.length}` },
      { key: '默认手续费率', value: settingsValues?.defaultFeeRate ?? settings?.default_fee_rate ?? '-' },
      { key: '结算模式', value: optionLabel(settlementModeOptions, settingsValues?.defaultSettlementMode ?? settings?.default_settlement_mode) },
      { key: '最近同步', value: <TimestampText value={settings?.last_successful_sync_at ?? null} /> }
    ],
    [allowedAssetLabels.length, assetConfigs.length, enabledAssetCount, settings, settingsValues]
  );

  const syncData = useMemo(
    () => [
      { key: '最近状态', value: syncStatusTag(settings?.last_sync_status ?? null) },
      { key: '最近成功', value: <TimestampText value={settings?.last_successful_sync_at ?? null} /> },
      { key: '开始时间', value: <TimestampText value={settings?.last_sync_started_at ?? null} /> },
      { key: '结束时间', value: <TimestampText value={settings?.last_sync_finished_at ?? null} /> },
      { key: '新增市场', value: settings?.last_sync_imported_count ?? '-' },
      { key: '更新市场', value: settings?.last_sync_updated_count ?? '-' }
    ],
    [settings]
  );

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={loadPage} theme="borderless">
            刷新
          </Button>
        }
        title="竞猜配置"
      />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
          <Descriptions align="plain" column={6} data={overviewData} layout="horizontal" />
          <Tabs
            activeKey={activeTab}
            className="admin-action-tabs"
            collapsible="auto"
            onChange={(nextTab) => setActiveTab(nextTab as PredictionTab)}
            tabList={predictionTabs}
            type="button"
          />

          {activeTab === 'settings' && settingsValues ? (
            <Space align="start" spacing={18} vertical style={{ width: '100%' }}>
              <div className="admin-action-workbench-grid">
                <section className="admin-action-panel">
                  <Title heading={4}>同步来源</Title>
                  <ConfigGrid>
                    <FieldColumn size="full">
                      <Space align="center" spacing={12}>
                        <Switch
                          aria-label="Polymarket 同步开关"
                          checked={settingsValues.syncEnabled}
                          checkedText="启用"
                          onChange={(checked) => setSettingsValues({ ...settingsValues, syncEnabled: checked })}
                          uncheckedText="停用"
                        />
                        <Text>Polymarket 市场同步</Text>
                      </Space>
                    </FieldColumn>
                    <FieldColumn>
                      <FieldLabel label="同步间隔秒数">
                        <AdminTextInput
                          ariaLabel="同步间隔秒数"
                          onChange={(value) => setSettingsValues({ ...settingsValues, syncIntervalSeconds: value })}
                          type="number"
                          value={settingsValues.syncIntervalSeconds}
                        />
                      </FieldLabel>
                    </FieldColumn>
                    <FieldColumn>
                      <FieldLabel label="报价有效秒数">
                        <AdminTextInput
                          ariaLabel="报价有效秒数"
                          onChange={(value) => setSettingsValues({ ...settingsValues, quoteTtlSeconds: value })}
                          type="number"
                          value={settingsValues.quoteTtlSeconds}
                        />
                      </FieldLabel>
                    </FieldColumn>
                    <FieldColumn size="full">
                      <FieldLabel label="Polymarket 标签或分类">
                        <AdminTextArea
                          ariaLabel="Polymarket 标签或分类"
                          autosize
                          onChange={(value) => setSettingsValues({ ...settingsValues, syncTags: value })}
                          placeholder="每行一个 tag_id 或 tag_slug；留空同步全部活跃市场"
                          value={settingsValues.syncTags}
                        />
                      </FieldLabel>
                    </FieldColumn>
                  </ConfigGrid>
                </section>

                <section className="admin-action-panel">
                  <Title heading={4}>交易与结算</Title>
                  <ConfigGrid>
                    <FieldColumn size="full">
                      <FieldLabel label="全局允许下注资产">
                        <AdminMultiSelect
                          ariaLabel="全局允许下注资产"
                          optionList={assetOptions}
                          placeholder="选择允许下注的虚拟资产"
                          value={settingsValues.allowedAssetIds}
                          onChange={(value) => setSettingsValues({ ...settingsValues, allowedAssetIds: value })}
                        />
                      </FieldLabel>
                      <Text type="secondary">{joinText(allowedAssetLabels)}</Text>
                    </FieldColumn>
                    <FieldColumn>
                      <FieldLabel label="默认手续费率">
                        <AdminTextInput
                          ariaLabel="默认手续费率"
                          onChange={(value) => setSettingsValues({ ...settingsValues, defaultFeeRate: value })}
                          type="number"
                          value={settingsValues.defaultFeeRate}
                        />
                      </FieldLabel>
                    </FieldColumn>
                    <FieldColumn>
                      <FieldLabel label="默认结算模式">
                        <AdminSelect
                          ariaLabel="默认结算模式"
                          onChange={(value) => setSettingsValues({ ...settingsValues, defaultSettlementMode: value })}
                          optionList={settlementModeOptions}
                          value={settingsValues.defaultSettlementMode}
                        />
                      </FieldLabel>
                    </FieldColumn>
                    <FieldColumn size="full">
                      <FieldLabel label="无效市场退款策略">
                        <AdminSelect
                          ariaLabel="无效市场退款策略"
                          onChange={(value) => setSettingsValues({ ...settingsValues, defaultInvalidRefundPolicy: value })}
                          optionList={invalidRefundPolicyOptions}
                          value={settingsValues.defaultInvalidRefundPolicy}
                        />
                      </FieldLabel>
                    </FieldColumn>
                  </ConfigGrid>
                </section>
              </div>
              <Row justify="end" style={{ width: '100%' }} type="flex">
                <Button icon={<IconSave aria-hidden="true" />} loading={savingSettings} onClick={saveSettings} theme="solid" type="primary">
                  保存全局策略
                </Button>
              </Row>
            </Space>
          ) : null}

          {activeTab === 'assets' ? (
            <section className="admin-action-panel">
              <Space align="center" spacing={12} style={{ width: '100%', justifyContent: 'space-between' }}>
                <Title heading={4} style={{ margin: 0 }}>下注资产</Title>
                <Space spacing={8}>
                  <Tag color="green">已启用 {enabledAssetCount}</Tag>
                  <Tag color="grey">共 {assetConfigs.length}</Tag>
                </Space>
              </Space>
              <Table
                aria-label="竞猜下注资产配置表"
                bordered
                columns={assetColumns}
                components={{ body: { outer: AssetConfigTable } }}
                dataSource={assetConfigs}
                loading={loading}
                pagination={false}
                rowKey="asset_id"
                scroll={containedTableScroll}
                style={containedTableStyle}
              />
            </section>
          ) : null}

          {activeTab === 'sync' ? (
            <Space align="start" spacing={18} vertical style={{ width: '100%' }}>
              <section className="admin-action-panel">
                <Row gutter={[24, 16]} style={{ width: '100%' }} type="flex" align="middle" justify="space-between">
                  <Col xs={24} lg={18}>
                    <Title heading={4}>同步任务</Title>
                    <Descriptions align="plain" column={3} data={syncData} layout="horizontal" />
                  </Col>
                  <Col xs={24} lg={6}>
                    <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                      <Button icon={<IconSync aria-hidden="true" />} loading={syncing} onClick={triggerSync} theme="solid" type="primary">
                        立即同步
                      </Button>
                    </Space>
                  </Col>
                </Row>
                {settings?.last_sync_error ? <Banner fullMode={false} type="danger" description={settings.last_sync_error} /> : null}
              </section>

              <section className="admin-action-panel">
                <Title heading={4}>同步日志</Title>
                <Table
                  aria-label="竞猜同步日志表"
                  bordered
                  columns={syncLogColumns}
                  components={{ body: { outer: SyncLogTable } }}
                  dataSource={syncLogs}
                  loading={loading}
                  pagination={false}
                  rowKey="id"
                  scroll={containedTableScroll}
                  style={containedTableStyle}
                />
              </section>
            </Space>
          ) : null}
        </Space>
      </Card>
    </main>
  );
}
