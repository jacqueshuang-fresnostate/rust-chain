import { Button, Modal, SideSheet, Space } from '@douyinfe/semi-ui';
import { useContext, useState } from 'react';
import { UNSAFE_DataRouterContext } from 'react-router-dom';
import type { ApiRecord } from '../../../../api/types';
import { ConfirmAction } from '../../../../shared/ConfirmAction';
import { adminErrorMessage } from '../../../../shared/adminErrorMessage';
import { useCanAdminRequest } from '../../../access';
import { UnsavedChangesGuard, useBeforeUnloadGuard } from '../../../settings/UnsavedChangesGuard';
import { createModalProps, recordString, type RowActionHelpers } from '../shared';
import { DefaultMarketFollowForm } from './DefaultMarketFollowForm';
import { DefaultMarketFollowStatus } from './DefaultMarketFollowStatus';
import { useDefaultMarketReferences, validateDefaultMarketReference } from './referencePairs';
import { DefaultMarketForm } from './DefaultMarketForm';
import { DefaultMarketPreview } from './DefaultMarketPreview';
import { validateDefaultMarketDraft } from './model';
import { useDefaultMarket } from './useDefaultMarket';

const sourceLabels = { none: '暂无生成源', default: '默认行情', strategy: '手工策略' };
const timeText = (value: number | null) => value === null ? '暂无记录' : new Date(value).toLocaleString('zh-CN', { hour12: false });

