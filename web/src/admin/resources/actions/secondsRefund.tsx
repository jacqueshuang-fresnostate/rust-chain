import { Button, SideSheet, Toast } from '@douyinfe/semi-ui';
import { useEffect, useRef, useState, useSyncExternalStore } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { authStore, type AuthSession } from '../../../auth/authStore';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminSwitch, AdminTextInput } from '../../../shared/SemiFormControls';
import { adminErrorMessage } from '../../../shared/adminErrorMessage';
import { compareDecimalText } from '../../../shared/decimal';
import { FinancialCommandIntentStore, financialCommandScopeFromSession, runRecoverableFinancialCommand } from '../../../shared/idempotency';
import { AdminRequestActionBoundary } from '../../access';
import { recordString, type RowActionHelpers } from './shared';

type Policy = { product_id: number; version: number; enabled: boolean; wait_seconds: number | null };
type Context = { order_id: number; policy_version: number | null; wait_seconds: number | null; eligible_at: number | null; checked_at: number; receipt: ApiRecord | null };
const uint = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const wait = (value: unknown): value is number => uint(value) && value <= 4294967295;
const modalWidth = 'min(480px, calc(100vw - 32px))';

export function parseSecondsRefundPolicy(value: unknown, id: string): Policy {
  const p = value as Policy | null;
  if (!p || String(p.product_id) !== id || !uint(p.version) || typeof p.enabled !== 'boolean' ||
      (p.enabled ? !wait(p.wait_seconds) || p.version === 0 : p.wait_seconds !== null)) {
    throw new Error('退款策略响应不完整，禁止提交');
  }
  return p;
}

export function parseSecondsRefundContext(value: unknown, id: string): Context {
  const c = value as Context | null;
  if (!c || String(c.order_id) !== id || !uint(c.checked_at) ||
      !(c.eligible_at === null || uint(c.eligible_at)) ||
      !(c.policy_version === null ? c.wait_seconds === null && c.eligible_at === null : uint(c.policy_version) && c.policy_version > 0 && wait(c.wait_seconds)) ||
      !(c.receipt === null || typeof c.receipt === 'object' && String(c.receipt.order_id) === id)) {
    throw new Error('退款资格响应不完整，禁止提交');
  }
  return c;
}

function PolicyEditor({ productId, helpers, session }: { productId: string; helpers: RowActionHelpers; session: AuthSession }) {
  const [visible, setVisible] = useState(false);
  const [policy, setPolicy] = useState<Policy | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [seconds, setSeconds] = useState('');
  const [error, setError] = useState('');
  const endpoint = `/admin/api/v1/seconds-contracts/products/${productId}/refund-policy`;
  const flight = useRef(false);
  useEffect(() => {
    if (!visible) return;
    const controller = new AbortController();
    setPolicy(null);
    setError('');
    void apiRequest(endpoint, { signal: controller.signal }).then((response) => {
      if (controller.signal.aborted) return;
      const next = parseSecondsRefundPolicy(response, productId);
      setPolicy(next);
      setEnabled(next.enabled);
      setSeconds(next.wait_seconds === null ? '' : String(next.wait_seconds));
    }).catch((cause: unknown) => {
      if (!controller.signal.aborted) setError(adminErrorMessage(cause, '加载退款策略失败'));
    });
    return () => controller.abort();
  }, [visible, endpoint, productId]);
  const validWait = /^\d+$/.test(seconds) && wait(Number(seconds));
  return <>
    <Button size="small" theme="borderless" onClick={() => setVisible(true)}>本金退款策略</Button>
    <SideSheet title="本金退款策略" visible={visible} width={modalWidth} onCancel={() => { if (!flight.current) setVisible(false); }}>
      {error ? <div role="alert">{error}</div> : null}
      {!policy && !error ? <p role="status">加载中</p> : null}
      {policy ? <div className="admin-action-form" style={{ gridTemplateColumns: 'minmax(0, 1fr)' }}>
        <p>当前版本：{policy.version}。仅新订单保存策略；历史订单不补录。停用不改变已保存的订单条款。</p>
        <AdminSwitch checked={enabled} label="允许无证据人工审核单退还本金" onChange={setEnabled} />
        <label>首次人工审核后等待秒数<AdminTextInput ariaLabel="首次人工审核后等待秒数" type="number" disabled={!enabled} value={seconds} onChange={setSeconds} /></label>
        <AdminRequestActionBoundary endpoint={endpoint} method="PATCH">
          <ConfirmAction actionText="保存退款策略" modalWidth={modalWidth} disabled={enabled && !validWait}
            title="确认修改未来订单退款策略"
            description={enabled ? `未来新订单等待 ${seconds} 秒后才可申请无证据本金退款；不自动执行，不包含盈利。` : '停止为新订单保存退款资格；已有快照保持不变。'}
            onConfirm={async (reason) => {
              if (flight.current) return;
              if (authStore.getSession('admin')?.generation !== session.generation) throw new Error('管理员会话已失效');
              if ([...reason].length > 512 || enabled && !validWait) throw new Error('请核对等待秒数及原因');
              flight.current = true;
              try {
                const result = await apiRequest(endpoint, { method: 'PATCH', body: JSON.stringify({
                  expected_version: policy.version, enabled, wait_seconds: enabled ? Number(seconds) : null, reason
                }) });
                const updated = parseSecondsRefundPolicy(result, productId);
                if (updated.version !== policy.version + 1 || updated.enabled !== enabled ||
                    updated.wait_seconds !== (enabled ? Number(seconds) : null)) throw new Error('策略保存结果未确认，请重新读取核对');
                Toast.success('退款策略已保存');
                setVisible(false);
                helpers.reload();
              } finally { flight.current = false; }
            }} />
        </AdminRequestActionBoundary>
      </div> : null}
    </SideSheet>
  </>;
}

