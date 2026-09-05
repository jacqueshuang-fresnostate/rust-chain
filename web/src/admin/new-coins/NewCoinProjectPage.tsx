import { Button, Card, Space, Tabs, TabPane, Typography } from '@douyinfe/semi-ui';
import { useCallback, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useParams } from 'react-router-dom';
import { apiRequest, ApiError } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { TimestampText } from '../../shared/TimestampText';
import { AmountText } from '../../shared/AmountText';
import { StatusTag } from '../../shared/StatusTag';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminRequestActionBoundary, useCanAdminRequest } from '../access';
import { ADMIN_OPTION_QUERY_KEY } from '../sharedOptionQuery';
import { loadProjectCenter, projectQueryKey, stages } from './projectModel';
import { NewCoinProjectSettings } from './NewCoinProjectSettings';
import { NewCoinGrant } from './NewCoinGrant';

function RecordLink({ path, children }: { path:string; children:React.ReactNode }) {
  const endpoint=path.split('?')[0].replace('/admin/','/admin/api/v1/');
  const canRead=useCanAdminRequest(endpoint,'GET');
  return canRead ? <Link to={path}>{children}</Link> : null;
}

export function NewCoinProjectPage() {
  const { projectId = '' }=useParams();
  const queryClient=useQueryClient();
  const [dirty,setDirty]=useState(false);
  const [error,setError]=useState('');
  const [revision,setRevision]=useState(0);
  const query=useQuery({queryKey:projectQueryKey(projectId),queryFn:({signal})=>loadProjectCenter(projectId,signal),enabled:/^[1-9]\d*$/.test(projectId),retry:false,refetchOnWindowFocus:false});
  const canWrite=useCanAdminRequest(`/admin/api/v1/new-coins/${projectId}/issuance`,'PATCH');
  const reload=useCallback(async()=>{
    await query.refetch(); setRevision(v=>v+1); setDirty(false);
    void queryClient.invalidateQueries({predicate:q=>q.queryKey[0]===ADMIN_OPTION_QUERY_KEY&&q.queryKey[2]==='reference:newCoinProject:100'});
  },[query.refetch,queryClient]);
  const data=query.data;
  if(!/^[1-9]\d*$/.test(projectId)) return <main className="exchange-page"><PageHeader title="项目编号无效" /><Link to="/admin/new-coins/projects">返回项目管理</Link></main>;
  if(!data || query.error) return <main className="exchange-page"><PageHeader title="新币项目中心" />
    <Card><Typography.Text>{query.isFetching?'正在读取项目配置…':query.error?.message ?? '项目不存在'}</Typography.Text><Button onClick={()=>void query.refetch()}>重新加载项目</Button></Card>
  </main>;
  const p=data.project;
  const stage=stages.find(s=>s.value===p.lifecycle_status);
  return <main className="exchange-page">
    <PageHeader title={`${p.symbol} · 项目中心`} description={`项目 ID ${p.id} · 发行资产 ${p.asset_id} · 计价资产 ${p.quote_asset_id ?? '未配置'}`}
      actions={<Space><Link to="/admin/new-coins/projects">返回项目管理</Link><Button disabled={dirty||query.isFetching} onClick={()=>void reload()}>刷新项目</Button></Space>} />
    <Card bordered={false}>
      <Space vertical align="start" spacing={16} style={{width:'100%'}}>
        <Space wrap><StatusTag value={p.lifecycle_status}/><StatusTag value={p.status}/><Typography.Text>预热中 → 申购中 → 派发中 → 已上市</Typography.Text></Space>
        <Typography.Text>申购只冻结资金；结束申购后人工派发并退差额，结清待处理订单后才允许上市。项目启停与个人解禁是独立状态。</Typography.Text>
        {data.lifecycle_block_reason ? <Typography.Text type="warning">{data.lifecycle_block_reason}</Typography.Text> : null}
        {dirty ? <Typography.Text type="warning">请先保存或重置配置，再推进项目阶段。</Typography.Text> : null}
        {error ? <Typography.Text type="danger">{error}</Typography.Text> : null}
        {data.next_lifecycle_status ? <AdminRequestActionBoundary key="lifecycle-action" endpoint={`/admin/api/v1/new-coins/${p.id}/lifecycle`} method="PATCH">
          <ConfirmAction actionText={stage?.action ?? '推进阶段'} disabled={!!data.lifecycle_block_reason||dirty||query.isFetching}
            title={`确认 ${p.symbol} ${stage?.action}；${p.lifecycle_status==='subscription'?'结束后停止接受新申购，不自动派发':p.lifecycle_status==='distribution'?'以本次操作时间记录上市，不自动启用现货交易对':'开放用户申购并冻结资金'}`}
            onConfirm={async reason=>{
              try { await apiRequest(`/admin/api/v1/new-coins/${p.id}/lifecycle`,{method:'PATCH',body:JSON.stringify({lifecycle_status:data.next_lifecycle_status,expected_config:data.configuration_version,reason})});setError('');await reload(); }
              catch(cause){setError(cause instanceof ApiError&&cause.status===409?'项目已变化或存在待处理申购，请刷新后重试。':cause instanceof Error?cause.message:'操作失败');throw cause;}
            }} />
        </AdminRequestActionBoundary> : null}
      </Space>
    </Card>
    <Tabs keepDOM lazyRender tabPaneMotion={false} style={{marginTop:24}}>
      <TabPane itemKey="overview" tab="项目概览">
        <Card bordered={false}><Space vertical align="start" spacing={16}>
          <Typography.Text>发行总量：<AmountText value={p.total_supply}/> · 发行价：<AmountText value={p.issue_price}/></Typography.Text>
          <Typography.Text>已预留：<AmountText value={p.reserved_supply}/> · 已派发：<AmountText value={p.allocated_supply}/> · 剩余额度：<AmountText value={p.remaining_supply}/></Typography.Text>
          <Typography.Text>计划上市时间：<TimestampText value={p.listed_at}/> · 实际上市时间：{p.actual_listed_at === null ? (p.lifecycle_status === 'listed' ? '历史事件未记录' : '尚未确认上市') : <TimestampText value={p.actual_listed_at}/>}</Typography.Text>
          <Typography.Text>计划时间不自动推进阶段；新增“上市即解禁”仓位须等待实际上市，固定时间和相对周期沿用各自到期规则。</Typography.Text>
          <Typography.Text>累计申购订单：{data.subscription_count} · 待派发 / 退款：{data.pending_manual_count}</Typography.Text>
          <Typography.Text>资产和计价币种在项目创建后保持不变；发行参数仅在预热且无订单、无额度占用时可修改。</Typography.Text>
        </Space></Card>
      </TabPane>
      <TabPane itemKey="settings" tab={canWrite?'项目配置':'配置详情'}>
        <NewCoinProjectSettings key={`${p.id}:${revision}`} data={data} onSaved={reload} onDirty={setDirty}/>
      </TabPane>
      <TabPane itemKey="records" tab="关联记录">
        <Card bordered={false}><Space vertical align="start" spacing={16}>
          <RecordLink path={`/admin/new-coins/subscriptions?project_id=${p.id}&status=pending`}>处理待派发申购</RecordLink>
          <RecordLink path={`/admin/new-coins/subscriptions?project_id=${p.id}`}>全部申购与配售</RecordLink>
          <RecordLink path={`/admin/new-coins/distributions?project_id=${p.id}`}>派发与退款记录</RecordLink>
          <RecordLink path={`/admin/new-coins/lock-positions?asset_id=${p.asset_id}`}>该资产锁仓（含其他项目及来源）</RecordLink>
          <RecordLink path={`/admin/new-coins/unlocks?asset_id=${p.asset_id}`}>该资产解禁记录（含其他项目及来源）</RecordLink>
          <RecordLink path={`/admin/new-coins/purchases?project_id=${p.id}`}>上市后购买订单</RecordLink>
          <RecordLink path={`/admin/audit-logs?target_type=new_coin_project&target_id=${p.id}`}>项目配置与生命周期审计</RecordLink>
        </Space></Card>
      </TabPane>
      {canWrite ? <TabPane itemKey="grant" tab="额外赠币" disabled={dirty}><Card bordered={false}><NewCoinGrant project={p} onSaved={reload}/></Card></TabPane> : null}
    </Tabs>
  </main>;
}
