import { IconRefresh } from '@douyinfe/semi-icons';
import { Banner, Button, DatePicker, Input, Modal, Pagination, SideSheet, Space, TextArea, Typography } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useRef, useState, useSyncExternalStore } from 'react';

import { authStore, type AuthSession } from '../../auth/authStore';
import { adminErrorMessage } from '../../shared/adminErrorMessage';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { ResizableTable } from '../../shared/ResizableTable';
import { TimestampText } from '../../shared/TimestampText';
import { hasAdminPermission, useAdminAccess } from '../access';
import { UnsavedChangesGuard } from '../settings/UnsavedChangesGuard';
import { FinancialReconciliationReportView } from './FinancialReconciliationReportView';
import type { ReconciliationAsset } from './financialReconciliationApi';
import {
  appendSnapshotFollowup, captureSnapshot, loadFollowups, loadSnapshot, loadSnapshotHistory,
  type FollowupHistory, type FollowupRecord, type SnapshotSummary
} from './financialReconciliationSnapshotsApi';

export function FinancialReconciliationSnapshots({ asset, disabled }: { asset: ReconciliationAsset; disabled: boolean }) {
  const session = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('admin'));
  const access = useAdminAccess();
  if (!session || !hasAdminPermission(access, 'governance.financial.read')) return null;
  return <SnapshotWorkbench key={`${session.generation}:${asset.asset_id}`} asset={asset} disabled={disabled}
    session={session} canOperate={hasAdminPermission(access, 'governance.financial.operate')} />;
}

function SnapshotWorkbench({ asset, disabled, session, canOperate }: {
  asset: ReconciliationAsset; disabled: boolean; session: AuthSession; canOperate: boolean;
}) {
  const [visible, setVisible] = useState(false);
  const [page, setPage] = useState(1);
  const [selected, setSelected] = useState<number>();
  const [capturing, setCapturing] = useState(false);
  const [editing, setEditing] = useState(false);
  const [notice, setNotice] = useState('');
  const inFlight = useRef(false);
  const history = useQuery({
    queryKey: ['reconciliation-snapshots', session.generation, session.subject, asset.asset_id, page],
    queryFn: ({ signal }) => loadSnapshotHistory(asset.asset_id, page, signal),
    enabled: visible && selected === undefined, retry: false, refetchOnWindowFocus: false
  });
  async function capture(reason: string) {
    if (inFlight.current) return;
    inFlight.current = true; setCapturing(true);
    try {
      const result = await captureSnapshot(session, asset.asset_id, reason);
      setNotice(`人工采集 #${result.id} 已保存；历史覆盖仍不完整。`);
      setSelected(result.id); setVisible(true);
    } finally { inFlight.current = false; setCapturing(false); }
  }
  return <>
    <Space wrap style={{ marginBlock: 16 }}>
      {canOperate ? <ConfirmAction actionText="人工采集" actionAriaLabel={`人工采集 ${asset.symbol}`}
        title={`人工采集 ${asset.symbol} 对账证据`} disabled={disabled || capturing}
        description="服务器将重新读取当前证据并保存不可变记录。采集时间不是日终结账时间；不补历史、不修改余额。"
        modalWidth="min(520px, calc(100vw - 32px))" onConfirm={capture} /> : null}
      <Button disabled={capturing} onClick={() => { setPage(1); setSelected(undefined); setVisible(true); }}>采集历史</Button>
    </Space>
    {notice ? <Banner type="success" closeIcon={null} description={notice} /> : null}
    <SideSheet title={`${asset.symbol} 人工采集历史`} visible={visible} width="min(1120px, 100vw)"
      maskClosable={false} closeOnEsc={!editing} onCancel={() => { if (!editing) setVisible(false); }} keepDOM={false}>
      {selected === undefined ? <>
        <Space wrap style={{ marginBottom: 16 }}>
          <Typography.Text>历史证据按人工采集时间保存，不是日终结账。</Typography.Text>
          <Button aria-label="刷新采集历史" icon={<IconRefresh aria-hidden="true" />} disabled={history.isFetching}
            onClick={() => void history.refetch()}>刷新</Button>
        </Space>
        {history.error ? <Banner type="danger" closeIcon={null} description={adminErrorMessage(history.error, '采集历史读取失败')} /> : null}
        <ResizableTable<SnapshotSummary> aria-label="人工采集记录" rowKey="id" pagination={false}
          loading={history.isFetching} dataSource={history.data?.snapshots ?? []} empty="暂无人工采集记录"
          columns={[
            { dataIndex: 'id', title: '采集 ID', width: 120 },
            { key: 'captured_at', title: '人工采集时间', width: 220, render: (_, row) => <TimestampText value={row.captured_at} /> },
            { dataIndex: 'admin_id', title: '采集管理员 ID', width: 150 },
            { dataIndex: 'reason', title: '采集原因', width: 300 },
            { key: 'actions', title: '操作', width: 160, render: (_, row) =>
              <Button theme="borderless" disabled={history.isFetching || Boolean(history.error)}
                aria-label={`查看采集 ${row.id}`} onClick={() => setSelected(row.id)}>查看证据</Button> }
          ]} />
        <Pagination currentPage={page} pageSize={20} total={Math.min(history.data?.total ?? 0, 100_020)}
          disabled={history.isFetching} onPageChange={setPage} showTotal style={{ marginTop: 16 }} />
      </> : <SnapshotEvidence key={selected} id={selected} assetId={asset.asset_id}
        session={session} canOperate={canOperate} onEditing={setEditing} onBack={() => setSelected(undefined)} />}
    </SideSheet>
  </>;
}

