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
import { MarketStrategyDraftDialog } from './MarketStrategyDraftDialog';
import {
  initialMarketStrategy,
  isMarketStrategySubmittable,
  marketStrategyBasePayload,
  nextMarketStrategyStatus
} from './model';
import { marketStrategyActivationError, marketStrategyCreateActivationError } from './runtime';
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
  const activationError = nextStatus === 'active' ? marketStrategyActivationError(record) : null;
  const editor = useMarketStrategyEditor(record, strategyId);
  const [draftAction, setDraftAction] = useState<'close' | 'reset' | null>(null);

  function closeEditor() {
    editor.setVisible(false);
    if (editor.config.status !== recordString(record, 'status')) helpers.reload();
  }

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
      <MarketStrategyVersionSheet onRestored={helpers.reload} strategyId={strategyId} strategyStatus={recordString(record, 'status')} />
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
            if (editor.submitting) return;
            if (editor.dirty) setDraftAction('close');
            else closeEditor();
          }}
          title="修改行情策略"
          visible={editor.visible}
          {...createModalProps('wide')}
          closeOnEsc={!editor.submitting}
        >
          <Card bordered={false}>
            <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
              {editor.config.status === 'active' ? (
                <div key="active-notice" role="alert">策略启用中，请先关闭此窗口，在列表暂停或禁用策略后再修改配置。预览不会改变正在运行的策略。</div>
              ) : (
                <div key="inactive-notice">修改会生成新配置版本，不会自动启用策略；保存后请回到列表按需启用。</div>
              )}
              <div key="configuration" inert={editor.submitting} style={{ width: '100%' }}><MarketStrategyForm
                key="configuration"
                active={editor.visible}
                includePairId={false}
                isEditing
                strategyId={strategyId}
                values={editor.config}
                onChange={editor.setConfig}
              /></div>
              <Button key="reset" disabled={!editor.dirty || editor.submitting} onClick={() => setDraftAction('reset')}>重置未保存修改</Button>
              <ConfirmAction
                key="save"
                actionText="提交修改"
                disabled={editor.submitting || editor.config.status === 'active' || !isMarketStrategySubmittable(editor.config, false)}
                title="确认修改行情策略"
                onConfirm={async (reason) => {
                  if (editor.config.status === 'active') throw new Error('请先暂停或禁用策略后再修改配置');
                  editor.setSubmitting(true);
                  try {
                    await submitAction('修改行情策略', () =>
                      apiRequest(`/admin/api/v1/market-strategies/${strategyId}`, {
                        method: 'PATCH',
                        body: JSON.stringify({ ...marketStrategyBasePayload(editor.config), reason })
                      })
                    );
                    editor.setVisible(false);
                    helpers.reload();
                  } finally { editor.setSubmitting(false); }
                }}
              />
            </Space>
          </Card>
        </SideSheet>
        <MarketStrategyDraftDialog action={draftAction} onCancel={() => setDraftAction(null)} onConfirm={() => {
          editor.reset();
          if (draftAction === 'close') closeEditor();
          setDraftAction(null);
        }} />
        {recordString(record, 'status') === 'active' ? (
          <ConfirmAction
            key="pause"
            actionText="暂停"
            disabled={!strategyId}
            title="暂停行情策略（暂停后可修改配置）"
            description="只暂停此人工策略；若交易对已启用默认行情，会在分钟边界接续生成。需要停止两种生成源时，请在交易对的默认行情中操作「暂停全部行情」。"
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
          disabled={!strategyId || Boolean(activationError)}
          title={activationError ?? `${actionText}行情策略`}
          description={nextStatus === 'active'
            ? `人工策略按保存的起始价 ${recordString(record, 'start_price')} 和目标价运行，不会自动对齐现价。请先核对实时行情并预览；接管可能产生价差，确认代表接受该配置与价差。启用会影响使用此行情的订单触发与结算取价。`
            : '仅禁用此人工策略；若默认行情已启用，会在分钟边界接续。全部停止请使用交易对的「暂停全部行情」。'}
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
  const [submitting, setSubmitting] = useState(false);
  const [resetVisible, setResetVisible] = useState(false);
  const activationError = marketStrategyCreateActivationError(strategy);

  return (
    <>
      <AdminModalTriggerButton onClick={() => setVisible(true)}>创建策略</AdminModalTriggerButton>
      <SideSheet
        onCancel={() => !submitting && setVisible(false)}
        title="创建策略"
        visible={visible}
        {...createModalProps('wide')}
        closeOnEsc={!submitting}
      >
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <p key="draft-notice">关闭窗口会在当前页面保留创建草稿；离开页面后不保留。新建默认保存为草稿，不会自动开始推送。</p>
            <div key="form" inert={submitting} style={{ width: '100%' }}><MarketStrategyForm
              active={visible}
              includePairId
              isEditing={false}
              values={strategy}
              onChange={setStrategy}
            /></div>
            {activationError ? <div key="activation-error" role="alert">{activationError}，或将初始状态改为草稿后保存。</div> : null}
            <Button key="reset" disabled={submitting || JSON.stringify(strategy) === JSON.stringify(initialMarketStrategy)} onClick={() => setResetVisible(true)}>清空创建草稿</Button>
            <ConfirmAction
              key="create"
              actionText="提交创建策略"
              disabled={submitting || Boolean(activationError) || !isMarketStrategySubmittable(strategy, true)}
              title="确认创建行情策略"
              onConfirm={async (reason) => {
                const expired = marketStrategyCreateActivationError(strategy);
                if (expired) throw new Error(expired);
                setSubmitting(true);
                try {
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
                } finally { setSubmitting(false); }
              }}
            />
          </Space>
        </Card>
      </SideSheet>
      <MarketStrategyDraftDialog action={resetVisible ? 'reset' : null} onCancel={() => setResetVisible(false)} onConfirm={() => { setStrategy(initialMarketStrategy); setResetVisible(false); }} />
    </>
  );
}
