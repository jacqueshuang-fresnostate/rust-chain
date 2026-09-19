import { IconRefresh } from '@douyinfe/semi-icons';
import { Banner, Button, DatePicker, Input, Modal, Pagination, Space, TextArea, Typography } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';

import { authStore } from '../../auth/authStore';
import { PageHeader } from '../../layouts/PageHeader';
import { adminErrorMessage } from '../../shared/adminErrorMessage';
import { AmountText } from '../../shared/AmountText';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { DetailDrawer } from '../../shared/DetailDrawer';
import { FilterBar, type FilterValues } from '../../shared/FilterBar';
import { ResizableTable } from '../../shared/ResizableTable';
import { TimestampText } from '../../shared/TimestampText';
import { hasAdminPermission, useAdminAccess } from '../access';
import {
  type FinancialRetry, leaseStatuses, loadFinancialRetries, requeueFinancialRetry,
  retryKinds, retryOutcomes, sourceTypes, updateFinancialRetryIncident, loadFinancialRetrySecondsOrder
} from './financialRetriesApi';

const fields = [
  { key: 'task_kind', label: '任务类型', type: 'select' as const, options: Object.entries(retryKinds).map(([value, label]) => ({ value, label })) },
  { key: 'outcome', label: '最近结果', type: 'select' as const, options: Object.entries(retryOutcomes).map(([value, label]) => ({ value, label })) }
];