export function SecondsRefundPolicyAction({ productId, helpers }: { productId: string; helpers: RowActionHelpers }) {
  const session = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('admin'));
  if (!session) return null;
  return <AdminRequestActionBoundary endpoint={`/admin/api/v1/seconds-contracts/products/${productId}/refund-policy`} method="GET">
    <PolicyEditor key={`${session.generation}:${productId}`} productId={productId} helpers={helpers} session={session} />
  </AdminRequestActionBoundary>;
}

function RefundEditor({ record, helpers, session }: { record: ApiRecord; helpers: RowActionHelpers; session: AuthSession }) {
  const id = recordString(record, 'id');
  const status = recordString(record, 'status');
  const asset = recordString(record, 'stake_asset');
  const amount = recordString(record, 'stake_amount');
  const endpoint = `/admin/api/v1/seconds-contracts/orders/${id}/principal-refund`;
  const [visible, setVisible] = useState(false);
  const [context, setContext] = useState<Context | null>(null);
  const [error, setError] = useState('');
  const flight = useRef(false);
  useEffect(() => {
    if (!visible) return;
    const controller = new AbortController();
    setContext(null);
    setError('');
    void apiRequest(endpoint, { signal: controller.signal }).then((response) => {
      if (!controller.signal.aborted) setContext(parseSecondsRefundContext(response, id));
    }).catch((cause: unknown) => {
      if (!controller.signal.aborted) setError(adminErrorMessage(cause, '加载退款资格失败'));
    });
    return () => controller.abort();
  }, [visible, endpoint, id]);
  const ready = Boolean(context?.policy_version && context.eligible_at !== null && context.eligible_at <= context.checked_at);
  return <>
    <Button size="small" theme="borderless" onClick={() => setVisible(true)}>{status === 'refunded' ? '退款凭证' : '本金退还'}</Button>
    <SideSheet title="无证据订单本金退还" visible={visible} width={modalWidth} onCancel={() => { if (!flight.current) setVisible(false); }}>
      {error ? <div role="alert">{error}</div> : null}
      {!context && !error ? <p role="status">加载中</p> : null}
      {context ? <>
        <p>原本金：{amount} {recordString(record, 'stake_asset_symbol')}。不包含盈利，不指定输赢。</p>
        {context.policy_version === null ? <p>开仓时未保存退款策略，本单不可退款。</p> : <>
          <p>开仓策略版本：{context.policy_version}；等待间隔：{context.wait_seconds} 秒。</p>
          <p>最早申请时间：{context.eligible_at === null ? '缺少原始审核证据' : new Date(context.eligible_at).toLocaleString()}</p>
        </>}
        {context.receipt ? <p>已有不可变退款凭证；重复操作仅用于核对原未决请求。</p> : null}
        <ConfirmAction dangerous actionText={context.receipt ? '核对退款' : '确认退还本金'} title="确认仅退还原本金"
          modalWidth={modalWidth} disabled={!ready || !asset}
          description="提交时重新核查历史证据、原扣款与佣金。存在可用历史行情、已付佣金或证据异常将拒绝。"
          onConfirm={async (reason) => {
            if (flight.current) return;
            if (authStore.getSession('admin')?.generation !== session.generation) throw new Error('管理员会话已失效');
            if (!reason.trim() || [...reason.trim()].length > 512) throw new Error('请填写不超过 512 字的退款原因');
            const storage = window.sessionStorage;
            storage.setItem('seconds-refund-probe', '1'); storage.removeItem('seconds-refund-probe');
            const store = new FinancialCommandIntentStore({ prefix: 'seconds-refund', storage });
            const scope = financialCommandScopeFromSession(session, 'seconds-refund', id, asset);
            const values = { order_id: id, reason: reason.trim() };
            if ((status === 'refunded' || context.receipt) && !store.hasPending(scope, values)) throw new Error('仅可核对原未决退款请求');
            flight.current = true;
            try {
              await runRecoverableFinancialCommand({ store, scope, values, request: async (key) => {
                const receipt = await apiRequest<ApiRecord>(endpoint, { method: 'POST',
                  body: JSON.stringify({ reason: values.reason, idempotency_key: key }) });
                if (!receipt || String(receipt.order_id) !== id || String(receipt.asset_id) !== asset ||
                    `admin:${receipt.admin_id}` !== session.subject || receipt.reason !== values.reason ||
                    receipt.idempotency_key !== key || receipt.policy_version !== context.policy_version ||
                    typeof receipt.amount !== 'string' || compareDecimalText(receipt.amount, amount) !== 0 ||
                    !uint(receipt.created_at)) throw new Error('退款结果尚未确认，请保留原原因重试核对');
              } });
              Toast.success('本金退款已确认'); setVisible(false); helpers.reload();
            } finally { flight.current = false; }
          }} />
      </> : null}
    </SideSheet>
  </>;
}

export function SecondsPrincipalRefundAction(props: { record: ApiRecord; helpers: RowActionHelpers }) {
  const session = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('admin'));
  if (!session || !['manual_review', 'refunded'].includes(recordString(props.record, 'status'))) return null;
  const id = recordString(props.record, 'id');
  return <AdminRequestActionBoundary endpoint={`/admin/api/v1/seconds-contracts/orders/${id}/principal-refund`} method="POST">
    <RefundEditor key={`${session.generation}:${id}`} {...props} session={session} />
  </AdminRequestActionBoundary>;
}
