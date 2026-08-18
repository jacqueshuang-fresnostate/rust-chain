import { Space, Typography } from '@douyinfe/semi-ui';

import {
  AdminCheckbox,
  AdminPasswordInput,
  AdminSelect,
  AdminTextInput
} from '../../../shared/SemiFormControls';
import { securityOptions } from './model';
import type { ConfigForm, SmtpConfig, SmtpTestResult } from './types';

export function SmtpConfigFields({
  form,
  onChange
}: {
  form: ConfigForm;
  onChange: (form: ConfigForm) => void;
}) {
  return (
    <div className="admin-action-form">
      <label>
        配置名称
        <AdminTextInput ariaLabel="配置名称" value={form.name} onChange={(name) => onChange({ ...form, name })} />
      </label>
      <label>
        优先级
        <AdminTextInput ariaLabel="优先级" type="number" value={form.priority} onChange={(priority) => onChange({ ...form, priority })} />
      </label>
      <label>
        SMTP host
        <AdminTextInput ariaLabel="SMTP host" value={form.host} onChange={(host) => onChange({ ...form, host })} />
      </label>
      <label>
        SMTP port
        <AdminTextInput ariaLabel="SMTP port" type="number" value={form.port} onChange={(port) => onChange({ ...form, port })} />
      </label>
      <label>
        加密方式
        <AdminSelect ariaLabel="加密方式" onChange={(security) => onChange({ ...form, security })} optionList={securityOptions} value={form.security} />
      </label>
      <label>
        发件邮箱
        <AdminTextInput ariaLabel="发件邮箱" value={form.fromEmail} onChange={(fromEmail) => onChange({ ...form, fromEmail })} />
      </label>
      <label>
        发件名称
        <AdminTextInput ariaLabel="发件名称" value={form.fromName} onChange={(fromName) => onChange({ ...form, fromName })} />
      </label>
      <label>
        SMTP 用户名
        <AdminTextInput ariaLabel="SMTP 用户名" value={form.username} onChange={(username) => onChange({ ...form, username })} />
      </label>
      <label>
        SMTP 密码
        <AdminPasswordInput ariaLabel="SMTP 密码" value={form.password} onChange={(password) => onChange({ ...form, password })} />
      </label>
      <div className="admin-action-checkbox">
        <AdminCheckbox checked={form.enabled} onChange={(enabled) => onChange({ ...form, enabled })}>
          启用 SMTP
        </AdminCheckbox>
      </div>
    </div>
  );
}

export function SmtpCredentialMetadata({
  config,
  lastTestResult
}: {
  config: SmtpConfig | null;
  lastTestResult: SmtpTestResult | null;
}) {
  if (!config) return null;
  return (
    <Space aria-label="SMTP 凭据元数据" spacing={4} vertical>
      <Typography.Text type="secondary">用户名：{config.username_mask?.trim() || '未配置'}</Typography.Text>
      <Typography.Text type="secondary">密码：{config.password_set ? '已配置' : '未配置'}</Typography.Text>
      <Typography.Text type="secondary">
        最近测试：{lastTestResult ? `${lastTestResult.recipient} / ${lastTestResult.configName}` : '本会话尚未测试'}
      </Typography.Text>
      <Typography.Text type="secondary">凭据轮换：仅输入新用户名或密码时更新；留空保持后端已有凭据。</Typography.Text>
    </Space>
  );
}
