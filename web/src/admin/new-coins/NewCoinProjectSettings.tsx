import { Button, Card, Space, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';
import { apiRequest, ApiError } from '../../api/client';
import { AdminCheckbox, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { isPositiveDecimalText } from '../../shared/decimal';
import { useCanAdminRequest } from '../access';
import { requiredNewCoinLocalDateTimeMillis } from '../newCoinDateTime';
import { UnsavedChangesGuard } from '../settings/UnsavedChangesGuard';
import { AdminReferenceSelect, useAdminReferenceOptions } from '../referenceOptions';
import { projectLocalTime, type ProjectCenter } from './projectModel';

type Section = 'issuance' | 'unlock-rule' | 'unlock-fee-rule' | 'post-listing-purchase';
const sections = [
  { value: 'issuance', label: '发行参数' }, { value: 'unlock-rule', label: '解禁规则' },
  { value: 'unlock-fee-rule', label: '解禁费用' }, { value: 'post-listing-purchase', label: '上市后购买' }
];
function initial(data: ProjectCenter) {
  const p = data.project;
  return { total: p.total_supply, price: p.issue_price, unlockType: p.unlock_type,
    listed: projectLocalTime(p.listed_at), fixed: projectLocalTime(p.fixed_unlock_at), relative: String(p.relative_unlock_seconds ?? ''),
    feeEnabled: p.unlock_fee_enabled, feeRate: p.unlock_fee_rate ?? '', feeBasis: p.unlock_fee_basis ?? 'market_value',
    purchaseEnabled: p.post_listing_purchase_enabled, pairId: String(p.post_listing_pair_id ?? '') };
}

export function NewCoinProjectSettings({ data, onSaved, onDirty }: { data: ProjectCenter; onSaved: () => Promise<unknown>; onDirty: (dirty: boolean) => void }) {
  const [section, setSection] = useState<Section>('issuance');
  const [form, setForm] = useState(() => initial(data));
  const [version] = useState(data.configuration_version);
  const [baseline, setBaseline] = useState(() => initial(data));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [conflict, setConflict] = useState(false);
  const p = data.project;
  const endpoint = `/admin/api/v1/new-coins/${p.id}/${section}`;
  const canWrite = useCanAdminRequest(endpoint, 'PATCH');
  const canReadPairs = useCanAdminRequest('/admin/api/v1/market/pairs', 'GET');
  const pairs = useAdminReferenceOptions('marketPair', section === 'post-listing-purchase' && canReadPairs && canWrite);
  const dirty = JSON.stringify(form) !== JSON.stringify(baseline);
  useEffect(() => { onDirty(dirty); return () => onDirty(false); }, [dirty, onDirty]);
  const editable = canWrite && p.status === 'active' && (section !== 'issuance' || data.issuance_editable)
    && (section !== 'post-listing-purchase' || p.lifecycle_status === 'listed');
  const set = <K extends keyof typeof form>(key: K, value: typeof form[K]) => setForm(current => ({ ...current, [key]: value }));
  const input = (key: 'total' | 'price' | 'listed' | 'fixed' | 'relative' | 'feeRate', label: string, type?: string) =>
    <label>{label}<AdminTextInput ariaLabel={label} disabled={!editable || busy} value={form[key]} type={type} onChange={v => set(key,v)} /></label>;
  const changed = Object.keys(form).filter(key => form[key as keyof typeof form] !== baseline[key as keyof typeof form]);
  const labels: Record<string,string> = { total:'发行总量',price:'发行价',unlockType:'解禁类型',listed:'计划上市时间',fixed:'固定解禁时间',relative:'相对周期秒数',feeEnabled:'解禁费用开关',feeRate:'费率',feeBasis:'计费依据',purchaseEnabled:'上市后购买开关',pairId:'购买交易对' };
  const previewValue = (value: string | boolean) => typeof value === 'boolean' ? (value ? '开启' : '关闭') : ({fixed_time:'固定时间解禁',relative_period:'相对周期解禁',immediate_on_listing:'上市即解禁',market_value:'市值',profit:'收益'} as Record<string,string>)[value] ?? (value || '空');
  return <Card bordered={false}>
    <UnsavedChangesGuard enabled={dirty || busy} />
    <Space vertical align="start" spacing={16} style={{ width: '100%' }}>
      <Typography.Title heading={4}>项目配置</Typography.Title>
      <label>配置分类<AdminSelect ariaLabel="配置分类" value={section} disabled={dirty || busy} optionList={sections} onChange={v => { setSection(v as Section); setError(''); }} /></label>
      {dirty ? <Typography.Text type="warning">有未保存更改，请先保存或重置后切换配置分类。</Typography.Text> : null}
      {!editable ? <Typography.Text type="secondary">当前阶段或权限下仅可查看。发行参数仅在预热且尚无订单/额度占用时可编辑；上市后购买仅在已上市阶段配置。</Typography.Text> : null}
      <div className="admin-action-form">
        {section === 'issuance' ? <>{input('total','发行总量')}{input('price','发行价')}</> : null}
        {section === 'unlock-rule' ? <>
          <label>解禁类型<AdminSelect ariaLabel="解禁类型" disabled={!editable || busy} value={form.unlockType} onChange={v => set('unlockType',v)} optionList={[
            {value:'immediate_on_listing',label:'上市即解禁'}, {value:'fixed_time',label:'固定时间解禁'}, {value:'relative_period',label:'相对周期解禁'}
          ]} /></label>
          {form.unlockType === 'immediate_on_listing' ? input('listed','计划上市时间','datetime-local') : null}
          {form.unlockType === 'fixed_time' ? input('fixed','固定解禁时间','datetime-local') : null}
          {form.unlockType === 'relative_period' ? input('relative','相对周期秒数') : null}
        </> : null}
        {section === 'unlock-fee-rule' ? <>
          <AdminCheckbox checked={form.feeEnabled} disabled={!editable || busy} onChange={v => set('feeEnabled',v)}>启用解禁费用</AdminCheckbox>
          {form.feeEnabled ? <>{input('feeRate','费率')}<label>计费依据<AdminSelect ariaLabel="计费依据" disabled={!editable || busy} value={form.feeBasis} onChange={v => set('feeBasis',v)} optionList={[{value:'market_value',label:'市值'},{value:'profit',label:'收益'}]} /></label></> : null}
        </> : null}
        {section === 'post-listing-purchase' ? <>
          <AdminCheckbox checked={form.purchaseEnabled} disabled={!editable || busy} onChange={v => set('purchaseEnabled',v)}>启用上市后购买</AdminCheckbox>
          {form.purchaseEnabled ? <AdminReferenceSelect placeholder="选择项目资产对应的交易对" label="购买交易对" value={form.pairId} options={!editable || busy ? pairs.options.map(o=>({...o,disabled:true})) : pairs.options} loading={pairs.loading} error={pairs.error}
            onChange={v=> { if(editable && !busy) set('pairId',v); }} /> : null}
        </> : null}
      </div>
      {section === 'unlock-rule' || section === 'unlock-fee-rule' ? <Typography.Text>仅影响后续形成的锁仓快照，不重写历史仓位。费用使用项目计价资产（ID：{p.quote_asset_id ?? '未配置'}）；计划上市时间仅作计划展示，不触发解禁；实际上市时间由“确认上市”命令记录。已形成的上市门禁不受后续计划或规则修改影响。</Typography.Text> : null}
      {section === 'post-listing-purchase' ? <Typography.Text>启用会同时激活选定交易对；关闭只停止本项目的上市后购买，不自动停用现货交易对。</Typography.Text> : null}
      {error ? <Typography.Text type="danger">{error}</Typography.Text> : null}
      {conflict ? <Button onClick={async () => { setBaseline(form); onDirty(false); await onSaved(); }}>丢弃草稿并加载最新配置</Button> : null}
      <Space key="configuration-actions">
        <Button disabled={busy || !dirty} onClick={() => { setForm(baseline); setError(''); setConflict(false); }}>重置当前配置</Button>
        <ConfirmAction actionText="保存当前配置" disabled={!editable || !dirty || busy || conflict}
          title={`确认更新${sections.find(s=>s.value===section)?.label}：${changed.map(k=>`${labels[k]} ${previewValue(baseline[k as keyof typeof form])} → ${previewValue(form[k as keyof typeof form])}`).join('；')}`}
          onConfirm={async reason => {
            setError(''); setBusy(true);
            try {
              let body: Record<string, unknown>;
              if (section === 'issuance') {
                if (!isPositiveDecimalText(form.total) || !isPositiveDecimalText(form.price)) throw new Error('发行总量和发行价必须为正数');
                body = { total_supply:form.total.trim(), issue_price:form.price.trim(), expected_total_supply:baseline.total, expected_issue_price:baseline.price };
              } else if (section === 'unlock-rule') {
                body = { unlock_type:form.unlockType };
                if(form.unlockType==='immediate_on_listing') body.listed_at=requiredNewCoinLocalDateTimeMillis(form.listed,'计划上市时间');
                if(form.unlockType==='fixed_time') body.fixed_unlock_at=requiredNewCoinLocalDateTimeMillis(form.fixed,'固定解禁时间');
                if(form.unlockType==='relative_period') {
                  const seconds=Number(form.relative); if(!Number.isSafeInteger(seconds)||seconds<=0) throw new Error('相对周期秒数必须为正整数');
                  body.relative_unlock_seconds=seconds;
                }
              } else if (section === 'unlock-fee-rule') {
                body={unlock_fee_enabled:form.feeEnabled};
                if(form.feeEnabled) {
                  if(!p.quote_asset_id || !isPositiveDecimalText(form.feeRate)) throw new Error('请检查计价资产和费用费率');
                  Object.assign(body,{unlock_fee_rate:form.feeRate.trim(),unlock_fee_basis:form.feeBasis,unlock_fee_asset:p.quote_asset_id});
                }
              } else {
                body={enabled:form.purchaseEnabled};
                if(form.purchaseEnabled) { const pair=Number(form.pairId); if(!Number.isSafeInteger(pair)||pair<=0) throw new Error('请选择购买交易对'); body.pair_id=pair; }
              }
              await apiRequest(endpoint,{method:'PATCH',body:JSON.stringify({...body,expected_config:version,reason})});
              setBaseline(form); onDirty(false); await onSaved();
            } catch(cause) { setConflict(cause instanceof ApiError && cause.status===409); setError(cause instanceof ApiError && cause.status===409 ? '配置已变化，请加载最新配置后重试。' : cause instanceof Error ? cause.message : '保存失败'); throw cause; }
            finally { setBusy(false); }
          }} />
      </Space>
    </Space>
  </Card>;
}
