import { Button, Card, SideSheet, Space } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../../api/client';
import type { ApiRecord } from '../../../../api/types';
import { AdminRequestActionBoundary } from '../../../access';
import { ConfirmAction } from '../../../../shared/ConfirmAction';
import { AdminModalTriggerButton } from '../../../../shared/SemiFormControls';
import { MarketStrategyRecoverySheet } from '../../../components/MarketStrategyRecoverySheet';
import { MarketStrategyVersionSheet } from '../../../components/MarketStrategyVersionSheet';
import {
  type RowActionHelpers,
  createModalProps,
  openRecordDetail,
  recordString,
  requiredPositiveInteger,
  submitAction,
  toggleActionText
} from '../shared';
import { MarketStrategyForm } from './MarketStrategyForm';
import {
  initialMarketStrategy,
  isMarketStrategySubmittable,
  marketStrategyBasePayload,
  nextMarketStrategyStatus
} from './model';
import { useMarketStrategyEditor } from './useMarketStrategyEditor';

export function MarketStrategyRowActions({
  helpers,
  record
}: {
  helpers: RowActionHelpers;
  record: ApiRecord;
}) {
  const strategyId = recordString(record, 'id');
  const nextStatus = nextMarketStrategyStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);
  const editor = useMarketStrategyEditor(record, strategyId);

  return (
    <div className="admin-market-strategy-row-actions">
      <Button
        disabled={!strategyId}
        onClick={() => openRecordDetail('/admin/api/v1/market-strategies', strategyId, helpers)}
        size="small"
        theme="borderless"
      >
        查看详情
      </Button>
      <MarketStrategyRecoverySheet strategyId={strategyId} />
      <MarketStrategyVersionSheet onRestored={helpers.reload} strategyId={strategyId} />
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/market-strategies/${strategyId}`} method="PATCH">
        <Button
          disabled={!strategyId}
          loading={editor.loading}
          onClick={() => void editor.openEditor()}
          size="small"
          theme="borderless"
        >
          修改
        </Button>
        <SideSheet
          onCancel={() => {
            editor.setVisible(false);
            if (editor.config.status !== recordString(record, 'status')) helpers.reload();
          }}
          title="修改行情策略"
          visible={editor.visible}
          {...createModalProps('wide')}
        >
          <Card bordered={false}>
            <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
              {editor.config.status === 'active' ? (
                <div key="active-notice" role="alert">策略启用中，请先关闭此窗口，在列表暂停或禁用策略后再修改配置。预览不会改变正在运行的策略。</div>
              ) : (
                <div key="inactive-notice">修改会生成新配置版本，不会自动启用策略；保存后请回到列表按需启用。</div>
              )}
              <MarketStrategyForm
                key="configuration"
                active={editor.visible}
                includePairId={false}
                isEditing
                strategyId={strategyId}
                values={editor.config}
                onChange={editor.setConfig}
              />
              <ConfirmAction
                key="save"
                actionText="提交修改"
                disabled={editor.config.status === 'active' || !isMarketStrategySubmittable(editor.config, false)}
                title="确认修改行情策略"
                onConfirm={async (reason) => {
                  if (editor.config.status === 'active') throw new Error('请先暂停或禁用策略后再修改配置');
                  await submitAction('修改行情策略', () =>
                    apiRequest(`/admin/api/v1/market-strategies/${strategyId}`, {
                      method: 'PATCH',
                      body: JSON.stringify({ ...marketStrategyBasePayload(editor.config), reason })
                    })
                  );
                  editor.setVisible(false);
                  helpers.reload();
                }}
              />
            </Space>
          </Card>
        </SideSheet>
        {recordString(record, 'status') === 'active' ? (
          <ConfirmAction
            key="pause"
            actionText="暂停"
            disabled={!strategyId}
            title="暂停行情策略（停止实时生成后可修改配置）"
            onConfirm={async (reason) => {
              await submitAction('暂停行情策略', () =>
                apiRequest(`/admin/api/v1/market-strategies/${strategyId}/status`, {
                  method: 'PATCH', body: JSON.stringify({ status: 'paused', reason })
                })
              );
              helpers.reload();
            }}
          />
        ) : null}
        <ConfirmAction
          key="toggle"
          actionText={actionText}
          disabled={!strategyId}
          title={`${actionText}行情策略`}
          onConfirm={async (reason) => {
            await submitAction(`${actionText}行情策略`, () =>
              apiRequest(`/admin/api/v1/market-strategies/${strategyId}/status`, {
                method: 'PATCH',
                body: JSON.stringify({ status: nextStatus, reason })
              })
            );
            helpers.reload();
          }}
        />
      </AdminRequestActionBoundary>
    </div>
  );
}

export function CreateMarketStrategyAction({ onCreated }: { onCreated?: () => void }) {
  const [strategy, setStrategy] = useState(initialMarketStrategy);
  const [visible, setVisible] = useState(false);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>创建策略</AdminModalTriggerButton>
      <SideSheet
        onCancel={() => setVisible(false)}
        title="创建策略"
        visible={visible}
        {...createModalProps('wide')}
      >
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <MarketStrategyForm
              active={visible}
              includePairId
              isEditing={false}
              values={strategy}
              onChange={setStrategy}
            />
            <ConfirmAction
              actionText="提交创建策略"
              disabled={!isMarketStrategySubmittable(strategy, true)}
              title="确认创建行情策略"
              onConfirm={async (reason) => {
                await submitAction('创建行情策略', () =>
                  apiRequest('/admin/api/v1/market-strategies', {
                    method: 'POST',
                    body: JSON.stringify({
                      pair_id: requiredPositiveInteger(strategy.pairId, '交易对ID'),
                      ...marketStrategyBasePayload(strategy),
                      status: strategy.status,
                      reason
                    })
                  })
                );
                setVisible(false);
                setStrategy(initialMarketStrategy);
                onCreated?.();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}
