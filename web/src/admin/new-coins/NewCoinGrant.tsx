import { Space, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { apiRequest } from '../../api/client';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminTextInput } from '../../shared/SemiFormControls';
import { isPositiveDecimalText } from '../../shared/decimal';
import { AdminReferenceSelect, isReferenceSelectable, useAdminReferenceOptions } from '../referenceOptions';
import { useCanAdminRequest } from '../access';
import type { NewCoinProject } from './projectModel';

/** 独立赠币不关联申购、不扣申购冻结资金；仅沿用已有派发权限和供给规则。 */
export function NewCoinGrant({ project, onSaved }: { project: NewCoinProject; onSaved: () => Promise<unknown> }) {
  const canReadUsers = useCanAdminRequest('/admin/api/v1/users', 'GET');
  const users = useAdminReferenceOptions('user', canReadUsers);
  const [userId,setUserId]=useState('');
  const [quantity,setQuantity]=useState('');
  const [key,setKey]=useState('');
  const [error,setError]=useState('');
  const valid = project.status==='active' && project.lifecycle_status==='distribution'
    && isReferenceSelectable(users.options,userId) && isPositiveDecimalText(quantity);
  return <Space vertical align="start" spacing={16}>
    <Typography.Title heading={4}>额外赠币</Typography.Title>
    <Typography.Text>这不是申购派发，不会结算申购单或退差额，会单独消耗项目剩余额度。仅在派发阶段可执行。</Typography.Text>
    <AdminReferenceSelect placeholder="选择接收用户" label="赠币接收用户" value={userId} options={users.options} loading={users.loading} error={users.error}
      onChange={v=>{setUserId(v);setKey(crypto.randomUUID());}} />
    <label>赠币数量<AdminTextInput ariaLabel="赠币数量" value={quantity} onChange={v=>{setQuantity(v);setKey(crypto.randomUUID());}} /></label>
    {error ? <Typography.Text type="danger">{error}</Typography.Text> : null}
    <ConfirmAction key="grant-action" actionText="确认额外赠币" disabled={!valid} title={`确认向用户 ${userId} 额外赠币 ${quantity}（不结算申购）`} onConfirm={async reason=>{
      try { await apiRequest(`/admin/api/v1/new-coins/${project.id}/distribute`,{method:'POST',body:JSON.stringify({user_id:Number(userId),quantity:quantity.trim(),idempotency_key:key,reason})}); setQuantity('');setError('');await onSaved(); }
      catch(cause){setError(cause instanceof Error ? cause.message : '赠币失败');throw cause;}
    }} />
  </Space>;
}
