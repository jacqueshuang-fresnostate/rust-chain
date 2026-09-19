import { Button, SideSheet, Toast } from '@douyinfe/semi-ui';
import { useEffect, useRef, useState, useSyncExternalStore } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { authStore, type AuthSession } from '../../../auth/authStore';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminTextInput } from '../../../shared/SemiFormControls';
import { adminErrorMessage } from '../../../shared/adminErrorMessage';
import { addDecimalText, canonicalDecimalText, compareDecimalText, decimalFitsStorage, isPositiveDecimalText, multiplyDecimalText } from '../../../shared/decimal';
import { FinancialCommandIntentStore, financialCommandScopeFromSession, runRecoverableFinancialCommand } from '../../../shared/idempotency';
import { AdminRequestActionBoundary } from '../../access';
import { createModalProps, recordString, type RowActionHelpers } from './shared';

const endpoint = '/admin/api/v1/spot/fills';
const orderEndpoint = '/admin/api/v1/spot/orders';
type FillIntent = { buy_order_id: string; sell_order_id: string; price: string; quantity: string };
type PreviewOrder = {
  id: string; user_id: string; pair_id: string; side: 'buy' | 'sell';
  price: string | null; quantity: string; filled_quantity: string; status: string;
};
type Preview = { intent: FillIntent; buy: PreviewOrder; sell: PreviewOrder };
const statusLabels: Record<string, string> = {
  pending: '待处理', open: '挂单中', partially_filled: '部分成交', filled: '已成交',
  cancelled: '已撤销', rejected: '已拒绝'
};

function parseOrder(value: unknown, id: string, side: 'buy' | 'sell'): PreviewOrder {
  if (!value || typeof value !== 'object') throw new Error('订单详情响应无效');
  const row = value as PreviewOrder;
  if (row.id !== id || row.side !== side || typeof row.user_id !== 'string' ||
      typeof row.pair_id !== 'string' || !row.pair_id || typeof row.status !== 'string' ||
      typeof row.quantity !== 'string' || !isPositiveDecimalText(row.quantity) ||
      typeof row.filled_quantity !== 'string' || compareDecimalText(row.filled_quantity, '0') === -1 ||
      canonicalDecimalText(row.filled_quantity) === null ||
      (row.price !== null && (typeof row.price !== 'string' || !isPositiveDecimalText(row.price)))) {
    throw new Error('订单详情字段无效，请重新核对');
  }
  return row;
}

function canFill(order: PreviewOrder, intent: FillIntent): boolean {
  const remaining = addDecimalText(order.quantity, `-${canonicalDecimalText(order.filled_quantity)}`);
  const limit = order.price === null ? null : compareDecimalText(intent.price, order.price);
  return ['pending', 'open', 'partially_filled'].includes(order.status) && remaining !== null &&
    compareDecimalText(intent.quantity, remaining) !== 1 &&
    (limit === null || (order.side === 'buy' ? limit <= 0 : limit >= 0));
}

function OrderPreview({ order }: { order: PreviewOrder }) {
  return (
    <section aria-label={order.side === 'buy' ? '买方订单预览' : '卖方订单预览'} style={{ overflowWrap: 'anywhere' }}>
      <h4>{order.side === 'buy' ? '买方订单' : '卖方订单'} {order.id}</h4>
      <dl style={{ display: 'grid', gridTemplateColumns: '88px minmax(0, 1fr)', gap: '10px 12px' }}>
        <dt>用户 ID</dt><dd style={{ margin: 0 }}>{order.user_id}</dd>
        <dt>交易对</dt><dd style={{ margin: 0 }}>{order.pair_id}</dd>
        <dt>状态</dt><dd style={{ margin: 0 }}>{statusLabels[order.status] ?? order.status}</dd>
        <dt>限价</dt><dd style={{ margin: 0 }}>{order.price ?? '-'}</dd>
        <dt>委托数量</dt><dd style={{ margin: 0 }}>{order.quantity}</dd>
        <dt>已成交数量</dt><dd style={{ margin: 0 }}>{order.filled_quantity}</dd>
      </dl>
    </section>
  );
}

