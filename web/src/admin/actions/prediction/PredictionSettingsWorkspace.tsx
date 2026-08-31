import { IconList, IconSetting } from '@douyinfe/semi-icons';
import { Banner, Button, Card, Descriptions, Input, Row, Space, Switch, Tabs, Tag, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { type ComponentPropsWithoutRef, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';

import { PageHeader } from '../../../layouts/PageHeader';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { ResizableTable } from '../../../shared/ResizableTable';
import {
  AdminMultiSelect,
  AdminSelect,
  AdminTextArea,
  AdminTextInput
} from '../../../shared/SemiFormControls';
import { StatusTag } from '../../../shared/StatusTag';
import { TimestampText } from '../../../shared/TimestampText';
import { containedTableStyle } from '../../../shared/tableLayout';
import { WorkflowPageActions } from '../../components/WorkflowPageActions';
import { AdminRequestActionBoundary } from '../../access';
import {
  invalidRefundPolicyOptions,
  joinText,
  optionLabel,
  settlementModeOptions
} from './model';
import {
  PredictionConfigGrid,
  PredictionFieldColumn,
  PredictionFieldLabel
} from './PredictionFields';
import type { PredictionAssetConfig, PredictionTab } from './types';
import { usePredictionSettings } from './usePredictionSettings';

const { Title, Text } = Typography;
type PredictionTableProps = ComponentPropsWithoutRef<'table'>;

const predictionTabs = [
  { itemKey: 'settings', tab: '全局策略', icon: <IconSetting aria-hidden="true" /> },
  { itemKey: 'assets', tab: '下注资产', icon: <IconList aria-hidden="true" /> }
];

function AssetConfigTable(props: PredictionTableProps) {
  return <table {...props} aria-label="竞猜下注资产配置表" />;
}

export function PredictionSettingsWorkspace() {
  const [searchParams, setSearchParams] = useSearchParams();
  const workspace = usePredictionSettings();
  const activeTab: PredictionTab = searchParams.get('tab') === 'assets' && workspace.canReadAssetConfigs ? 'assets' : 'settings';

  function selectTab(nextTab: PredictionTab) {
    const nextParams = new URLSearchParams(searchParams);
    if (nextTab === 'assets') {
      nextParams.set('tab', 'assets');
    } else {
      nextParams.delete('tab');
    }
    setSearchParams(nextParams, { replace: true });
  }

  const assetOptions = useMemo(
    () => workspace.assetConfigs.map((asset) => ({ label: asset.asset_symbol, value: String(asset.asset_id) })),
    [workspace.assetConfigs]
  );
  const allowedAssetLabels = useMemo(() => {
    const selectedIds = new Set(workspace.settingsValues?.allowedAssetIds ?? []);
    return workspace.assetConfigs
      .filter((asset) => selectedIds.has(String(asset.asset_id)))
      .map((asset) => asset.asset_symbol);
  }, [workspace.assetConfigs, workspace.settingsValues]);
  const enabledAssetCount = useMemo(
    () => workspace.assetConfigs.filter((asset) => asset.enabled).length,
    [workspace.assetConfigs]
  );

  const assetColumns = useMemo<Array<ColumnProps<PredictionAssetConfig>>>(
    () => [
      { dataIndex: 'asset_symbol', title: '资产' },
      {
        dataIndex: 'enabled',
        title: '状态',
        width: 150,
        render: (_value, record) => {
          const draft = workspace.assetDrafts[String(record.asset_id)];
          return (
            <Switch
              aria-label={`${record.asset_symbol} 允许下注`}
              checked={Boolean(draft?.enabled)}
              checkedText="启用"
              onChange={(enabled) => workspace.updateAssetDraft(record.asset_id, { enabled })}
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
          const draft = workspace.assetDrafts[String(record.asset_id)];
          return (
            <Input
              aria-label={`${record.asset_symbol} 默认最大赔付`}
              onChange={(value) =>
                workspace.updateAssetDraft(record.asset_id, { maxPayoutAmount: String(value) })
              }
              style={{ width: 180 }}
              type="number"
              value={draft?.maxPayoutAmount ?? '0'}
            />
          );
        }
      },
      {
        dataIndex: 'updated_at',
        title: '更新时间',
        width: 180,
        render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />
      },
      {
        dataIndex: 'asset_id',
        key: 'actions',
        title: '操作',
        width: 120,
        render: (_value, record) => (
          <AdminRequestActionBoundary endpoint="/admin/api/v1/prediction/asset-configs" method="POST">
            <ConfirmAction
              actionText="保存"
              title={`确认保存 ${record.asset_symbol} 下注配置`}
              onConfirm={(reason) => workspace.saveAssetConfig(record, reason)}
            />
          </AdminRequestActionBoundary>
        )
      }
    ],
    [workspace.assetDrafts]
  );

  const overviewData = useMemo(
    () => [
      {
        key: '同步开关',
        value: <StatusTag value={workspace.settingsValues?.syncEnabled ?? workspace.settings?.sync_enabled ?? false} />
      },
      { key: '允许资产', value: `${allowedAssetLabels.length} 个` },
      { key: '已启用资产', value: `${enabledAssetCount} / ${workspace.assetConfigs.length}` },
      {
        key: '默认手续费率',
        value: workspace.settingsValues?.defaultFeeRate ?? workspace.settings?.default_fee_rate ?? '-'
      },
      {
        key: '结算模式',
        value: optionLabel(
          settlementModeOptions,
          workspace.settingsValues?.defaultSettlementMode ?? workspace.settings?.default_settlement_mode
        )
      }
    ],
    [allowedAssetLabels.length, enabledAssetCount, workspace.assetConfigs.length, workspace.settings, workspace.settingsValues]
  );

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <WorkflowPageActions
            loading={workspace.loading}
            onRefresh={() => void workspace.loadSettings()}
            shortcutLabel="前往同步运行"
            shortcutPath="/admin/prediction/sync"
          />
        }
        description="维护全局策略和下注资产；同步执行与运行日志在独立工作区处理。"
        title="竞猜配置"
      />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
          {workspace.conflict ? (
            <Banner
              description={(
                <Space align="center" spacing={12} wrap>
                  <Text>{workspace.conflict}</Text>
                  <Button onClick={() => void workspace.loadSettings()} size="small" theme="solid" type="warning">
                    放弃草稿并重新加载
                  </Button>
                </Space>
              )}
              fullMode={false}
              type="warning"
            />
          ) : null}
          <Descriptions align="plain" column={5} data={overviewData} layout="horizontal" />
          <Tabs
            activeKey={activeTab}
            className="admin-action-tabs"
            collapsible="auto"
            onChange={(nextTab) => selectTab(nextTab as PredictionTab)}
            tabList={workspace.canReadAssetConfigs ? predictionTabs : predictionTabs.filter((tab) => tab.itemKey !== 'assets')}
            type="button"
          />

          {activeTab === 'settings' && workspace.settingsValues ? (
            <Space align="start" spacing={18} vertical style={{ width: '100%' }}>
              <div className="admin-action-workbench-grid">
                <section className="admin-action-panel">
                  <Title heading={4}>同步来源</Title>
                  <PredictionConfigGrid>
                    <PredictionFieldColumn size="full">
                      <Space align="center" spacing={12}>
                        <Switch
                          aria-label="Polymarket 同步开关"
                          checked={workspace.settingsValues.syncEnabled}
                          checkedText="启用"
                          onChange={(syncEnabled) => workspace.updateSettingsValues({ syncEnabled })}
                          uncheckedText="停用"
                        />
                        <Text>Polymarket 市场同步</Text>
                      </Space>
                    </PredictionFieldColumn>
                    <PredictionFieldColumn>
                      <PredictionFieldLabel label="同步间隔秒数">
                        <AdminTextInput
                          ariaLabel="同步间隔秒数"
                          onChange={(syncIntervalSeconds) => workspace.updateSettingsValues({ syncIntervalSeconds })}
                          type="number"
                          value={workspace.settingsValues.syncIntervalSeconds}
                        />
                      </PredictionFieldLabel>
                    </PredictionFieldColumn>
                    <PredictionFieldColumn>
                      <PredictionFieldLabel label="报价有效秒数">
                        <AdminTextInput
                          ariaLabel="报价有效秒数"
                          onChange={(quoteTtlSeconds) => workspace.updateSettingsValues({ quoteTtlSeconds })}
                          type="number"
                          value={workspace.settingsValues.quoteTtlSeconds}
                        />
                      </PredictionFieldLabel>
                    </PredictionFieldColumn>
                    <PredictionFieldColumn size="full">
                      <PredictionFieldLabel label="Polymarket 标签或分类">
                        <AdminTextArea
                          ariaLabel="Polymarket 标签或分类"
                          autosize
                          onChange={(syncTags) => workspace.updateSettingsValues({ syncTags })}
                          placeholder="每行一个 tag_id 或 tag_slug；留空同步全部活跃市场"
                          value={workspace.settingsValues.syncTags}
                        />
                      </PredictionFieldLabel>
                    </PredictionFieldColumn>
                  </PredictionConfigGrid>
                </section>

                <section className="admin-action-panel">
                  <Title heading={4}>交易与结算</Title>
                  <PredictionConfigGrid>
                    <PredictionFieldColumn size="full">
                      <PredictionFieldLabel label="全局允许下注资产">
                        <AdminMultiSelect
                          ariaLabel="全局允许下注资产"
                          onChange={(allowedAssetIds) => workspace.updateSettingsValues({ allowedAssetIds })}
                          optionList={assetOptions}
                          placeholder="选择允许下注的虚拟资产"
                          value={workspace.settingsValues.allowedAssetIds}
                        />
                      </PredictionFieldLabel>
                      <Text type="secondary">{joinText(allowedAssetLabels)}</Text>
                    </PredictionFieldColumn>
                    <PredictionFieldColumn>
                      <PredictionFieldLabel label="默认手续费率">
                        <AdminTextInput
                          ariaLabel="默认手续费率"
                          onChange={(defaultFeeRate) => workspace.updateSettingsValues({ defaultFeeRate })}
                          type="number"
                          value={workspace.settingsValues.defaultFeeRate}
                        />
                      </PredictionFieldLabel>
                    </PredictionFieldColumn>
                    <PredictionFieldColumn>
                      <PredictionFieldLabel label="默认结算模式">
                        <AdminSelect
                          ariaLabel="默认结算模式"
                          onChange={(defaultSettlementMode) => workspace.updateSettingsValues({ defaultSettlementMode })}
                          optionList={settlementModeOptions}
                          value={workspace.settingsValues.defaultSettlementMode}
                        />
                      </PredictionFieldLabel>
                    </PredictionFieldColumn>
                    <PredictionFieldColumn size="full">
                      <PredictionFieldLabel label="无效市场退款策略">
                        <AdminSelect
                          ariaLabel="无效市场退款策略"
                          onChange={(defaultInvalidRefundPolicy) =>
                            workspace.updateSettingsValues({ defaultInvalidRefundPolicy })
                          }
                          optionList={invalidRefundPolicyOptions}
                          value={workspace.settingsValues.defaultInvalidRefundPolicy}
                        />
                      </PredictionFieldLabel>
                    </PredictionFieldColumn>
                  </PredictionConfigGrid>
                </section>
              </div>
              <Row justify="end" style={{ width: '100%' }} type="flex">
                <AdminRequestActionBoundary endpoint="/admin/api/v1/prediction/settings" method="PATCH">
                  <ConfirmAction
                    actionText="保存全局策略"
                    title="确认保存竞猜全局策略"
                    onConfirm={workspace.saveSettings}
                  />
                </AdminRequestActionBoundary>
              </Row>
            </Space>
          ) : null}

          {activeTab === 'assets' && workspace.canReadAssetConfigs ? (
            <section className="admin-action-panel">
              <Space align="center" spacing={12} style={{ width: '100%', justifyContent: 'space-between' }}>
                <Title heading={4} style={{ margin: 0 }}>下注资产</Title>
                <Space spacing={8}>
                  <Tag color="green">已启用 {enabledAssetCount}</Tag>
                  <Tag color="grey">共 {workspace.assetConfigs.length}</Tag>
                </Space>
              </Space>
              <ResizableTable
                aria-label="竞猜下注资产配置表"
                bordered
                columns={assetColumns}
                components={{ body: { outer: AssetConfigTable } }}
                dataSource={workspace.assetConfigs}
                loading={workspace.loading}
                pagination={false}
                rowKey="asset_id"
                style={containedTableStyle}
              />
            </section>
          ) : null}
        </Space>
      </Card>
    </main>
  );
}