function SnapshotEvidence({ id, assetId, session, canOperate, onBack, onEditing }: {
  id: number; assetId: number; session: AuthSession; canOperate: boolean; onBack: () => void; onEditing: (value: boolean) => void;
}) {
  const [page, setPage] = useState(1);
  const [editing, setEditing] = useState(false);
  const evidence = useQuery({
    queryKey: ['reconciliation-snapshot', session.generation, session.subject, id],
    queryFn: ({ signal }) => loadSnapshot(id, assetId, signal), retry: false, refetchOnWindowFocus: false
  });
  const followups = useQuery({
    queryKey: ['reconciliation-followups', session.generation, session.subject, id, page],
    queryFn: ({ signal }) => loadFollowups(id, page, signal), retry: false, refetchOnWindowFocus: false
  });
  return <>
    <Space wrap style={{ marginBottom: 16 }}>
      <Button disabled={editing} onClick={onBack}>返回采集列表</Button>
      <Button aria-label="刷新跟进记录" icon={<IconRefresh aria-hidden="true" />}
        disabled={editing || followups.isFetching} onClick={() => void followups.refetch()}>刷新跟进</Button>
    </Space>
    <Banner type="warning" closeIcon={null} description="以下为保存的原始证据，不是当前余额或日终结账。历史与托管证据仍不完整；备注不代表差异已解决。" />
    {evidence.error ? <Banner type="danger" closeIcon={null} description={adminErrorMessage(evidence.error, '原始证据读取失败')} /> : null}
    {evidence.data ? <>
      <Space wrap style={{ marginBlock: 16 }}>
        <Typography.Text strong>采集 #{id}</Typography.Text>
        <Typography.Text>人工采集时间 <TimestampText value={evidence.data.captured_at} /></Typography.Text>
        <Typography.Text>采集管理员 #{evidence.data.admin_id}</Typography.Text>
      </Space>
      <p style={{ overflowWrap: 'anywhere' }}>采集原因：{evidence.data.reason}</p>
      <Typography.Text type="tertiary" style={{ overflowWrap: 'anywhere' }}>证据摘要：{evidence.data.report_hash}</Typography.Text>
      <FinancialReconciliationReportView report={evidence.data.report} />
    </> : <section className="admin-table-state">{evidence.isPending ? '正在读取原始证据…' : '原始证据不可用'}</section>}
    <Typography.Title heading={5} style={{ marginTop: 24 }}>差异跟进记录</Typography.Title>
    {followups.error ? <Banner type="danger" closeIcon={null} description={adminErrorMessage(followups.error, '跟进记录读取失败')} /> : null}
    {followups.data ? <>
      <p>追加记录 {followups.data.total} 条；最新版本 {followups.data.latest_version}。</p>
      {canOperate && evidence.data && page === 1 ? <FollowupEditor key={id} session={session}
        snapshot={evidence.data} history={followups.data}
        disabled={followups.isFetching || Boolean(followups.error) || Boolean(evidence.error)}
        onEditing={(value) => { setEditing(value); onEditing(value); }} onSaved={async () => { await followups.refetch(); }} /> : null}
      <ResizableTable<FollowupRecord> aria-label="采集跟进记录" rowKey="id" pagination={false}
        dataSource={followups.data.records} loading={followups.isFetching} empty="尚未登记负责人、期限或跟进记录"
        columns={[
          { dataIndex: 'version', title: '版本', width: 100 },
          { key: 'owner', title: '负责人', width: 150, render: (_, row) => row.owner_admin_id === null ? '未分配' : `管理员 #${row.owner_admin_id}` },
          { key: 'due', title: '明确处理期限', width: 220, render: (_, row) => row.due_at === null ? '未设置' : <TimestampText value={row.due_at} /> },
          { dataIndex: 'notes', title: '跟进备注', width: 300, render: (value) => <span style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{value}</span> },
          { dataIndex: 'admin_id', title: '登记管理员 ID', width: 160 },
          { key: 'recorded_at', title: '登记时间', width: 220, render: (_, row) => <TimestampText value={row.recorded_at} /> },
          { dataIndex: 'reason', title: '操作原因', width: 260 }
        ]} />
      <Pagination currentPage={page} pageSize={20} total={Math.min(followups.data.total, 100_020)}
        disabled={editing || followups.isFetching} onPageChange={setPage} showTotal style={{ marginTop: 16 }} />
    </> : <section className="admin-table-state">{followups.isPending ? '正在读取跟进记录…' : '跟进记录不可用'}</section>}
  </>;
}

