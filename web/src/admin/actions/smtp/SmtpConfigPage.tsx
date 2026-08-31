import { Banner, Button, Card, SideSheet, Space, Tabs, Toast, Typography } from '@douyinfe/semi-ui';

import { PageHeader } from '../../../layouts/PageHeader';
import { AdminRequestActionBoundary } from '../../access';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminSelect, AdminTextInput } from '../../../shared/SemiFormControls';
import { deliveryStrategyOptions, smtpModuleTabs } from './model';
import { SmtpConfigFields, SmtpCredentialMetadata } from './SmtpConfigFields';
import { SmtpConfigTable } from './SmtpConfigTable';
import type { SmtpModuleTab } from './types';
import { submitSmtpAction, useSmtpConfigWorkspace } from './useSmtpConfigWorkspace';
import { VerificationTemplatesEditor } from './VerificationTemplatesEditor';

const { Text, Title } = Typography;

export function SmtpConfigPage() {
  const workspace = useSmtpConfigWorkspace();
  const testConfigOptions = [
    { value: 'strategy', label: '按当前策略选择' },
    ...workspace.configs.map((item) => ({
      value: String(item.id),
      label: `${item.name}${item.enabled ? '' : '（未启用）'}`
    }))
  ];
  const selectedConfigTitle = workspace.selectedConfigId ? '编辑发信配置' : '发信配置';

  const saveConfigAction = (
    <AdminRequestActionBoundary endpoint={`/admin/api/v1/smtp/configs/${workspace.selectedConfigId}`} method="PATCH">
      <ConfirmAction
        actionText="保存配置"
        title="确认保存 SMTP 配置"
        onConfirm={(reason) =>
          submitSmtpAction('保存 SMTP 配置', () => workspace.saveCurrentConfig(reason))
        }
      />
    </AdminRequestActionBoundary>
  );

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="SMTP 邮件配置" />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Banner
          closeIcon={null}
          description={workspace.compatibility.description}
          fullMode={false}
          title="旧版单例兼容状态"
          type={workspace.compatibility.warning ? 'warning' : 'info'}
        />
        <Tabs
          activeKey={workspace.activeTab}
          className="admin-action-tabs"
          onChange={(nextTab) => workspace.setActiveTab(nextTab as SmtpModuleTab)}
          tabBarExtraContent={
            <Button
              loading={workspace.loading}
              onClick={() =>
                void workspace.loadConfig().catch((error: unknown) =>
                  Toast.error(error instanceof Error ? error.message : '加载 SMTP 配置失败')
                )
              }
              theme="borderless"
            >
              刷新
            </Button>
          }
          tabList={smtpModuleTabs}
          type="button"
        />

        {workspace.activeTab === 'configs' ? (
          <div className="admin-action-workbench-grid">
            <section className="admin-action-panel">
              <div className="admin-earn-section-header">
                <Title heading={4}>发信配置列表</Title>
                <AdminRequestActionBoundary endpoint="/admin/api/v1/smtp/configs" method="POST">
                  <Button onClick={workspace.startCreateConfig} theme="solid" type="primary">新增配置</Button>
                </AdminRequestActionBoundary>
              </div>
              <SmtpConfigTable
                configs={workspace.configs}
                loading={workspace.loading}
                onSelect={workspace.selectConfig}
                onToggle={workspace.toggleConfigEnabled}
                selectedConfigId={workspace.selectedConfigId}
              />
            </section>
            <section className="admin-action-panel">
              <Title heading={4}>{selectedConfigTitle}</Title>
              {workspace.selectedConfigId ? (
                <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                  <SmtpCredentialMetadata
                    config={workspace.selectedConfig}
                    lastTestResult={workspace.lastTestResult}
                  />
                  <SmtpConfigFields form={workspace.configForm} onChange={workspace.setConfigForm} />
                  {saveConfigAction}
                </Space>
              ) : (
                <Text type="secondary">暂无发信配置</Text>
              )}
            </section>
          </div>
        ) : null}

        {workspace.activeTab === 'templates' ? (
          <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
            <Text type="secondary">{selectedConfigTitle}：{workspace.configForm.name}</Text>
            <VerificationTemplatesEditor
              form={workspace.configForm}
              mode="edit"
              onChange={workspace.setConfigForm}
            />
            {saveConfigAction}
          </Space>
        ) : null}

        {workspace.activeTab === 'strategy' ? (
          <section className="admin-action-panel">
            <Title heading={4}>发信策略</Title>
            <div className="admin-action-form admin-action-form-wide">
              <label>
                发送策略
                <AdminSelect
                  ariaLabel="发送策略"
                  onChange={workspace.setDeliveryStrategy}
                  optionList={deliveryStrategyOptions}
                  value={workspace.deliveryStrategy}
                />
              </label>
            </div>
            <AdminRequestActionBoundary endpoint="/admin/api/v1/smtp/delivery-settings" method="PATCH">
              <ConfirmAction
                actionText="保存策略"
                title="确认保存 SMTP 发信策略"
                onConfirm={(reason) =>
                  submitSmtpAction('保存 SMTP 发信策略', () => workspace.saveDeliverySettings(reason))
                }
              />
            </AdminRequestActionBoundary>
          </section>
        ) : null}

        {workspace.activeTab === 'test' ? (
          <section className="admin-action-panel">
            <Title heading={4}>测试发送</Title>
            <div className="admin-action-form">
              <label>
                发信方式
                <AdminSelect
                  ariaLabel="发信方式"
                  onChange={workspace.setTestConfigChoice}
                  optionList={testConfigOptions}
                  value={workspace.testConfigChoice}
                />
              </label>
              <label>
                测试收件邮箱
                <AdminTextInput
                  ariaLabel="测试收件邮箱"
                  value={workspace.testRecipient}
                  onChange={workspace.setTestRecipient}
                />
              </label>
            </div>
            <AdminRequestActionBoundary endpoint="/admin/api/v1/smtp/test" method="POST">
              <ConfirmAction
                actionText="测试发送"
                title="确认发送 SMTP 测试邮件"
                onConfirm={(reason) => submitSmtpAction('SMTP 测试邮件', () => workspace.sendTest(reason))}
              />
            </AdminRequestActionBoundary>
            {workspace.lastTestResult ? (
              <Text type="secondary">
                最近测试收件邮箱：{workspace.lastTestResult.recipient} / {workspace.lastTestResult.configName}
              </Text>
            ) : (
              <Text type="secondary">最近测试：本会话尚未测试</Text>
            )}
          </section>
        ) : null}
      </Card>

      {workspace.createSheetVisible ? (
        <SideSheet
          onCancel={() => workspace.setCreateSheetVisible(false)}
          title="新增发信配置"
          visible={workspace.createSheetVisible}
          width={760}
        >
          <Card bordered={false}>
            <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
              <SmtpConfigFields form={workspace.createConfigForm} onChange={workspace.setCreateConfigForm} />
              <VerificationTemplatesEditor
                form={workspace.createConfigForm}
                mode="create"
                onChange={workspace.setCreateConfigForm}
              />
              <AdminRequestActionBoundary endpoint="/admin/api/v1/smtp/configs" method="POST">
                <ConfirmAction
                  actionText="新增配置"
                  title="确认新增 SMTP 配置"
                  onConfirm={(reason) => submitSmtpAction('新增 SMTP 配置', () => workspace.createConfig(reason))}
                />
              </AdminRequestActionBoundary>
            </Space>
          </Card>
        </SideSheet>
      ) : null}
    </main>
  );
}