function SpotFillEditor({ helpers, record, session }: { helpers: RowActionHelpers; record: ApiRecord; session: AuthSession }) {
  const [visible, setVisible] = useState(false);
  const [counterOrder, setCounterOrder] = useState('');
  const [price, setPrice] = useState('');
  const [quantity, setQuantity] = useState('');
  const [preview, setPreview] = useState<Preview | null>(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const requestOwner = useRef<AbortController | null>(null);
  const inFlight = useRef(false);
  const orderId = recordString(record, 'id');
  const side = recordString(record, 'side');
  const draftValid = /^[1-9]\d*$/.test(orderId) && /^[1-9]\d*$/.test(counterOrder.trim()) &&
    counterOrder.trim() !== orderId && decimalFitsStorage(price) && decimalFitsStorage(quantity) &&
    compareDecimalText(multiplyDecimalText(price, quantity) ?? '', '1e20') === -1 &&
    isPositiveDecimalText(price) && isPositiveDecimalText(quantity);

  useEffect(() => () => requestOwner.current?.abort(), []);

  function invalidate() {
    requestOwner.current?.abort();
    requestOwner.current = null;
    setLoading(false);
    setPreview(null);
    setError('');
  }

  async function loadPreview() {
    if (!draftValid || inFlight.current) return;
    invalidate();
    const controller = new AbortController();
    requestOwner.current = controller;
    setLoading(true);
    const intent: FillIntent = {
      buy_order_id: side === 'buy' ? orderId : counterOrder.trim(),
      sell_order_id: side === 'sell' ? orderId : counterOrder.trim(),
      price: canonicalDecimalText(price)!,
      quantity: canonicalDecimalText(quantity)!
    };
    try {
      const [buyValue, sellValue] = await Promise.all([
        apiRequest<unknown>(`${orderEndpoint}/${intent.buy_order_id}`, { signal: controller.signal }),
        apiRequest<unknown>(`${orderEndpoint}/${intent.sell_order_id}`, { signal: controller.signal })
      ]);
      if (requestOwner.current !== controller || controller.signal.aborted) return;
      const buy = parseOrder(buyValue, intent.buy_order_id, 'buy');
      const sell = parseOrder(sellValue, intent.sell_order_id, 'sell');
      if (buy.pair_id !== sell.pair_id) throw new Error('双方订单必须属于同一交易对');
      setPreview({ intent, buy, sell });
    } catch (cause) {
      if (requestOwner.current === controller && !controller.signal.aborted) {
        setError(adminErrorMessage(cause, '订单预览失败'));
      }
    } finally {
      if (requestOwner.current === controller) setLoading(false);
    }
  }

  async function confirm(reason: string) {
    if (inFlight.current) return;
    if (!preview || !reason.trim() || [...reason.trim()].length > 512) throw new Error('请核对订单并填写不超过 512 字的原因');
    if (authStore.getSession('admin')?.generation !== session.generation) throw new Error('管理员会话已失效，请重新登录');
    // Persistence is required before issuing a financial command; unavailable storage fails closed.
    const storage = window.sessionStorage;
    const storageProbe = 'spot-fill-storage-probe';
    storage.setItem(storageProbe, '1');
    storage.removeItem(storageProbe);
    const store = new FinancialCommandIntentStore({ prefix: 'admin-spot-fill', storage });
    const values = { ...preview.intent, reason: reason.trim() };
    const scope = financialCommandScopeFromSession(session, 'admin-spot-fill', preview.buy.user_id, preview.buy.pair_id);
    if (!store.hasPending(scope, values) && (!canFill(preview.buy, preview.intent) || !canFill(preview.sell, preview.intent))) {
      throw new Error('订单状态、剩余数量或限价不允许新成交；未决原请求可按原参数核对');
    }
    inFlight.current = true;
    setSubmitting(true);
    try {
      await runRecoverableFinancialCommand({
        store, scope, values,
        request: async (idempotencyKey) => {
          const response = await apiRequest<{ trade?: ApiRecord }>(endpoint, {
            method: 'POST',
            body: JSON.stringify({ ...values, idempotency_key: idempotencyKey })
          });
          const trade = response?.trade;
          if (!trade || typeof trade.id !== 'string' || !trade.id ||
              trade.buy_order_id !== values.buy_order_id || trade.sell_order_id !== values.sell_order_id ||
              typeof trade.price !== 'string' || compareDecimalText(trade.price, values.price) !== 0 ||
              typeof trade.quantity !== 'string' || compareDecimalText(trade.quantity, values.quantity) !== 0) {
            throw new Error('成交结果尚未确认，请保留原参数重试核对');
          }
        }
      });
      Toast.success('手工成交已确认');
      setVisible(false);
      invalidate();
      setCounterOrder('');
      setPrice('');
      setQuantity('');
      helpers.reload();
    } finally {
      inFlight.current = false;
      setSubmitting(false);
    }
  }

  return (
    <>
      <Button disabled={!orderId || !['buy', 'sell'].includes(side)} onClick={() => setVisible(true)} size="small" theme="borderless">手工成交</Button>
      <SideSheet {...createModalProps('medium')} title="现货手工成交" visible={visible}
        onCancel={() => { if (!inFlight.current) { invalidate(); setVisible(false); } }}>
        {visible ? (
          <div>
            <div className="admin-action-form">
              <label>当前订单 ID<AdminTextInput ariaLabel="当前订单 ID" readOnly value={orderId} onChange={() => undefined} /></label>
              <label>对手订单 ID<AdminTextInput ariaLabel="对手订单 ID" disabled={submitting} value={counterOrder} onChange={(value) => { invalidate(); setCounterOrder(value); }} /></label>
              <label>成交价格<AdminTextInput ariaLabel="成交价格" disabled={submitting} value={price} onChange={(value) => { invalidate(); setPrice(value); }} /></label>
              <label>成交数量<AdminTextInput ariaLabel="成交数量" disabled={submitting} value={quantity} onChange={(value) => { invalidate(); setQuantity(value); }} /></label>
            </div>
            <Button disabled={!draftValid || submitting || loading} loading={loading} onClick={() => void loadPreview()}>核对双方订单</Button>
            {error ? <p role="alert">{error}</p> : null}
            {preview ? (
              <>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 260px), 1fr))', gap: 16 }}>
                  <OrderPreview order={preview.buy} />
                  <OrderPreview order={preview.sell} />
                </div>
                <p>成交价格：{preview.intent.price} · 数量：{preview.intent.quantity} · 报价金额：{multiplyDecimalText(preview.intent.price, preview.intent.quantity)}</p>
                <ConfirmAction
                  actionText="确认手工成交" title="确认双边资金结算" disabled={submitting}
                  description={`买单 ${preview.buy.id} / 卖单 ${preview.sell.id}；价格 ${preview.intent.price}，数量 ${preview.intent.quantity}。提交将结算双方冻结资金，订单状态以服务端为准。`}
                  onConfirm={confirm}
                />
              </>
            ) : null}
          </div>
        ) : null}
      </SideSheet>
    </>
  );
}

export function SpotFillAction(props: { helpers: RowActionHelpers; record: ApiRecord }) {
  const session = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('admin'));
  if (!session) return null;
  return (
    <AdminRequestActionBoundary endpoint={endpoint} method="POST">
      <AdminRequestActionBoundary endpoint={orderEndpoint} method="GET">
        <SpotFillEditor key={`${session.generation}:${recordString(props.record, 'id')}`} {...props} session={session} />
      </AdminRequestActionBoundary>
    </AdminRequestActionBoundary>
  );
}