function FollowupEditor({ session, snapshot, history, disabled, onEditing, onSaved }: {
  session: AuthSession; snapshot: SnapshotSummary; history: FollowupHistory; disabled: boolean;
  onEditing: (value: boolean) => void; onSaved: () => Promise<void>;
}) {
  const [version, setVersion] = useState<number>();
  const [owner, setOwner] = useState('');
  const [due, setDue] = useState<number | null>(null);
  const [notes, setNotes] = useState('');
  const [reason, setReason] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [discard, setDiscard] = useState(false);
  const inFlight = useRef(false);
  const ownerValid = owner === '' || (/^[1-9]\d*$/.test(owner) && Number.isSafeInteger(Number(owner)));
  const dueValid = due === null || (Number.isSafeInteger(due) && due >= -30_610_224_000_000 && due <= 253_402_300_799_999);
  const valid = !disabled && ownerValid && dueValid && notes.trim() !== '' && [...notes.trim()].length <= 2000
    && reason.trim() !== '' && [...reason.trim()].length <= 500;
  function close() { setVersion(undefined); setDiscard(false); onEditing(false); }
  async function save() {
    if (!valid || version === undefined || inFlight.current) return;
    inFlight.current = true; setSubmitting(true); setError('');
    try {
      await appendSnapshotFollowup(session, snapshot, {
        expected_version: version, owner_admin_id: owner === '' ? null : Number(owner), due_at: due, notes
      }, reason);
      close();
      await onSaved();
    } catch (cause) { setError(adminErrorMessage(cause, '跟进登记失败；保留原版本和草稿，请重试核对或放弃后重新读取')); }
    finally { inFlight.current = false; setSubmitting(false); }
  }
  return <>
    <Button style={{ marginBottom: 16 }} disabled={disabled} onClick={() => {
      const latest = history.records[0];
      setVersion(history.latest_version); setOwner(latest?.owner_admin_id ? String(latest.owner_admin_id) : '');
      setDue(latest?.due_at ?? null); setNotes(''); setReason(''); setError(''); onEditing(true);
    }}>登记跟进</Button>
    {version !== undefined ? <UnsavedChangesGuard enabled /> : null}
    <Modal title={`登记采集 #${snapshot.id} 跟进`} visible={version !== undefined}
      width="min(560px, calc(100vw - 32px))" motion={false} maskClosable={false} closeOnEsc={!submitting}
      okText="追加跟进记录" onOk={save} confirmLoading={submitting}
      okButtonProps={{ 'aria-label': '追加跟进记录', disabled: !valid || submitting }}
      cancelButtonProps={{ 'aria-label': '取消跟进', disabled: submitting }}
      onCancel={() => { if (!submitting) setDiscard(true); }}>
      <p>仅追加负责人、期限和备注，不改变财务证据、资金或业务状态。</p>
      {error ? <div role="alert"><Typography.Text type="danger">{error}</Typography.Text></div> : null}
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
        <label style={{ width: '100%' }}>跟进备注
          <TextArea aria-label="跟进备注" value={notes} onChange={setNotes} disabled={submitting} maxCount={2000} />
        </label>
        <label style={{ width: '100%' }}>操作原因
          <TextArea aria-label="操作原因" value={reason} onChange={setReason} disabled={submitting} maxCount={500} />
        </label>
      </Space>
    </Modal>
    <Modal title="放弃跟进草稿？" visible={discard} motion={false} maskClosable={false}
      okText="放弃草稿" cancelText="继续编辑" onOk={close} onCancel={() => setDiscard(false)}
      okButtonProps={{ 'aria-label': '放弃草稿' }} cancelButtonProps={{ 'aria-label': '继续编辑' }}>
      未保存备注将丢失；已发送但结果未确认的请求仍保留原幂等键。
    </Modal>
  </>;
}
