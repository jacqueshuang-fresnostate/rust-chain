import { Button, Space, Typography } from '@douyinfe/semi-ui';

import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminRequestActionBoundary } from '../../access';
import { ResizableTable } from '../../../shared/ResizableTable';
import { StatusTag } from '../../../shared/StatusTag';
import type { SmtpConfig } from './types';
import { submitSmtpAction } from './useSmtpConfigWorkspace';

export function SmtpConfigTable({
  configs,
  loading,
  onSelect,
  onToggle,
  selectedConfigId
}: {
  configs: SmtpConfig[];
  loading: boolean;
  onSelect: (config: SmtpConfig) => void;
  onToggle: (config: SmtpConfig, enabled: boolean, reason: string) => Promise<void>;
  selectedConfigId: string;
}) {
  const columns = [
    {
      dataIndex: 'name',
      key: 'name',
      title: '配置名称',
      render: (_value: unknown, record: SmtpConfig) => (
        <Space spacing={8}>
          <Typography.Text strong>{record.name}</Typography.Text>
          {String(record.id) === selectedConfigId ? <Typography.Text type="tertiary">编辑中</Typography.Text> : null}
        </Space>
      )
    },
    { dataIndex: 'host', key: 'host', title: 'SMTP host' },
    { dataIndex: 'from_email', key: 'from_email', title: '发件邮箱' },
    { dataIndex: 'priority', key: 'priority', title: '优先级' },
    {
      dataIndex: 'password_set',
      key: 'password_set',
      title: '凭据状态',
      render: (_value: unknown, record: SmtpConfig) => (
        <span>{record.password_set ? '密码已配置' : '密码未配置'}</span>
      )
    },
    {
      dataIndex: 'enabled',
      key: 'enabled',
      title: '启用状态',
      render: (enabled: boolean) => <StatusTag value={enabled} />
    },
    {
      key: 'actions',
      title: '操作',
      render: (_value: unknown, record: SmtpConfig) => (
        <Space>
          <Button onClick={() => onSelect(record)} theme="borderless">编辑</Button>
          <AdminRequestActionBoundary endpoint={`/admin/api/v1/smtp/configs/${record.id}`} method="PATCH">
            <ConfirmAction
              actionText={record.enabled ? '停用' : '启用'}
              title={record.enabled ? '确认停用发信配置' : '确认启用发信配置'}
              onConfirm={(reason) =>
                submitSmtpAction(record.enabled ? '停用发信配置' : '启用发信配置', () =>
                  onToggle(record, !record.enabled, reason)
                )
              }
            />
          </AdminRequestActionBoundary>
        </Space>
      )
    }
  ];

  return (
    <ResizableTable
      aria-label="SMTP 发信配置列表"
      columns={columns}
      dataSource={configs}
      loading={loading}
      pagination={false}
      rowKey="id"
      style={{ width: '100%' }}
    />
  );
}
