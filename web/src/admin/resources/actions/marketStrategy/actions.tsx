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
    <>
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
          onCancel={() => editor.setVisible(false)}
          title="修改行情策略"
          visible={editor.visible}
          {...createModalProps('wide')}
        >
          <Card bordered={false}>
            <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
              <MarketStrategyForm
                active={editor.visible}
                includePairId={false}
                isEditing
                strategyId={strategyId}
                values={editor.config}
                onChange={editor.setConfig}
              />
              <ConfirmAction
                actionText="提交修改"
                disabled={!isMarketStrategySubmittable(editor.config, false)}
                title="确认修改行情策略"
                onConfirm={async (reason) => {
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
        <ConfirmAction
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
    </>
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
