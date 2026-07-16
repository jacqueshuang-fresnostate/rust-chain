import { Button, Card, SideSheet, Space, Table, Tabs, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { QuillRichTextEditor, type RichTextBlock, type RichTextLeaf, type RichTextTextBlock, type RichTextValue } from '../../shared/QuillRichTextEditor';
import { AdminCheckbox, AdminPasswordInput, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import { StatusTag } from '../../shared/StatusTag';

const { Text, Title } = Typography;

type VerificationTemplateForm = {
  content: RichTextValue;
  enabled: boolean;
  key: string;
  name: string;
  purpose: string;
};

type ConfigForm = {
  enabled: boolean;
  fromEmail: string;
  fromName: string;
  host: string;
  name: string;
  password: string;
  port: string;
  priority: string;
  security: string;
  username: string;
  verificationCodeTemplates: VerificationTemplateForm[];
};

const defaultTemplateContent: RichTextValue = [
  {
    type: 'p',
    children: [
      { text: '您的{{subject}}是 ' },
      { text: '{{code}}', bold: true },
      { text: '，{{expires_minutes}} 分钟内有效。' }
    ]
  }
];

function cloneRichTextValue(value: RichTextValue): RichTextValue {
  return value.map((block) => (block.type === 'image' ? { ...block } : { ...block, children: block.children.map((leaf) => ({ ...leaf })) }));
}

function createDefaultTemplate(): VerificationTemplateForm {
  return {
    content: cloneRichTextValue(defaultTemplateContent),
    enabled: true,
    key: 'default',
    name: '通用验证码模板',
    purpose: 'default'
  };
}

function createDefaultConfigForm(): ConfigForm {
  return {
    enabled: false,
    fromEmail: '',
    fromName: '',
    host: '',
    name: '默认发信配置',
    password: '',
    port: '587',
    priority: '100',
    security: 'starttls',
    username: '',
    verificationCodeTemplates: [createDefaultTemplate()]
  };
}

const securityOptions = [
  { value: 'none', label: '不加密' },
  { value: 'starttls', label: 'STARTTLS 加密' },
  { value: 'tls', label: 'TLS/SSL 加密' }
];

const templatePurposeOptions = [
  { value: 'default', label: '通用验证码' },
  { value: 'bind', label: '绑定邮箱' },
  { value: 'two_factor_reset', label: '重置双因素认证' },
  { value: 'login_2fa_reset', label: '重置登录双因素认证' },
  { value: 'fund_password_reset', label: '重置资金密码' }
];

const deliveryStrategyOptions = [
  { value: 'priority', label: '按优先级发送' },
  { value: 'round_robin', label: '轮询发送' }
];

type SmtpModuleTab = 'configs' | 'templates' | 'strategy' | 'test';

const smtpModuleTabs = [
  { itemKey: 'configs', tab: '发信配置' },
  { itemKey: 'templates', tab: '验证码模板' },
  { itemKey: 'strategy', tab: '发信策略' },
  { itemKey: 'test', tab: '测试发送' }
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
  priority: number;
  security: string;
  username_mask?: string | null;
  verification_code_template_html?: string | null;
  verification_code_templates?: VerificationCodeTemplateDto[] | null;
};

type SmtpDeliverySettings = {
  strategy: string;
};

type SmtpConfigListResponse = {
  configs: SmtpConfig[];
  delivery_settings: SmtpDeliverySettings;
};

type VerificationCodeTemplateDto = {
  enabled: boolean;
  html: string;
  key: string;
  name: string;
  purpose?: string | null;
};

type SaveSmtpConfigPayload = {
  enabled: boolean;
  from_email: string;
  from_name?: string;
  host: string;
  name: string;
  password?: string;
  port: number;
  priority: number;
  reason: string;
  security: string;
  username?: string;
  verification_code_template_html: string | null;
  verification_code_templates: VerificationCodeTemplateDto[];
};

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

function leafToHtml(leaf: RichTextLeaf): string {
  let html = escapeHtml(leaf.text);
  if (leaf.bold) html = `<strong>${html}</strong>`;
  if (leaf.italic) html = `<em>${html}</em>`;
  if (leaf.underline) html = `<u>${html}</u>`;
  return html;
}

function blockTag(type: RichTextTextBlock['type']): 'blockquote' | 'h1' | 'h2' | 'h3' | 'p' {
  return type;
}

function richTextValueToHtml(value: RichTextValue): string {
  return value
    .map((block) => {
      if (block.type === 'image') {
        return '';
      }
      const tag = blockTag(block.type);
      const content = block.children.map(leafToHtml).join('') || '<br>';
      return `<${tag}>${content}</${tag}>`;
    })
    .join('');
}

function plainTextToRichTextValue(text: string): RichTextValue {
  return text
    .replace(/\r\n/g, '\n')
    .split('\n')
    .map((line) => ({ type: 'p', children: [{ text: line }] }));
}

function collectInlineLeaves(node: Node, marks: Omit<RichTextLeaf, 'text'> = {}): RichTextLeaf[] {
  if (node.nodeType === Node.TEXT_NODE) {
    return [{ ...marks, text: node.textContent ?? '' }];
  }
  if (!(node instanceof HTMLElement)) {
    return [];
  }
  const tag = node.tagName.toLowerCase();
  const nextMarks = {
    ...marks,
    ...(tag === 'strong' || tag === 'b' ? { bold: true } : {}),
    ...(tag === 'em' || tag === 'i' ? { italic: true } : {}),
    ...(tag === 'u' ? { underline: true } : {})
  };
  if (tag === 'br') {
    return [{ ...marks, text: '\n' }];
  }
  return Array.from(node.childNodes).flatMap((child) => collectInlineLeaves(child, nextMarks));
}

function blockTypeFromElement(element: Element): RichTextTextBlock['type'] {
  const tag = element.tagName.toLowerCase();
  if (tag === 'h1' || tag === 'h2' || tag === 'h3' || tag === 'blockquote') {
    return tag;
  }
  return 'p';
}

function htmlToRichTextValue(html: string): RichTextValue {
  const trimmed = html.trim();
  if (!trimmed) {
    return cloneRichTextValue(defaultTemplateContent);
  }
  const doc = new DOMParser().parseFromString(trimmed, 'text/html');
  const elements = Array.from(doc.body.children);
  if (elements.length === 0) {
    return plainTextToRichTextValue(doc.body.textContent ?? trimmed);
  }
  return elements.map((element) => {
    const children = collectInlineLeaves(element).filter((leaf) => leaf.text.length > 0);
    return {
      type: blockTypeFromElement(element),
      children: children.length > 0 ? children : [{ text: '' }]
    };
  });
}

function templateFormFromDto(template: VerificationCodeTemplateDto): VerificationTemplateForm {
  return {
    content: htmlToRichTextValue(template.html),
    enabled: template.enabled,
    key: template.key,
    name: template.name,
    purpose: template.purpose ?? 'default'
  };
}

function templateDtoFromForm(template: VerificationTemplateForm): VerificationCodeTemplateDto {
  return {
    enabled: template.enabled,
    html: richTextValueToHtml(template.content),
    key: template.key.trim(),
    name: template.name.trim(),
    purpose: template.purpose === 'default' ? null : template.purpose
  };
}

function legacyTemplateFromConfig(config: SmtpConfig): VerificationTemplateForm[] {
  const html = config.verification_code_template_html?.trim();
  return html
    ? [
        {
          ...createDefaultTemplate(),
          content: htmlToRichTextValue(html)
        }
      ]
    : [];
}

function formFromConfig(config: SmtpConfig | null): ConfigForm {
  if (!config) {
    return createDefaultConfigForm();
  }
  const templates = config.verification_code_templates?.length ? config.verification_code_templates.map(templateFormFromDto) : legacyTemplateFromConfig(config);

  return {
    enabled: config.enabled,
    fromEmail: config.from_email,
    fromName: config.from_name ?? '',
    host: config.host,
    name: config.name,
    password: '',
    port: String(config.port),
    priority: String(config.priority),
    security: config.security,
    username: '',
    verificationCodeTemplates: templates.length > 0 ? templates : [createDefaultTemplate()]
  };
}

function payloadFromForm(form: ConfigForm, reason: string): SaveSmtpConfigPayload {
  const templates = form.verificationCodeTemplates.map(templateDtoFromForm);
  const legacyTemplate = templates.find((template) => template.purpose === null && template.enabled)?.html ?? templates[0]?.html ?? null;
  const payload: SaveSmtpConfigPayload = {
    enabled: form.enabled,
    from_email: form.fromEmail.trim(),
    host: form.host.trim(),
    name: form.name.trim(),
    port: Number.parseInt(form.port, 10) || 0,
    priority: Number.parseInt(form.priority, 10) || 0,
    reason,
    security: form.security,
    verification_code_template_html: legacyTemplate,
    verification_code_templates: templates
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
  const [activeTab, setActiveTab] = useState<SmtpModuleTab>('configs');
  const [configs, setConfigs] = useState<SmtpConfig[]>([]);
  const [configForm, setConfigForm] = useState<ConfigForm>(() => createDefaultConfigForm());
  const [createConfigForm, setCreateConfigForm] = useState<ConfigForm>(() => createDefaultConfigForm());
  const [createSheetVisible, setCreateSheetVisible] = useState(false);
  const [deliveryStrategy, setDeliveryStrategy] = useState('priority');
  const [loading, setLoading] = useState(true);
  const [selectedConfigId, setSelectedConfigId] = useState('');
  const [testConfigChoice, setTestConfigChoice] = useState('strategy');
  const [testRecipient, setTestRecipient] = useState('');
  const [lastTestRecipient, setLastTestRecipient] = useState<string | null>(null);

  function createNewConfigForm(configCount = configs.length): ConfigForm {
    return {
      ...createDefaultConfigForm(),
      name: `发信配置 ${configCount + 1}`
    };
  }

  async function loadConfig(preferredConfigId?: number | 'new') {
    setLoading(true);
    try {
      const saved = await apiRequest<SmtpConfigListResponse>('/admin/api/v1/smtp/configs');
      const nextConfigs = saved.configs ?? [];
      const nextStrategy = saved.delivery_settings?.strategy || 'priority';
      const preferredId = preferredConfigId === undefined ? selectedConfigId : String(preferredConfigId);
      const nextSelected = nextConfigs.find((item) => String(item.id) === preferredId);
      const fallbackSelected = nextSelected ?? nextConfigs[0] ?? null;
      setConfigs(nextConfigs);
      setDeliveryStrategy(nextStrategy);
      setSelectedConfigId(fallbackSelected ? String(fallbackSelected.id) : '');
      setConfigForm(fallbackSelected ? formFromConfig(fallbackSelected) : createNewConfigForm(nextConfigs.length));
      setTestConfigChoice((current) => (current === 'strategy' || nextConfigs.some((item) => String(item.id) === current) ? current : 'strategy'));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadConfig().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  function selectConfig(config: SmtpConfig) {
    setSelectedConfigId(String(config.id));
    setConfigForm(formFromConfig(config));
  }

  function startCreateConfig() {
    setCreateConfigForm(createNewConfigForm());
    setCreateSheetVisible(true);
  }

  async function saveCurrentConfig(reason: string) {
    if (!selectedConfigId) {
      throw new Error('请先选择发信配置');
    }
    const payload = payloadFromForm(configForm, reason);
    const saved = await apiRequest<SmtpConfig>(`/admin/api/v1/smtp/configs/${selectedConfigId}`, {
      method: 'PATCH',
      body: JSON.stringify(payload)
    });
    await loadConfig(saved.id);
  }

  async function createConfig(reason: string) {
    const payload = payloadFromForm(createConfigForm, reason);
    const saved = await apiRequest<SmtpConfig>('/admin/api/v1/smtp/configs', {
      method: 'POST',
      body: JSON.stringify(payload)
    });
    setCreateSheetVisible(false);
    setCreateConfigForm(createNewConfigForm(configs.length + 1));
    await loadConfig(saved.id);
  }

  async function toggleConfigEnabled(config: SmtpConfig, enabled: boolean, reason: string) {
    const payload = payloadFromForm({ ...formFromConfig(config), enabled }, reason);
    const saved = await apiRequest<SmtpConfig>(`/admin/api/v1/smtp/configs/${config.id}`, {
      method: 'PATCH',
      body: JSON.stringify(payload)
    });
    await loadConfig(saved.id);
  }

  async function saveDeliverySettings(reason: string) {
    const saved = await apiRequest<SmtpDeliverySettings>('/admin/api/v1/smtp/delivery-settings', {
      method: 'PATCH',
      body: JSON.stringify({ strategy: deliveryStrategy, reason })
    });
    setDeliveryStrategy(saved.strategy);
  }

  function updateTemplate(index: number, patch: Partial<VerificationTemplateForm>) {
    setConfigForm((current) => ({
      ...current,
      verificationCodeTemplates: current.verificationCodeTemplates.map((template, templateIndex) => (templateIndex === index ? { ...template, ...patch } : template))
    }));
  }

  function nextTemplatePurpose(): string {
    const usedPurposes = new Set(configForm.verificationCodeTemplates.map((template) => template.purpose));
    return templatePurposeOptions.find((option) => !usedPurposes.has(option.value))?.value ?? 'bind';
  }

  function nextTemplateKey(purpose: string): string {
    const keys = new Set(configForm.verificationCodeTemplates.map((template) => template.key));
    let key = purpose;
    let index = 2;
    while (keys.has(key)) {
      key = `${purpose}-${index}`;
      index += 1;
    }
    return key;
  }

  function addTemplate() {
    const purpose = nextTemplatePurpose();
    const label = templatePurposeOptions.find((option) => option.value === purpose)?.label ?? '验证码';
    setConfigForm((current) => ({
      ...current,
      verificationCodeTemplates: [
        ...current.verificationCodeTemplates,
        {
          content: cloneRichTextValue(defaultTemplateContent),
          enabled: true,
          key: nextTemplateKey(purpose),
          name: `${label}模板`,
          purpose
        }
      ]
    }));
  }

  function removeTemplate(index: number) {
    setConfigForm((current) => ({
      ...current,
      verificationCodeTemplates: current.verificationCodeTemplates.length > 1 ? current.verificationCodeTemplates.filter((_, templateIndex) => templateIndex !== index) : current.verificationCodeTemplates
    }));
  }

  function updateCreateTemplate(index: number, patch: Partial<VerificationTemplateForm>) {
    setCreateConfigForm((current) => ({
      ...current,
      verificationCodeTemplates: current.verificationCodeTemplates.map((template, templateIndex) => (templateIndex === index ? { ...template, ...patch } : template))
    }));
  }

  function nextCreateTemplatePurpose(): string {
    const usedPurposes = new Set(createConfigForm.verificationCodeTemplates.map((template) => template.purpose));
    return templatePurposeOptions.find((option) => !usedPurposes.has(option.value))?.value ?? 'bind';
  }

  function nextCreateTemplateKey(purpose: string): string {
    const keys = new Set(createConfigForm.verificationCodeTemplates.map((template) => template.key));
    let key = purpose;
    let index = 2;
    while (keys.has(key)) {
      key = `${purpose}-${index}`;
      index += 1;
    }
    return key;
  }

  function addCreateTemplate() {
    const purpose = nextCreateTemplatePurpose();
    const label = templatePurposeOptions.find((option) => option.value === purpose)?.label ?? '验证码';
    setCreateConfigForm((current) => ({
      ...current,
      verificationCodeTemplates: [
        ...current.verificationCodeTemplates,
        {
          content: cloneRichTextValue(defaultTemplateContent),
          enabled: true,
          key: nextCreateTemplateKey(purpose),
          name: `${label}模板`,
          purpose
        }
      ]
    }));
  }

  function removeCreateTemplate(index: number) {
    setCreateConfigForm((current) => ({
      ...current,
      verificationCodeTemplates: current.verificationCodeTemplates.length > 1 ? current.verificationCodeTemplates.filter((_, templateIndex) => templateIndex !== index) : current.verificationCodeTemplates
    }));
  }

  function renderConfigFields(form: ConfigForm, setForm: (form: ConfigForm) => void) {
    return (
      <div className="admin-action-form">
        <label>
          配置名称
          <AdminTextInput ariaLabel="配置名称" value={form.name} onChange={(name) => setForm({ ...form, name })} />
        </label>
        <label>
          优先级
          <AdminTextInput ariaLabel="优先级" type="number" value={form.priority} onChange={(priority) => setForm({ ...form, priority })} />
        </label>
        <label>
          SMTP host
          <AdminTextInput ariaLabel="SMTP host" value={form.host} onChange={(host) => setForm({ ...form, host })} />
        </label>
        <label>
          SMTP port
          <AdminTextInput ariaLabel="SMTP port" type="number" value={form.port} onChange={(port) => setForm({ ...form, port })} />
        </label>
        <label>
          加密方式
          <AdminSelect ariaLabel="加密方式" onChange={(security) => setForm({ ...form, security })} optionList={securityOptions} value={form.security} />
        </label>
        <label>
          发件邮箱
          <AdminTextInput ariaLabel="发件邮箱" value={form.fromEmail} onChange={(fromEmail) => setForm({ ...form, fromEmail })} />
        </label>
        <label>
          发件名称
          <AdminTextInput ariaLabel="发件名称" value={form.fromName} onChange={(fromName) => setForm({ ...form, fromName })} />
        </label>
        <label>
          SMTP 用户名
          <AdminTextInput ariaLabel="SMTP 用户名" value={form.username} onChange={(username) => setForm({ ...form, username })} />
        </label>
        <label>
          SMTP 密码
          <AdminPasswordInput ariaLabel="SMTP 密码" value={form.password} onChange={(password) => setForm({ ...form, password })} />
        </label>
        <div className="admin-action-checkbox">
          <AdminCheckbox checked={form.enabled} onChange={(enabled) => setForm({ ...form, enabled })}>
            启用 SMTP
          </AdminCheckbox>
        </div>
      </div>
    );
  }

  const configColumns = [
    {
      dataIndex: 'name',
      key: 'name',
      title: '配置名称',
      render: (_value: unknown, record: SmtpConfig) => (
        <Space spacing={8}>
          <Text strong>{record.name}</Text>
          {String(record.id) === selectedConfigId ? <Text type="tertiary">编辑中</Text> : null}
        </Space>
      )
    },
    {
      dataIndex: 'host',
      key: 'host',
      title: 'SMTP host'
    },
    {
      dataIndex: 'from_email',
      key: 'from_email',
      title: '发件邮箱'
    },
    {
      dataIndex: 'priority',
      key: 'priority',
      title: '优先级'
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
          <Button onClick={() => selectConfig(record)} theme="borderless">
            编辑
          </Button>
          <ConfirmAction
            actionText={record.enabled ? '停用' : '启用'}
            title={record.enabled ? '确认停用发信配置' : '确认启用发信配置'}
            onConfirm={(reason) => submitAction(record.enabled ? '停用发信配置' : '启用发信配置', () => toggleConfigEnabled(record, !record.enabled, reason))}
          />
        </Space>
      )
    }
  ];
  const testConfigOptions = [
    { value: 'strategy', label: '按当前策略选择' },
    ...configs.map((item) => ({ value: String(item.id), label: `${item.name}${item.enabled ? '' : '（未启用）'}` }))
  ];
  const selectedConfigTitle = selectedConfigId ? '编辑发信配置' : '发信配置';

  function renderConfigActions() {
    return (
      <Space>
        <ConfirmAction
          actionText="保存配置"
          title="确认保存 SMTP 配置"
          onConfirm={(reason) => submitAction('保存 SMTP 配置', () => saveCurrentConfig(reason))}
        />
      </Space>
    );
  }

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="SMTP 邮件配置" />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Tabs
          activeKey={activeTab}
          className="admin-action-tabs"
          onChange={(nextTab) => setActiveTab(nextTab as SmtpModuleTab)}
          tabBarExtraContent={
            <Button loading={loading} onClick={() => loadConfig().catch((error) => Toast.error(errorMessage(error)))} theme="borderless">
              刷新
            </Button>
          }
          tabList={smtpModuleTabs}
          type="button"
        />

        {activeTab === 'configs' ? (
          <div className="admin-action-workbench-grid">
            <section className="admin-action-panel">
              <div className="admin-earn-section-header">
                <Title heading={4}>发信配置列表</Title>
                <Button onClick={startCreateConfig} theme="solid" type="primary">
                  新增配置
                </Button>
              </div>
              <Table columns={configColumns} dataSource={configs} loading={loading} pagination={false} rowKey="id" style={{ width: '100%' }} />
            </section>
            <section className="admin-action-panel">
              <Title heading={4}>{selectedConfigTitle}</Title>
              {selectedConfigId ? (
                <>
                  {renderConfigFields(configForm, setConfigForm)}
                  {renderConfigActions()}
                  <Text type="secondary">密码输入框留空时保留已保存密码。</Text>
                </>
              ) : (
                <Text type="secondary">暂无发信配置</Text>
              )}
            </section>
          </div>
        ) : null}

        {activeTab === 'templates' ? (
          <section className="admin-action-panel">
            <div className="admin-earn-section-header">
              <Space align="start" spacing={4} vertical>
                <Title heading={4}>验证码 HTML 模板</Title>
                <Text type="secondary">{selectedConfigTitle}：{configForm.name}</Text>
              </Space>
              <Button onClick={addTemplate} theme="borderless">
                新增模板
              </Button>
            </div>
            <div className="admin-earn-introduction-list">
              {configForm.verificationCodeTemplates.map((template, index) => (
                <Card bordered className="admin-earn-introduction-card" key={`${template.key}-${index}`}>
                  <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                    <Title heading={5}>邮件模板 {index + 1}</Title>
                    <div className="admin-action-form admin-action-form-wide">
                      <label>
                        模板名称
                        <AdminTextInput ariaLabel={`模板名称 ${index + 1}`} value={template.name} onChange={(name) => updateTemplate(index, { name })} />
                      </label>
                      <label>
                        模板用途
                        <AdminSelect
                          ariaLabel={`模板用途 ${index + 1}`}
                          onChange={(purpose) => updateTemplate(index, { purpose, key: template.key === template.purpose ? nextTemplateKey(purpose) : template.key })}
                          optionList={templatePurposeOptions}
                          value={template.purpose}
                        />
                      </label>
                      <div className="admin-action-checkbox">
                        <AdminCheckbox checked={template.enabled} onChange={(enabled) => updateTemplate(index, { enabled })}>
                          启用模板
                        </AdminCheckbox>
                      </div>
                    </div>
                    <QuillRichTextEditor
                      ariaLabel={`验证码 HTML 模板 ${index + 1}`}
                      placeholder="请输入验证码邮件内容，可使用 {{subject}}、{{code}}、{{expires_minutes}}"
                      value={template.content}
                      onChange={(content) => updateTemplate(index, { content })}
                    />
                    <Button disabled={configForm.verificationCodeTemplates.length === 1} onClick={() => removeTemplate(index)} theme="borderless">
                      删除模板
                    </Button>
                  </Space>
                </Card>
              ))}
            </div>
            {renderConfigActions()}
          </section>
        ) : null}

        {activeTab === 'strategy' ? (
          <section className="admin-action-panel">
            <Title heading={4}>发信策略</Title>
            <div className="admin-action-form admin-action-form-wide">
              <label>
                发送策略
                <AdminSelect ariaLabel="发送策略" onChange={setDeliveryStrategy} optionList={deliveryStrategyOptions} value={deliveryStrategy} />
              </label>
            </div>
            <ConfirmAction actionText="保存策略" title="确认保存 SMTP 发信策略" onConfirm={(reason) => submitAction('保存 SMTP 发信策略', () => saveDeliverySettings(reason))} />
          </section>
        ) : null}

        {activeTab === 'test' ? (
          <section className="admin-action-panel">
            <Title heading={4}>测试发送</Title>
            <div className="admin-action-form">
              <label>
                发信方式
                <AdminSelect ariaLabel="发信方式" onChange={setTestConfigChoice} optionList={testConfigOptions} value={testConfigChoice} />
              </label>
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
                  const body = {
                    recipient: testRecipient.trim(),
                    reason,
                    ...(testConfigChoice === 'strategy' ? {} : { config_id: Number(testConfigChoice) })
                  };
                  const response = await apiRequest<{ config_id: number; config_name: string; recipient: string; sent: boolean }>('/admin/api/v1/smtp/test', {
                    method: 'POST',
                    body: JSON.stringify(body)
                  });
                  setLastTestRecipient(`${response.recipient} / ${response.config_name}`);
                })
              }
            />
            {lastTestRecipient ? <Text type="secondary">最近测试收件邮箱：{lastTestRecipient}</Text> : null}
          </section>
        ) : null}
      </Card>
      {createSheetVisible ? (
        <SideSheet onCancel={() => setCreateSheetVisible(false)} title="新增发信配置" visible={createSheetVisible} width={760}>
          <Card bordered={false}>
            <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
              {renderConfigFields(createConfigForm, setCreateConfigForm)}
              <section className="admin-action-panel">
                <div className="admin-earn-section-header">
                  <Title heading={4}>验证码 HTML 模板</Title>
                  <Button onClick={addCreateTemplate} theme="borderless">
                    新增模板
                  </Button>
                </div>
                <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                  {createConfigForm.verificationCodeTemplates.map((template, index) => (
                    <Card bordered className="admin-earn-introduction-card" key={`${template.key}-${index}`}>
                      <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                        <Title heading={5}>邮件模板 {index + 1}</Title>
                        <div className="admin-action-form admin-action-form-wide">
                          <label>
                            模板名称
                            <AdminTextInput ariaLabel={`新增模板名称 ${index + 1}`} value={template.name} onChange={(name) => updateCreateTemplate(index, { name })} />
                          </label>
                          <label>
                            模板用途
                            <AdminSelect
                              ariaLabel={`新增模板用途 ${index + 1}`}
                              onChange={(purpose) => updateCreateTemplate(index, { purpose, key: template.key === template.purpose ? nextCreateTemplateKey(purpose) : template.key })}
                              optionList={templatePurposeOptions}
                              value={template.purpose}
                            />
                          </label>
                          <div className="admin-action-checkbox">
                            <AdminCheckbox checked={template.enabled} onChange={(enabled) => updateCreateTemplate(index, { enabled })}>
                              启用模板
                            </AdminCheckbox>
                          </div>
                        </div>
                        <QuillRichTextEditor
                          ariaLabel={`新增验证码 HTML 模板 ${index + 1}`}
                          placeholder="请输入验证码邮件内容，可使用 {{subject}}、{{code}}、{{expires_minutes}}"
                          value={template.content}
                          onChange={(content) => updateCreateTemplate(index, { content })}
                        />
                        <Button disabled={createConfigForm.verificationCodeTemplates.length === 1} onClick={() => removeCreateTemplate(index)} theme="borderless">
                          删除模板
                        </Button>
                      </Space>
                    </Card>
                  ))}
                </Space>
              </section>
              <ConfirmAction actionText="新增配置" title="确认新增 SMTP 配置" onConfirm={(reason) => submitAction('新增 SMTP 配置', () => createConfig(reason))} />
            </Space>
          </Card>
        </SideSheet>
      ) : null}
    </main>
  );
}
