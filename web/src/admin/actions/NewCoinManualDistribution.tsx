import { Button, Modal, Space, TextArea, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { apiRequest } from '../../api/client';
import type { DetailDrawerData } from '../../shared/DetailDrawer';
import type { ApiRecord } from '../../api/types';
import { AdminTextInput } from '../../shared/SemiFormControls';
import { addDecimalText, compareDecimalText, isNonNegativeDecimalText, multiplyDecimalText } from '../../shared/decimal';
import { useCanAdminRequest } from '../access';
import { loadProjectCenter, projectQueryKey } from '../new-coins/projectModel';
import { ADMIN_OPTION_QUERY_KEY } from '../sharedOptionQuery';

/** 订单行即最终结算对象；不允许手填用户或关联另一张订单。 */
export function NewCoinManualDistribution({ order, onSettled }: { order: ApiRecord; onSettled: () => void }) {
  const [open, setOpen] = useState(false);
  const [quantity, setQuantity] = useState('');
  const [reason, setReason] = useState('');
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const queryClient = useQueryClient();
  const projectId = String(order.project_id);
  const endpoint = `/admin/api/v1/new-coins/${projectId}/distribute`;
  const canRead = useCanAdminRequest(`/admin/api/v1/new-coins/${projectId}`, 'GET');
  const canWrite = useCanAdminRequest(endpoint, 'POST');
  const project = useQuery({ queryKey: projectQueryKey(projectId), queryFn: ({ signal }) => loadProjectCenter(projectId, signal), enabled: open && canRead, retry: false, refetchOnWindowFocus: false });
  const price = typeof order.issue_price === 'string' ? order.issue_price : '';
  const requested = typeof order.requested_quantity === 'string' ? order.requested_quantity : '';
  const frozen = typeof order.frozen_quote_amount === 'string' ? order.frozen_quote_amount : '';
  const payment = multiplyDecimalText(price, quantity.trim());
  const refund = payment === null ? null : addDecimalText(frozen, `-${payment}`);
  const comparison = compareDecimalText(quantity.trim(), requested);
  const valid = isNonNegativeDecimalText(quantity.trim()) && (comparison === -1 || comparison === 0)
    && payment !== null && refund !== null && isNonNegativeDecimalText(refund)
    && !project.isFetching && !project.error && project.data?.project.lifecycle_status === 'distribution' && project.data.project.status === 'active';
  if (!canRead || !canWrite || order.status !== 'pending' || order.settlement_mode !== 'manual_distribution') return null;
  const close = () => { if (!busy) { setOpen(false); setReason(''); setError(''); } };
  return <>
    <Button size="small" aria-label={`派发申购 ${order.id}`} onClick={() => { setOpen(true); void project.refetch(); }}>派发 / 退款</Button>
    <Modal title={`最终结算申购 ${order.id}`} visible={open} motion={false} maskClosable={false} closeOnEsc={!busy}
      onCancel={close} cancelButtonProps={{ disabled: busy, 'aria-label': '取消' }} okText="确认派发并退差额"
      confirmLoading={busy} okButtonProps={{ disabled: !valid || !reason.trim() || busy, 'aria-label': '确认派发并退差额' }}
      onOk={async () => {
        if (!valid || busy) return;
        setBusy(true); setError('');
        try {
          await apiRequest(endpoint, { method: 'POST', body: JSON.stringify({ user_id: order.user_id, subscription_id: order.id, quantity: quantity.trim(), idempotency_key: key, reason: reason.trim() }) });
          setOpen(false); setQuantity(''); setReason(''); setKey('');
          void queryClient.invalidateQueries({ queryKey: projectQueryKey(projectId) });
          void queryClient.invalidateQueries({ predicate: q => q.queryKey[0] === ADMIN_OPTION_QUERY_KEY && q.queryKey[2] === 'reference:newCoinProject:100' });
          onSettled();
        } catch (cause) { setError(cause instanceof Error ? cause.message : '结算失败'); }
        finally { setBusy(false); }
      }}>
      <Space vertical align="start" spacing={16} style={{ width: '100%' }}>
        <Typography.Text>项目 {projectId} · 用户 {String(order.user_id)} · 申购数量 {requested}</Typography.Text>
        <Typography.Text>一次确认最终派发量；填 0 全额退款，部分派发后的剩余数量不再重复派发。</Typography.Text>
        {project.isFetching ? <Typography.Text>正在核对项目阶段…</Typography.Text> : null}
        {project.error ? <><Typography.Text type="danger">{project.error.message}</Typography.Text><Button onClick={() => void project.refetch()}>重新核对项目</Button></> : null}
        {project.data && project.data.project.lifecycle_status !== 'distribution' ? <Typography.Text type="warning">请先在项目中心结束申购，进入派发阶段。</Typography.Text> : null}
        <label>最终派发数量<AdminTextInput ariaLabel="最终派发数量" disabled={busy} value={quantity} onChange={v => { setQuantity(v); setKey(crypto.randomUUID()); }} /></label>
        <Typography.Text>冻结：{frozen}；实际扣款：{payment ?? '—'}；退回差额：{refund ?? '—'}（资产 ID：{String(order.quote_asset)}）</Typography.Text>
        <TextArea aria-label="操作原因" placeholder="填写派发或退款原因" disabled={busy} value={reason} onChange={setReason} />
        {error ? <Typography.Text type="danger">{error}。请核对状态后重试；相同数量保留原幂等键。</Typography.Text> : null}
      </Space>
    </Modal>
  </>;
}

export function NewCoinSubscriptionRowActions({ order, onSettled, openDetail }: {
  order: ApiRecord; onSettled: () => void; openDetail: (detail: DetailDrawerData) => void;
}) {
  return <><NewCoinManualDistribution order={order} onSettled={onSettled}/><Button size="small" aria-label={`查看申购 ${order.id}`} onClick={()=>openDetail({ title:`申购 ${order.id}`,data:order })}>查看详情</Button></>;
}