export function FinancialRetriesPage() {
  const [filters, setFilters] = useState<FilterValues>({});
  const [page, setPage] = useState(1);
  const [submitting, setSubmitting] = useState(false);
  const [notice, setNotice] = useState('');
  const access = useAdminAccess();
  const session = authStore.getSession('admin');
  const query = useQuery({
    queryKey: ['admin-financial-retries', session?.subject, session?.generation, filters, page],
    queryFn: ({ signal }) => loadFinancialRetries(filters, page, signal),
    retry: false,
    throwOnError: false,
    refetchOnWindowFocus: false
  });
  const canOperate = hasAdminPermission(access, 'governance.financial.operate');
  const canAudit = hasAdminPermission(access, 'audit.logs.read');
  const canReadSeconds = hasAdminPermission(access, 'seconds.orders.read');
  const busy = query.isFetching || submitting;

  async function requeue(row: FinancialRetry, reason: string) {
    setSubmitting(true);
    setNotice('');
    try {
      await requeueFinancialRetry(row, reason);
      setNotice('重新排期已记录，等待后台任务扫描。');
      await query.refetch({ throwOnError: false });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="资金异常工作台" actions={
        <Button icon={<IconRefresh aria-hidden="true" />} disabled={busy} onClick={() => void query.refetch({ throwOnError: false })}>刷新</Button>
      } />
      {notice ? <Banner type="success" closeIcon={null} description={notice} /> : null}
      {query.error ? <Banner type="danger" closeIcon={null} description={adminErrorMessage(query.error, '资金异常加载失败')} /> : null}
      <FilterBar fields={fields} loading={busy} value={filters} onChange={(value) => {
        setFilters(value); setPage(1); setNotice('');
      }} />
      {query.data ? (
        <>
          <Space wrap style={{ marginBlock: 16 }}>
            <Typography.Text strong>筛选结果 {query.data.total} 条</Typography.Text>
            {query.data.counts.map((count) => <Typography.Text key={count.outcome}>{retryOutcomes[count.outcome]} {count.count} 条</Typography.Text>)}
            <Typography.Text type="tertiary">快照时间 <TimestampText value={query.data.checked_at} /></Typography.Text>
          </Space>
          <ResizableTable<FinancialRetry>
            aria-label="资金异常任务"
            dataSource={query.data.retries}
            loading={query.isFetching}
            pagination={false}
            rowKey={(row) => row ? `${row.task_kind}:${row.item_id}` : ''}
            empty="暂无符合条件的资金重试记录"
            columns={[
              { key: 'task_kind', title: '任务类型', width: 160, render: (_, row) => retryKinds[row.task_kind] },
              { dataIndex: 'item_id', title: '任务记录 ID', width: 130 },
              { key: 'source_type', title: '来源类型', width: 140, render: (_, row) => row.source_type ? sourceTypes[row.source_type] ?? '其他来源' : '来源缺失' },
              { dataIndex: 'source_order_id', title: '来源订单标识', width: 200, render: (value, row) =>
                row.task_kind === 'seconds' && canReadSeconds
                  ? <Link to={`/admin/seconds-contract/orders/${row.item_id}`}>{value}</Link> : value ?? '-' },
              { dataIndex: 'user_id', title: '来源用户 ID', width: 130, render: (value) => value ?? '-' },
              { key: 'amount', title: '来源金额（本金 / 佣金 / 押注）', width: 230, render: (_, row) => <AmountText value={row.amount} asset={row.asset ?? undefined} /> },
              { key: 'outcome', title: '最近结果', width: 150, render: (_, row) => retryOutcomes[row.outcome] },
              { dataIndex: 'attempt_count', title: '尝试次数', width: 110, render: (value) => value ?? '-' },
              { key: 'last_attempt_at', title: '上次尝试', width: 220, render: (_, row) => <TimestampText value={row.last_attempt_at} /> },
              { key: 'next_attempt_at', title: '下次尝试 / 租约期限', width: 220, render: (_, row) => <TimestampText value={row.next_attempt_at} /> },
              { key: 'lease_status', title: '租约状态', width: 170, render: (_, row) => row.task_kind === 'seconds' ? '非后台重试任务' : leaseStatuses[row.lease_status] },
              { key: 'owner', title: '负责人', width: 180, render: (_, row) => row.incident.owner_admin_id === null ? '未分配' : `${row.incident.owner_name} (#${row.incident.owner_admin_id})` },
              { key: 'due', title: '处理期限', width: 220, render: (_, row) => row.incident.due_at === null ? '未设置' : <TimestampText value={row.incident.due_at} /> },
              { key: 'sla', title: '期限状态', width: 130, render: (_, row) => row.incident.due_at === null ? '未设置期限' : row.incident.due_at <= query.data!.checked_at ? '已逾期' : '期限内' },
              { key: 'failure', title: '复核错误码', width: 220, render: (_, row) => row.failure_code ?? '-' },
              { key: 'failure_at', title: '转复核时间', width: 220, render: (_, row) => <TimestampText value={row.failure_at} /> },
              { key: 'window', title: '价格证据窗口', width: 250, render: (_, row) => row.review_window_start === null ? '-' : <>
                <TimestampText value={row.review_window_start} /><br /><TimestampText value={row.review_window_end} />
              </> },
              ...(canOperate || canAudit ? [{
                key: 'actions', title: '操作', width: 340, fixed: 'right' as const,
                render: (_: unknown, row: FinancialRetry) => <Space>
                  {canOperate && row.task_kind !== 'seconds' ? <ConfirmAction
                    key={`requeue:${row.version}`}
                    actionText="重新排期"
                    actionAriaLabel={`重新排期 ${retryKinds[row.task_kind]} ${row.item_id}`}
                    title={`重新排期 ${retryKinds[row.task_kind]} #${row.item_id}`}
                    description="仅调整下次扫描时间，不会直接付款、退款或修改订单状态。资金处理仍由原业务任务校验。"
                    disabled={busy || Boolean(query.error) || row.lease_status === 'active'}
                    onConfirm={(reason) => requeue(row, reason)}
                  /> : null}
                  {canOperate ? <IncidentAction key={`incident:${row.task_kind}:${row.item_id}`}
                    row={row} disabled={busy || Boolean(query.error)}
                    onSaved={async () => {
                      setNotice('负责人和处理期限已记录，资金与订单状态未变更。');
                      await query.refetch({ throwOnError: false });
                    }} /> : null}
                  {canAudit ? <Link key="audit" to={`/admin/audit-logs?target_type=financial_retry&target_id=${row.item_id}`}>审计记录</Link> : null}
                </Space>
              }] : [])
            ]}
          />
          <Pagination currentPage={page} pageSize={20} total={Math.min(query.data.total, 100_020)}
            disabled={busy} onPageChange={setPage} showTotal style={{ marginTop: 16 }} />
        </>
      ) : <section className="admin-table-state" aria-live="polite">
        {query.isPending ? '正在读取资金异常…' : '无法读取资金异常'}
      </section>}
    </main>
  );
}

function IncidentAction({ row, disabled, onSaved }: {
  row: FinancialRetry; disabled: boolean; onSaved: () => Promise<void>;
}) {
  const [snapshot, setSnapshot] = useState<FinancialRetry | null>(null);
  const [owner, setOwner] = useState('');
  const [due, setDue] = useState<number | null>(null);
  const [reason, setReason] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const ownerValid = owner === '' || (/^[1-9]\d*$/.test(owner) && Number.isSafeInteger(Number(owner)));
  const stale = snapshot !== null && snapshot.incident.version !== row.incident.version;
  const valid = !disabled && !stale && ownerValid && reason.trim().length > 0 && [...reason.trim()].length <= 500;

  async function save() {
    if (!snapshot || !valid || submitting) return;
    setSubmitting(true); setError('');
    try {
      await updateFinancialRetryIncident(snapshot, owner === '' ? null : Number(owner), due, reason.trim());
      setSnapshot(null);
      await onSaved();
    } catch (cause) {
      setError(adminErrorMessage(cause, '跟进信息保存失败'));
    } finally { setSubmitting(false); }
  }

  return <>
    <Button theme="borderless" disabled={disabled || submitting}
      aria-label={`跟进信息 ${retryKinds[row.task_kind]} ${row.item_id}`}
      onClick={() => {
        setSnapshot(row); setOwner(row.incident.owner_admin_id === null ? '' : String(row.incident.owner_admin_id));
        setDue(row.incident.due_at); setReason(''); setError('');
      }}>跟进信息</Button>
    <Modal title={`跟进信息 ${retryKinds[row.task_kind]} #${row.item_id}`} visible={snapshot !== null}
      width="min(520px, calc(100vw - 32px))" motion={false} maskClosable={false} closeOnEsc={!submitting}
      confirmLoading={submitting} okText="保存" onOk={save}
      okButtonProps={{ disabled: !valid || submitting, 'aria-label': '保存跟进信息' }}
      cancelButtonProps={{ disabled: submitting, 'aria-label': '取消' }}
      onCancel={() => { if (!submitting) setSnapshot(null); }}>
      {error || stale ? <div role="alert"><Typography.Text type="danger">{error || '跟进信息已变化，请关闭后重新打开。'}</Typography.Text></div> : null}
      <Space vertical align="start" style={{ width: '100%' }}>
        <label style={{ width: '100%' }}>负责人（管理员 ID）
          <Input aria-label="负责人（管理员 ID）" value={owner} onChange={setOwner} disabled={submitting} showClear placeholder="未分配" />
        </label>
        {!ownerValid ? <Typography.Text type="danger">负责人 ID 必须为正整数</Typography.Text> : null}
        <label style={{ width: '100%' }}>处理期限（本地时间）
          <DatePicker aria-label="处理期限" type="dateTime" value={due === null ? undefined : new Date(due)}
            placeholder="未设置" showClear disabled={submitting} style={{ width: '100%' }}
            onChange={(value) => setDue(value instanceof Date ? value.getTime() : null)} onClear={() => setDue(null)} />
        </label>
        <label style={{ width: '100%' }}>操作原因
          <TextArea aria-label="操作原因" value={reason} onChange={setReason} disabled={submitting} maxCount={500} />
        </label>
      </Space>
    </Modal>
  </>;
}

/** 精确 ID 深链复用既有只读详情接口；此页没有判输赢、结算或退款动作。 */
export function FinancialRetrySecondsOrderPage() {
  const { orderId = '' } = useParams();
  const navigate = useNavigate();
  const session = authStore.getSession('admin');
  const query = useQuery({
    queryKey: ['admin-financial-retry-seconds-order', session?.subject, session?.generation, orderId],
    queryFn: ({ signal }) => loadFinancialRetrySecondsOrder(orderId, signal),
    retry: false, throwOnError: false, refetchOnWindowFocus: false
  });
  return <main className="exchange-page admin-action-page">
    <PageHeader title={`秒合约订单 #${orderId}`} actions={<Link to="/admin/seconds-contract/orders">订单列表</Link>} />
    {query.error ? <Banner type="danger" closeIcon={null} description={adminErrorMessage(query.error, '订单读取失败')} /> : null}
    {query.isPending ? <p>正在读取订单…</p> : null}
    <DetailDrawer detail={query.data ? { title: `秒合约订单 #${orderId}`, data: query.data } : null}
      onClose={() => navigate('/admin/seconds-contract/orders')} />
  </main>;
}
