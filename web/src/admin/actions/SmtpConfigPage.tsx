import { Button, Card, Space, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminCheckbox, AdminPasswordInput, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import { StatusTag } from '../../shared/StatusTag';

const { Text, Title } = Typography;

const defaultConfigForm = {
  enabled: false,
  fromEmail: '',
  fromName: '',
  host: '',
  password: '',
  port: '587',
  security: 'starttls',
  username: ''
};

const securityOptions = [
  { value: 'none', label: 'None' },
  { value: 'starttls', label: 'STARTTLS' },
  { value: 'tls', label: 'TLS' }
];

type SmtpConfig = {
  enabled: boolean;
  from_email: string;
  from_name?: string | null;
  host: string;
  id: number;
  name: string;
  password_set: boolean;
  port: number;
  security: string;
  username_mask?: string | null;
};

type SaveSmtpConfigPayload = {
  enabled: boolean;
  from_email: string;
  from_name?: string;
  host: string;
  password?: string;
  port: number;
  reason: string;
  security: string;
  username?: string;
};

type ConfigForm = typeof defaultConfigForm;

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function formFromConfig(config: SmtpConfig | null): ConfigForm {
  if (!config) {
    return defaultConfigForm;
  }

  return {
    enabled: config.enabled,
    fromEmail: config.from_email,
    fromName: config.from_name ?? '',
    host: config.host,
    password: '',
    port: String(config.port),
    security: config.security,
    username: ''
  };
}

function payloadFromForm(form: ConfigForm, reason: string): SaveSmtpConfigPayload {
  const payload: SaveSmtpConfigPayload = {
    enabled: form.enabled,
    from_email: form.fromEmail.trim(),
    host: form.host.trim(),
    port: Number.parseInt(form.port, 10) || 0,
    reason,
    security: form.security
  };
  const fromName = form.fromName.trim();
  const username = form.username.trim();
  const password = form.password.trim();
  if (fromName) {
    payload.from_name = fromName;
  }
  if (username) {
    payload.username = username;
  }
  if (password) {
    payload.password = password;
  }
  return payload;
}

async function submitAction(label: string, request: () => Promise<unknown>) {
  try {
    await request();
    Toast.success(`${label}已提交`);
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

export function SmtpConfigPage() {
  const [config, setConfig] = useState<SmtpConfig | null>(null);
  const [configForm, setConfigForm] = useState<ConfigForm>(defaultConfigForm);
  const [loading, setLoading] = useState(true);
  const [testRecipient, setTestRecipient] = useState('');
  const [lastTestRecipient, setLastTestRecipient] = useState<string | null>(null);

  async function loadConfig() {
    setLoading(true);
    try {
      const saved = await apiRequest<SmtpConfig | null>('/admin/api/v1/smtp/config');
      setConfig(saved);
      setConfigForm(formFromConfig(saved));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadConfig().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="SMTP 邮件配置" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>发信配置</Title>
            <div className="admin-action-form">
              <label>
                SMTP host
                <AdminTextInput ariaLabel="SMTP host" value={configForm.host} onChange={(host) => setConfigForm({ ...configForm, host })} />
              </label>
              <label>
                SMTP port
                <AdminTextInput ariaLabel="SMTP port" type="number" value={configForm.port} onChange={(port) => setConfigForm({ ...configForm, port })} />
              </label>
              <label>
                加密方式
                <AdminSelect
                  ariaLabel="加密方式"
                  onChange={(security) => setConfigForm({ ...configForm, security })}
                  optionList={securityOptions}
                  value={configForm.security}
                />
              </label>
              <label>
                发件邮箱
                <AdminTextInput ariaLabel="发件邮箱" value={configForm.fromEmail} onChange={(fromEmail) => setConfigForm({ ...configForm, fromEmail })} />
              </label>
              <label>
                发件名称
                <AdminTextInput ariaLabel="发件名称" value={configForm.fromName} onChange={(fromName) => setConfigForm({ ...configForm, fromName })} />
              </label>
              <label>
                SMTP 用户名
                <AdminTextInput ariaLabel="SMTP 用户名" value={configForm.username} onChange={(username) => setConfigForm({ ...configForm, username })} />
              </label>
              <label>
                SMTP 密码
                <AdminPasswordInput ariaLabel="SMTP 密码" value={configForm.password} onChange={(password) => setConfigForm({ ...configForm, password })} />
              </label>
              <div className="admin-action-checkbox">
                <AdminCheckbox checked={configForm.enabled} onChange={(enabled) => setConfigForm({ ...configForm, enabled })}>
                  启用 SMTP
                </AdminCheckbox>
              </div>
            </div>
            <Space>
              <ConfirmAction
                actionText="保存配置"
                title="确认保存 SMTP 配置"
                onConfirm={(reason) =>
                  submitAction('保存 SMTP 配置', async () => {
                    const saved = await apiRequest<SmtpConfig>('/admin/api/v1/smtp/config', {
                      method: 'PATCH',
                      body: JSON.stringify(payloadFromForm(configForm, reason))
                    });
                    setConfig(saved);
                    setConfigForm(formFromConfig(saved));
                  })
                }
              />
              <Button loading={loading} onClick={() => loadConfig().catch((error) => Toast.error(errorMessage(error)))}>
                刷新配置
              </Button>
            </Space>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>配置状态</Title>
            <div className="admin-action-summary">
              <span>配置名称：{config?.name ?? '-'}</span>
              <span>启用状态：<StatusTag value={config?.enabled ?? false} /></span>
              <span>当前用户名：{config?.username_mask ?? '-'}</span>
              <span>SMTP 密码：{config?.password_set ? '已设置' : '未设置'}</span>
              <span>发件邮箱：{config?.from_email ?? '-'}</span>
              <span>加密方式：{config?.security ?? '-'}</span>
            </div>
            <Text type="secondary">密码输入框留空时保留已保存密码。</Text>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>测试发送</Title>
            <div className="admin-action-form">
              <label>
                测试收件邮箱
                <AdminTextInput ariaLabel="测试收件邮箱" value={testRecipient} onChange={setTestRecipient} />
              </label>
            </div>
            <ConfirmAction
              actionText="测试发送"
              title="确认发送 SMTP 测试邮件"
              onConfirm={(reason) =>
                submitAction('SMTP 测试邮件', async () => {
                  const response = await apiRequest<{ recipient: string; sent: boolean }>('/admin/api/v1/smtp/test', {
                    method: 'POST',
                    body: JSON.stringify({ recipient: testRecipient.trim(), reason })
                  });
                  setLastTestRecipient(response.recipient);
                })
              }
            />
            {lastTestRecipient ? <Text type="secondary">最近测试收件邮箱：{lastTestRecipient}</Text> : null}
          </Space>
        </Card>
      </div>
    </main>
  );
}
