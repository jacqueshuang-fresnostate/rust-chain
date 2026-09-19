import { IconDelete, IconPlus } from '@douyinfe/semi-icons';
import { Button, Modal, Space, Tooltip, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';

import { apiRequest } from '../../api/client';
import { AdminSelect, AdminSwitch, AdminTextInput } from '../../shared/SemiFormControls';
import { AdminRequestActionBoundary } from '../access';
import { AssetSelect, useAssetOptions } from '../resources/actions/shared';
import { AdminSettingsPage, SettingsSaveConfirmation, useAdminSettingsEditor, buildSettingsDifferences, type SettingsFieldDefinition } from '../settings';
import { emptyWithdrawalPolicy, parseWithdrawalPolicy, validWithdrawalPolicy, type WithdrawalPolicy, type WithdrawalPolicyResponse } from './withdrawalPolicy';
import { parseSafeInteger } from '../../shared/integer';

const { Title, Text } = Typography;

const policyFields: SettingsFieldDefinition<WithdrawalPolicy>[] = [
  { key: 'enabled', field: '启用策略', read: (p) => p.enabled, format: (v) => v ? '启用' : '停用' },
  { key: 'amount_basis', field: '金额口径', read: (p) => p.amount_basis, format: (v) => v === 'principal' ? '提现本金' : '本金与手续费' },
  { key: 'address_cooling_seconds', field: '新地址冷静期', read: (p) => p.address_cooling_seconds, format: (v) => v === null ? '停用' : `${v} 秒` },
  { key: 'security_cooling_seconds', field: '安全变更冷静期', read: (p) => p.security_cooling_seconds, format: (v) => v === null ? '停用' : `${v} 秒` },
  { key: 'allowances', field: '累计额度', read: (p) => p.allowances, format: (v) => (v as WithdrawalPolicy['allowances']).map((r) =>
    `用户 ${r.user_id ?? '每位'}；KYC ${r.kyc_level ?? '全部'}；滚动 ${r.window_seconds} 秒；上限 ${r.max_amount}；${r.pending_mode === 'all_outstanding' ? '全部未终结单持续占额' : '仅窗口内单占额'}`).join(' / ') || '未配置' },
  { key: 'review_tiers', field: '复核阶梯', read: (p) => p.review_tiers, format: (v) => (v as WithdrawalPolicy['review_tiers']).map((r) =>
    `金额 ≥ ${r.min_amount}：${r.required_approvals} 人`).join(' / ') || '一次人工审核' }
];

function OptionalSeconds({ label, value, onChange }: { label: string; value: number | null; onChange: (value: number | null) => void }) {
  return <label>{label}<PolicyIntegerInput label={label} min={1} max={4294967295} value={value} onChange={onChange} /></label>;
}

function PolicyIntegerInput({ label, value, onChange, min, max = Number.MAX_SAFE_INTEGER }: {
  label: string; value: number | null; onChange: (value: number | null) => void; min: number; max?: number;
}) {
  const [text, setText] = useState(value === null ? '' : String(value));
  useEffect(() => {
    if (!Number.isNaN(value)) setText(value === null ? '' : String(value));
  }, [value]);
  return <AdminTextInput ariaLabel={label} value={text} onChange={(next) => {
    setText(next);
    // Invalid nonblank input must not become null (which removes a policy restriction).
    onChange(next.trim() === '' ? null : parseSafeInteger(next, min, max) ?? Number.NaN);
  }} />;
}

function PolicyEditor({ assetId, onDirtyChange }: { assetId: number; onDirtyChange: (dirty: boolean) => void }) {
  const endpoint = `/admin/api/v1/wallet/withdrawal-policies/${assetId}`;
  const editor = useAdminSettingsEditor<WithdrawalPolicyResponse, WithdrawalPolicyResponse>({
    settingKey: endpoint,
    initialForm: { asset_id: assetId, revision: 0, policy: emptyWithdrawalPolicy },
    load: async () => parseWithdrawalPolicy(await apiRequest<unknown>(endpoint), assetId),
    selectForm: (data) => data,
    save: async (draft, reason) => {
      if (!validWithdrawalPolicy(draft.policy)) throw new Error('提现策略数值无效，请检查输入');
      return parseWithdrawalPolicy(await apiRequest<unknown>(endpoint, {
      method: 'PATCH', body: JSON.stringify({ expected_revision: draft.revision, policy: draft.policy, reason })
      }), assetId);
    },
    successMessage: '提现策略已保存'
  });
  const policy = editor.draft.policy;
  useEffect(() => { onDirtyChange(editor.isDirty); }, [editor.isDirty, onDirtyChange]);
  const update = (patch: Partial<WithdrawalPolicy>) => editor.setDraft((current) => ({
    ...current, policy: { ...current.policy, ...patch }
  }));
  const differences = buildSettingsDifferences(editor.baseline?.policy ?? policy, policy, policyFields);
  return <AdminSettingsPage title="提现策略" feedback={editor.feedback} isDirty={editor.isDirty}
    isInitialLoading={editor.isInitialLoading} isReady={editor.isReady} isRefreshing={editor.isFetching}
    loadError={editor.loadError} onReload={editor.reloadLatest}>
    <fieldset disabled={editor.isSaving} style={{ border: 0, padding: 0, minWidth: 0 }}>
      <div className="admin-action-form">
        <AdminSwitch label="启用策略" checked={policy.enabled} onChange={(enabled) => update({ enabled })} />
        <label>额度与复核金额口径<AdminSelect ariaLabel="金额口径" value={policy.amount_basis}
          onChange={(value) => update({ amount_basis: value as WithdrawalPolicy['amount_basis'] })}
          optionList={[{ value: 'principal', label: '提现本金' }, { value: 'total_reserved', label: '本金与手续费' }]} /></label>
        <OptionalSeconds label="新地址冷静期（秒，空为停用）" value={policy.address_cooling_seconds} onChange={(value) => update({ address_cooling_seconds: value })} />
        <OptionalSeconds label="安全变更冷静期（秒，空为停用）" value={policy.security_cooling_seconds} onChange={(value) => update({ security_cooling_seconds: value })} />
      </div>
      <section style={{ marginTop: 24 }}>
        <Space wrap><Title heading={5}>累计额度</Title><Button icon={<IconPlus aria-hidden="true" />} theme="borderless"
          onClick={() => update({ allowances: [...policy.allowances, {
            user_id: null, kyc_level: null, window: 'rolling', window_seconds: 0,
            pending_mode: 'all_outstanding', max_amount: ''
          }] })}>添加额度规则</Button></Space>
        {policy.allowances.length === 0 ? <p><Text type="tertiary">未配置累计额度</Text></p> : null}
        {policy.allowances.map((rule, index) => {
          const change = (patch: Partial<typeof rule>) => update({ allowances: policy.allowances.map((r, i) => i === index ? { ...r, ...patch } : r) });
          return <fieldset key={index} className="admin-action-form" style={{ marginTop: 12, minWidth: 0 }}>
            <legend>额度规则 {index + 1}</legend>
            <label>用户 ID（空为每位用户）<PolicyIntegerInput label={`规则${index + 1}用户`} min={1} value={rule.user_id} onChange={(v) => change({ user_id: v })} /></label>
            <label>KYC 等级（空为所有等级）<PolicyIntegerInput label={`规则${index + 1}KYC`} min={0} max={2147483647} value={rule.kyc_level} onChange={(v) => change({ kyc_level: v })} /></label>
            <label>滚动窗口（秒）<PolicyIntegerInput label={`规则${index + 1}窗口秒数`} min={1} max={4294967295} value={rule.window_seconds} onChange={(v) => change({ window_seconds: v ?? 0 })} /></label>
            <label>累计上限（资产数量）<AdminTextInput ariaLabel={`规则${index + 1}累计上限`} value={rule.max_amount} onChange={(max_amount) => change({ max_amount })} /></label>
            <label>未终结申请<AdminSelect ariaLabel={`规则${index + 1}待处理口径`} value={rule.pending_mode}
              onChange={(v) => change({ pending_mode: v as typeof rule.pending_mode })}
              optionList={[{ value: 'all_outstanding', label: '全部持续占额' }, { value: 'within_window', label: '仅窗口内创建的申请占额' }]} /></label>
            <Tooltip content="移除额度规则"><Button aria-label={`移除额度规则${index + 1}`} icon={<IconDelete aria-hidden="true" />} theme="borderless" type="danger"
              onClick={() => update({ allowances: policy.allowances.filter((_, i) => i !== index) })} /></Tooltip>
          </fieldset>;
        })}
      </section>
      <section style={{ marginTop: 24 }}>
        <Space wrap><Title heading={5}>独立复核</Title><Button icon={<IconPlus aria-hidden="true" />} theme="borderless"
          onClick={() => update({ review_tiers: [...policy.review_tiers, { min_amount: '', required_approvals: 0 }] })}>添加复核阶梯</Button></Space>
        {policy.review_tiers.length === 0 ? <p><Text type="tertiary">沿用一次人工审核</Text></p> : null}
        {policy.review_tiers.map((tier, index) => {
          const change = (patch: Partial<typeof tier>) => update({ review_tiers: policy.review_tiers.map((r, i) => i === index ? { ...r, ...patch } : r) });
          return <fieldset key={index} className="admin-action-form" style={{ marginTop: 12, minWidth: 0 }}>
            <legend>复核阶梯 {index + 1}</legend>
            <label>含边界金额下限（首档为零）<AdminTextInput ariaLabel={`阶梯${index + 1}金额下限`} value={tier.min_amount} onChange={(min_amount) => change({ min_amount })} /></label>
            <label>独立管理员人数<PolicyIntegerInput label={`阶梯${index + 1}复核人数`} min={1} max={20} value={tier.required_approvals} onChange={(v) => change({ required_approvals: v ?? 0 })} /></label>
            <Tooltip content="移除复核阶梯"><Button aria-label={`移除复核阶梯${index + 1}`} icon={<IconDelete aria-hidden="true" />} theme="borderless" type="danger"
              onClick={() => update({ review_tiers: policy.review_tiers.filter((_, i) => i !== index) })} /></Tooltip>
          </fieldset>;
        })}
      </section>
      <div style={{ marginTop: 24 }}>
        <AdminRequestActionBoundary endpoint={endpoint} method="PATCH">
          <SettingsSaveConfirmation actionText="保存提现策略" title="确认提现策略变更" differences={differences}
            disabled={editor.isSaving} riskLevel="high" onConfirm={editor.saveChanges}
            impactSummary="所有匹配额度规则共同生效。拒绝或确定失败释放额度，结果不明继续占额；已有申请保留原复核及冷静期策略。"
            validationIssues={validWithdrawalPolicy(policy) ? [] : [{ key: 'policy', field: '规则', message: '请填写正整数时窗、精确金额、从零开始的递增阶梯及不递减的复核人数。' }]} />
        </AdminRequestActionBoundary>
      </div>
    </fieldset>
  </AdminSettingsPage>;
}

export function WithdrawalPolicyPage() {
  const [params] = useSearchParams();
  const [assetId, setAssetId] = useState(params.get('asset_id') ?? '');
  const [dirty, setDirty] = useState(false);
  const { assetOptions, assetLoading } = useAssetOptions();
  const selectAsset = (next: string) => {
    if (!dirty || next === assetId) { setAssetId(next); return; }
    Modal.confirm({
      title: '放弃未保存的提现策略？', content: '切换资产将丢弃当前未保存的变更。',
      okText: '放弃并切换', cancelText: '继续编辑',
      onOk: () => { setDirty(false); setAssetId(next); }
    });
  };
  return <>
    <div className="admin-action-form" style={{ padding: '16px 24px' }}>
      <AssetSelect label="策略资产" value={assetId} onChange={selectAsset} options={assetOptions} loading={assetLoading} />
    </div>
    {/^[1-9]\d*$/.test(assetId) && Number.isSafeInteger(Number(assetId))
      ? <PolicyEditor assetId={Number(assetId)} key={assetId} onDirtyChange={setDirty} />
      : <div style={{ padding: '16px 24px' }}><Title heading={4}>提现策略</Title><Text type="tertiary">请选择资产</Text></div>}
  </>;
}