function DefaultMarketEditor({ pairId, record, helpers, onClose }: {
  pairId: string; record: ApiRecord; helpers: RowActionHelpers; onClose: () => void;
}) {
  const state = useDefaultMarket(pairId, helpers.reload);
  const canWrite = useCanAdminRequest(`/admin/api/v1/market-pairs/${pairId}/default-generator`, 'PATCH');
  const canReadReferences = useCanAdminRequest('/admin/api/v1/market-pairs', 'GET');
  const references = useDefaultMarketReferences(canWrite && canReadReferences && state.draft?.mode === 'follow' && !state.loading);
  const dataRouter = useContext(UNSAFE_DataRouterContext);
  const [discard, setDiscard] = useState<'close' | 'reload' | null>(null);
  const pricePrecision = recordString(record, 'price_precision').trim() ? Number(recordString(record, 'price_precision')) : NaN;
  const qtyPrecision = recordString(record, 'qty_precision').trim() ? Number(recordString(record, 'qty_precision')) : NaN;
  const invalid = state.draft ? (canWrite ? validateDefaultMarketReference(state.draft, pairId, references, canReadReferences) : '')
    || validateDefaultMarketDraft(state.draft, pricePrecision, qtyPrecision) : '';
  const disabled = !canWrite || state.loading || state.saving || state.conflict;
  useBeforeUnloadGuard(!dataRouter && (state.dirty || state.saving));

  function request(action: 'close' | 'reload') {
    if (state.saving) return;
    if (state.dirty) setDiscard(action);
    else if (action === 'close') onClose();
    else void state.load();
  }

  return <>
    {dataRouter ? <UnsavedChangesGuard enabled={state.dirty || state.saving} /> : null}
    <SideSheet {...createModalProps('wide')} closeOnEsc={!state.saving} closable={!state.saving} motion={false}
      onCancel={() => request('close')} title={`默认行情 · ${state.value?.symbol ?? recordString(record, 'symbol')}`} visible>
      <div className="admin-market-preview-sheet" aria-busy={state.loading || state.saving}>
        <p>当前生效的手工策略优先；手工策略暂停、结束或没有生效排期时，已启用的默认行情会在分钟边界接续。策略运行异常时不会启动第二个生成源。全部暂停会停止两种生成源。</p>
        <p>生成的 K 线、盘口和成交展示来自行情生成器，不代表实际用户成交。修改默认配置不会解除全部暂停。</p>
        <p>手工策略保留已保存的起始价和目标价，不会自动对齐现价，接管前请核对实时价并预览。默认行情接回时沿用已归档价格；修改参数从新分钟生效。</p>
        <div><Button disabled={state.loading || state.saving} onClick={() => request('reload')}>重新加载最新配置</Button></div>
        {!canWrite ? <p role="status">当前为只读权限，可查看配置与运行状态。</p> : null}
        {state.loading ? <p role="status">正在读取最新配置…</p> : null}
        {state.error ? <div className="admin-inline-error" role="alert">{state.error}</div> : null}
        {state.conflict ? <div className="admin-inline-error" role="alert">配置版本已变更，草稿已保留。请先重新加载最新配置，不会自动覆盖或重试。</div> : null}
        {state.value ? <section aria-label="默认行情运行状态">
          <h3>读取时的运行状态</h3>
          <p>{state.value.configured ? '已配置' : '尚未配置'} · 配置版本 V{state.value.version} · {state.value.enabled ? '默认行情已启用' : '默认行情未启用'} · {state.value.all_market_paused ? '全部行情已暂停' : '未设置全部暂停'}</p>
          <p>生成源：{sourceLabels[state.value.runtime.active_source] ?? state.value.runtime.active_source} · 运行代次：{state.value.runtime.generation}</p>
          <p>手工策略：{state.value.runtime.strategy_id === null ? '无' : `#${state.value.runtime.strategy_id} / V${state.value.runtime.strategy_version}`} · 默认运行版本：{state.value.runtime.default_version === null ? '无' : `V${state.value.runtime.default_version}`}</p>
          <p>最近运行价格：{state.value.runtime.last_price ?? '暂无价格'} · 最近运行时间：{timeText(state.value.runtime.last_tick_at)}</p>
          <p>配置更新时间：{timeText(state.value.updated_at)} · 随机种子：{state.value.seed}</p>
          {state.value.runtime.error_message ? <div role="alert">运行异常：{adminErrorMessage(state.value.runtime.error_message)}</div> : null}
          <DefaultMarketFollowStatus value={state.value} />
          <p className="admin-form-hint">以上为读取时快照，不保证当前仍在更新；可重新加载检查。全部暂停和交易对禁用时不会生成行情。</p>
        </section> : null}
        {state.draft ? <>
          <h3>默认生成参数{state.dirty ? ' · 有未保存更改' : ''}</h3>
          <DefaultMarketFollowForm draft={state.draft} disabled={disabled} edit={state.edit} pairId={pairId}
            references={references.data} referencePair={state.value?.reference_pair ?? null} referenceLoading={references.loading} canRead={canReadReferences} />
          <DefaultMarketForm draft={state.draft} disabled={disabled} edit={state.edit} pricePrecision={pricePrecision} qtyPrecision={qtyPrecision} />
          {invalid ? <div role="alert" className="admin-inline-error">{invalid}</div> : null}
          <div className="admin-market-preview-heading">
            <Button disabled={disabled || Boolean(invalid) || state.previewing} loading={state.previewing} onClick={() => void state.runPreview()}>生成默认行情预览</Button>
            {canWrite ? <ConfirmAction actionText="保存默认行情配置" title="确认保存默认行情配置" disabled={disabled || Boolean(invalid) || !state.dirty}
              description="请先预览并核对价格与成交量。启用后在没有生效人工排期时持续生成，生成价格会进入既有订单触发和结算取价链；模拟成交展示本身不创建真实订单。"
              onConfirm={(reason) => { if (disabled || invalid) return Promise.reject(new Error(invalid || '当前配置暂不可保存，请核对权限和状态。')); return state.mutate(reason); }} /> : null}
          </div>
          {state.previewing ? <p role="status">正在生成一分钟 OHLCV 样本…</p> : null}
          {state.previewError ? <div role="alert" className="admin-inline-error">{state.previewError}</div> : null}
          {state.preview ? <DefaultMarketPreview preview={state.preview} requestedMode={state.draft.mode} /> : null}
          <section aria-label="全部行情暂停控制">
            <h3>全部行情开关</h3>
            <p>独立于默认行情启用状态。恢复全部行情仅解除暂停，仍遵循交易对状态、手工策略优先和默认行情启用条件。暂停只停止生成，不会撤销已有订单、平仓或清除历史缓存。</p>
            {state.dirty ? <p>请先保存草稿，或重新加载并放弃草稿，再操作全部行情开关。</p> : null}
            {canWrite ? <ConfirmAction actionText={state.value?.all_market_paused ? '恢复全部行情' : '暂停全部行情'}
              title={state.value?.all_market_paused ? '确认恢复全部行情' : '确认暂停全部行情'} dangerous={!state.value?.all_market_paused}
              disabled={disabled || state.dirty} onConfirm={(reason) => state.mutate(reason, !state.value?.all_market_paused)} /> : null}
          </section>
        </> : null}
        <Space><Button disabled={state.saving} onClick={() => request('close')}>关闭默认行情</Button></Space>
      </div>
    </SideSheet>
    <Modal visible={discard !== null} title="放弃默认行情草稿？" maskClosable={false} motion={false}
      cancelButtonProps={{ 'aria-label': '继续编辑' }} cancelText="继续编辑" okText="放弃草稿" okButtonProps={{ 'aria-label': '放弃草稿', type: 'danger' }}
      onCancel={() => setDiscard(null)} onOk={() => { const action = discard; setDiscard(null); if (action === 'close') onClose(); else void state.load(); }}>
      未保存的更改将丢失；重新加载后使用服务端最新版本。不会发送保存请求。
    </Modal>
  </>;
}

export function DefaultMarketAction({ record, helpers }: { record: ApiRecord; helpers: RowActionHelpers }) {
  const [visible, setVisible] = useState(false);
  const pairId = recordString(record, 'id');
  return <>
    <Button disabled={!pairId} onClick={() => setVisible(true)} size="small" theme="borderless">默认行情</Button>
    {visible ? <DefaultMarketEditor key={pairId} pairId={pairId} record={record} helpers={helpers} onClose={() => setVisible(false)} /> : null}
  </>;
}
