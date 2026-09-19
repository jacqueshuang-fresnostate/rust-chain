import { Toast } from '@douyinfe/semi-ui';
import { useRef, useSyncExternalStore } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { authStore, type AuthSession } from '../../../auth/authStore';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { compareDecimalText } from '../../../shared/decimal';
import { FinancialCommandIntentStore, financialCommandScopeFromSession, runRecoverableFinancialCommand } from '../../../shared/idempotency';
import { AdminRequestActionBoundary } from '../../access';
import { recordString, type RowActionHelpers } from './shared';

function ReversalConfirmation({ helpers, record, session }: { helpers: RowActionHelpers; record: ApiRecord; session: AuthSession }) {
  const inFlight = useRef(false);
  const id = recordString(record, 'id');
  const status = recordString(record, 'status');
  const asset = recordString(record, 'payout_asset_id');
  const amount = recordString(record, 'commission_amount');
  const endpoint = `/admin/api/v1/agent-commissions/${id}/reversal`;

  async function confirm(reason: string) {
    if (inFlight.current) return;
    if (authStore.getSession('admin')?.generation !== session.generation) throw new Error('管理员会话已失效，请重新登录');
    if (!reason.trim() || [...reason.trim()].length > 512) throw new Error('请填写不超过 512 字的冲正原因');
    const storage = window.sessionStorage;
    storage.setItem('commission-reversal-storage-probe', '1');
    storage.removeItem('commission-reversal-storage-probe');
    const store = new FinancialCommandIntentStore({ prefix: 'commission-reversal', storage });
    const scope = financialCommandScopeFromSession(session, 'commission-reversal', id, asset);
    const values = { commission_id: id, reason: reason.trim() };
    if (status !== 'settled' && !store.hasPending(scope, values)) throw new Error('仅可冲正已支付佣金，或核对原未决请求');
    inFlight.current = true;
    try {
      await runRecoverableFinancialCommand({
        scope, store, values,
        request: async (key) => {
          const receipt = await apiRequest<ApiRecord>(endpoint, {
            method: 'POST', body: JSON.stringify({ idempotency_key: key, reason: values.reason })
          });
          if (!receipt || String(receipt.commission_id) !== id ||
              String(receipt.asset_id) !== asset || `admin:${receipt.admin_id}` !== session.subject ||
              receipt.idempotency_key !== key || receipt.reason !== values.reason ||
              typeof receipt.amount !== 'string' || compareDecimalText(receipt.amount, amount) !== 0 ||
              typeof receipt.created_at !== 'number') {
            throw new Error('冲正结果尚未确认，请保留原原因重试核对');
          }
        }
      });
      Toast.success('佣金冲正已确认');
      helpers.reload();
    } finally {
      inFlight.current = false;
    }
  }

  return <ConfirmAction
    actionText={status === 'reversed' ? '核对冲正' : '冲正'}
    dangerous disabled={!/^[1-9]\d*$/.test(id) || !asset || !['settled', 'reversed'].includes(status)}
    title="冲正已支付佣金"
    modalWidth="min(480px, calc(100vw - 32px))"
    description={`佣金 ${id}；原金额 ${amount}，资产 ID ${asset}。全额扣回原收款钱包的可用余额，余额不足将拒绝；原计佣依据保持不变。`}
    onConfirm={confirm}
  />;
}

export function CommissionReversalAction(props: { helpers: RowActionHelpers; record: ApiRecord }) {
  const session = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('admin'));
  if (!session) return null;
  const id = recordString(props.record, 'id');
  return <AdminRequestActionBoundary endpoint={`/admin/api/v1/agent-commissions/${id}/reversal`} method="POST">
    <ReversalConfirmation key={`${session.generation}:${id}`} {...props} session={session} />
  </AdminRequestActionBoundary>;
}
