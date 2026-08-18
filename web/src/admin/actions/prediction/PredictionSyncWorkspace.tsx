import { IconSync } from '@douyinfe/semi-icons';
import { Banner, Button, Card, Col, Descriptions, Row, Space, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { type ComponentPropsWithoutRef, useMemo } from 'react';

import { PageHeader } from '../../../layouts/PageHeader';
import { ResizableTable } from '../../../shared/ResizableTable';
import { TimestampText } from '../../../shared/TimestampText';
import { containedTableStyle } from '../../../shared/tableLayout';
import { WorkflowPageActions } from '../../components/WorkflowPageActions';
import { triggerTypeLabel } from './model';
import { PredictionSyncStatusTag } from './PredictionStatus';
import type { PredictionSyncLog } from './types';
import { usePredictionSync } from './usePredictionSync';

type PredictionTableProps = ComponentPropsWithoutRef<'table'>;

function SyncLogTable(props: PredictionTableProps) {
  return <table {...props} aria-label="竞猜同步日志表" />;
}

export function PredictionSyncWorkspace() {
  const workspace = usePredictionSync();
  const syncLogColumns = useMemo<Array<ColumnProps<PredictionSyncLog>>>(
    () => [
      {
        dataIndex: 'trigger_type',
        title: '触发方式',
        width: 140,
        render: (value) => <span>{triggerTypeLabel(typeof value === 'string' ? value : null)}</span>
      },
      {
        dataIndex: 'status',
        title: '状态',
        width: 120,
        render: (value) => <PredictionSyncStatusTag value={typeof value === 'string' ? value : null} />
      },
      { dataIndex: 'imported_count', title: '新增', width: 100 },
      { dataIndex: 'updated_count', title: '更新', width: 100 },
      {
        dataIndex: 'error_message',
        title: '错误信息',
        ellipsis: true,
        render: (value) => <span>{typeof value === 'string' && value ? value : '-'}</span>
      },
      {
        dataIndex: 'started_at',
        title: '开始时间',
        width: 180,
        render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />
      },
      {
        dataIndex: 'finished_at',
        title: '结束时间',
        width: 180,
        render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />
      }
    ],
    []
  );
  const syncData = useMemo(
    () => [
      {
        key: '最近状态',
        value: <PredictionSyncStatusTag value={workspace.settings?.last_sync_status ?? null} />
      },
      {
        key: '最近成功',
        value: <TimestampText value={workspace.settings?.last_successful_sync_at ?? null} />
      },
      {
        key: '开始时间',
        value: <TimestampText value={workspace.settings?.last_sync_started_at ?? null} />
      },
      {
        key: '结束时间',
        value: <TimestampText value={workspace.settings?.last_sync_finished_at ?? null} />
      },
      { key: '新增市场', value: workspace.settings?.last_sync_imported_count ?? '-' },
      { key: '更新市场', value: workspace.settings?.last_sync_updated_count ?? '-' }
    ],
    [workspace.settings]
  );

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <WorkflowPageActions
            loading={workspace.loading}
            onRefresh={() => void workspace.loadSync()}
            shortcutLabel="返回竞猜配置"
            shortcutPath="/admin/prediction/settings"
          />
        }
        description="手动执行 Polymarket 同步并查看最近运行结果，不在此处编辑配置。"
        title="竞猜同步运行"
      />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Space align="start" spacing={18} vertical style={{ width: '100%' }}>
          <section className="admin-action-panel">
            <Row align="middle" gutter={[24, 16]} justify="space-between" style={{ width: '100%' }} type="flex">
              <Col xs={24} lg={18}>
                <Typography.Title heading={4}>同步任务</Typography.Title>
                <Descriptions align="plain" column={3} data={syncData} layout="horizontal" />
              </Col>
              <Col xs={24} lg={6}>
                <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                  <Button
                    icon={<IconSync aria-hidden="true" />}
                    loading={workspace.syncing}
                    onClick={() => void workspace.triggerSync()}
                    theme="solid"
                    type="primary"
                  >
                    立即同步
                  </Button>
                </Space>
              </Col>
            </Row>
            {workspace.settings?.last_sync_error ? (
              <Banner fullMode={false} type="danger" description={workspace.settings.last_sync_error} />
            ) : null}
          </section>

          <section className="admin-action-panel">
            <Typography.Title heading={4}>同步日志</Typography.Title>
            <ResizableTable
              aria-label="竞猜同步日志表"
              bordered
              columns={syncLogColumns}
              components={{ body: { outer: SyncLogTable } }}
              dataSource={workspace.syncLogs}
              loading={workspace.loading}
              pagination={false}
              rowKey="id"
              style={containedTableStyle}
            />
          </section>
        </Space>
      </Card>
    </main>
  );
}
