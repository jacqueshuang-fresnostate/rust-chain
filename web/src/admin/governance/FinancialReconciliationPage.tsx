import { IconRefresh } from '@douyinfe/semi-icons';
import { Banner, Button, Pagination, Space, Typography } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useState, useSyncExternalStore } from 'react';

import { authStore } from '../../auth/authStore';
import { PageHeader } from '../../layouts/PageHeader';
import { adminErrorMessage } from '../../shared/adminErrorMessage';
import { ResizableTable } from '../../shared/ResizableTable';
import { TimestampText } from '../../shared/TimestampText';
import { FinancialReconciliationReportView } from './FinancialReconciliationReportView';
import { FinancialReconciliationSnapshots } from './FinancialReconciliationSnapshots';
import { loadFinancialReconciliation, type ReconciliationAsset } from './financialReconciliationApi';

export function FinancialReconciliationPage() {
  const [page, setPage] = useState(1);
  const [assetId, setAssetId] = useState<number>();
  const session = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('admin'));
  const query = useQuery({
    queryKey: ['admin-financial-reconciliation', session?.subject, session?.generation, page, assetId],
    queryFn: ({ signal }) => loadFinancialReconciliation(page, assetId, signal),
    retry: false, throwOnError: false, refetchOnWindowFocus: false
  });
  return <main className="exchange-page admin-action-page">
    <PageHeader title="平台资产对账" actions={
      <Button icon={<IconRefresh aria-hidden="true" />} disabled={query.isFetching}
        onClick={() => void query.refetch({ throwOnError: false })}>刷新</Button>
    } />
    <Banner type="warning" closeIcon={null}
      description="历史覆盖不完整。本页不是偿付能力评估或储备证明；库存分录净变动不等于真实托管库存。" />
    {query.error ? <Banner type="danger" closeIcon={null}
      description={`${adminErrorMessage(query.error, '平台对账加载失败')}${query.data ? '；保留上次快照。' : ''}`} /> : null}
    {query.data ? <>
      <Space wrap style={{ marginBlock: 16 }}>
        <Typography.Text>资产目录 {query.data.total} 项（含停用资产）</Typography.Text>
        <Typography.Text type="tertiary">快照时间 <TimestampText value={query.data.checked_at} /></Typography.Text>
      </Space>
      <ResizableTable<ReconciliationAsset> aria-label="对账资产目录" rowKey="asset_id"
        dataSource={query.data.assets} pagination={false} loading={query.isFetching} empty="暂无资产"
        columns={[
          { dataIndex: 'asset_id', title: '资产 ID', width: 120 },
          { dataIndex: 'symbol', title: '资产', width: 180 },
          { dataIndex: 'precision_scale', title: '资产精度', width: 120 },
          { key: 'actions', title: '操作', width: 200, render: (_, row) =>
            <Button theme="borderless" disabled={query.isFetching} aria-label={`查看 ${row.symbol} 对账`}
              onClick={() => setAssetId(row.asset_id)}>{assetId === row.asset_id ? '当前资产' : '查看对账'}</Button> }
        ]} />
      <Pagination currentPage={page} pageSize={20} total={Math.min(query.data.total, 100_020)}
        disabled={query.isFetching} onPageChange={(next) => { setPage(next); setAssetId(undefined); }}
        showTotal style={{ marginBlock: 16 }} />
      {query.data.report
        ? <section key={query.data.report.asset.asset_id}>
          <FinancialReconciliationSnapshots asset={query.data.report.asset} disabled={query.isFetching || Boolean(query.error)} />
          <FinancialReconciliationReportView report={query.data.report} />
        </section>
        : <section className="admin-table-state">尚未选择对账资产</section>}
    </> : <section className="admin-table-state" aria-live="polite">
      {query.isPending ? '正在读取对账证据…' : '对账证据不可用'}
    </section>}
  </main>;
}
