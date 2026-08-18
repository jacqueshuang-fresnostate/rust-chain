import type { RichTextLeaf, RichTextTextBlock, RichTextValue } from '../../../shared/QuillRichTextEditor';
import type {
  ConfigForm,
  SaveSmtpConfigPayload,
  SmtpConfig,
  VerificationCodeTemplateDto,
  VerificationTemplateForm
} from './types';

export const securityOptions = [
  { value: 'none', label: '不加密' },
  { value: 'starttls', label: 'STARTTLS 加密' },
  { value: 'tls', label: 'TLS/SSL 加密' }
];

export const templatePurposeOptions = [
  { value: 'default', label: '通用验证码' },
  { value: 'bind', label: '绑定邮箱' },
  { value: 'two_factor_reset', label: '重置双因素认证' },
  { value: 'login_2fa_reset', label: '重置登录双因素认证' },
  { value: 'fund_password_reset', label: '重置资金密码' }
];

export const deliveryStrategyOptions = [
  { value: 'priority', label: '按优先级发送' },
  { value: 'round_robin', label: '轮询发送' }
];

export const smtpModuleTabs = [
  { itemKey: 'configs', tab: '发信配置' },
  { itemKey: 'templates', tab: '验证码模板' },
  { itemKey: 'strategy', tab: '发信策略' },
  { itemKey: 'test', tab: '测试发送' }
];

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

export function cloneRichTextValue(value: RichTextValue): RichTextValue {
  return value.map((block) =>
    block.type === 'image'
      ? { ...block }
      : { ...block, children: block.children.map((leaf) => ({ ...leaf })) }
  );
}

export function createDefaultTemplate(): VerificationTemplateForm {
  return {
    content: cloneRichTextValue(defaultTemplateContent),
    enabled: true,
    key: 'default',
    name: '通用验证码模板',
    purpose: 'default'
  };
}

export function createDefaultConfigForm(name = '默认发信配置'): ConfigForm {
  return {
    enabled: false,
    fromEmail: '',
    fromName: '',
    host: '',
    name,
    password: '',
    port: '587',
    priority: '100',
    security: 'starttls',
    username: '',
    verificationCodeTemplates: [createDefaultTemplate()]
  };
}

export function createNewConfigForm(configCount: number): ConfigForm {
  return createDefaultConfigForm(`发信配置 ${configCount + 1}`);
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function leafToHtml(leaf: RichTextLeaf): string {
  let html = escapeHtml(leaf.text);
  if (leaf.bold) html = `<strong>${html}</strong>`;
  if (leaf.italic) html = `<em>${html}</em>`;
  if (leaf.underline) html = `<u>${html}</u>`;
  return html;
}

export function richTextValueToHtml(value: RichTextValue): string {
  return value
    .map((block) => {
      if (block.type === 'image') return '';
      const tag: RichTextTextBlock['type'] = block.type;
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
  if (node.nodeType === Node.TEXT_NODE) return [{ ...marks, text: node.textContent ?? '' }];
  if (!(node instanceof HTMLElement)) return [];

  const tag = node.tagName.toLowerCase();
  const nextMarks = {
    ...marks,
    ...(tag === 'strong' || tag === 'b' ? { bold: true } : {}),
    ...(tag === 'em' || tag === 'i' ? { italic: true } : {}),
    ...(tag === 'u' ? { underline: true } : {})
  };
  if (tag === 'br') return [{ ...marks, text: '\n' }];
  return Array.from(node.childNodes).flatMap((child) => collectInlineLeaves(child, nextMarks));
}

function blockTypeFromElement(element: Element): RichTextTextBlock['type'] {
  const tag = element.tagName.toLowerCase();
  return tag === 'h1' || tag === 'h2' || tag === 'h3' || tag === 'blockquote' ? tag : 'p';
}

export function htmlToRichTextValue(html: string): RichTextValue {
  const trimmed = html.trim();
  if (!trimmed) return cloneRichTextValue(defaultTemplateContent);

  const documentValue = new DOMParser().parseFromString(trimmed, 'text/html');
  const elements = Array.from(documentValue.body.children);
  if (elements.length === 0) return plainTextToRichTextValue(documentValue.body.textContent ?? trimmed);

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
  return html ? [{ ...createDefaultTemplate(), content: htmlToRichTextValue(html) }] : [];
}

export function formFromConfig(config: SmtpConfig | null): ConfigForm {
  if (!config) return createDefaultConfigForm();
  const templates = config.verification_code_templates?.length
    ? config.verification_code_templates.map(templateFormFromDto)
    : legacyTemplateFromConfig(config);

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

export function payloadFromForm(form: ConfigForm, reason: string): SaveSmtpConfigPayload {
  const templates = form.verificationCodeTemplates.map(templateDtoFromForm);
  const legacyTemplate =
    templates.find((template) => template.purpose === null && template.enabled)?.html ??
    templates[0]?.html ??
    null;
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
  if (fromName) payload.from_name = fromName;
  if (username) payload.username = username;
  if (password) payload.password = password;
  return payload;
}

function nextTemplatePurpose(form: ConfigForm): string {
  const usedPurposes = new Set(form.verificationCodeTemplates.map((template) => template.purpose));
  return templatePurposeOptions.find((option) => !usedPurposes.has(option.value))?.value ?? 'bind';
}

function nextTemplateKey(form: ConfigForm, purpose: string): string {
  const keys = new Set(form.verificationCodeTemplates.map((template) => template.key));
  let key = purpose;
  let index = 2;
  while (keys.has(key)) {
    key = `${purpose}-${index}`;
    index += 1;
  }
  return key;
}

export function addTemplate(form: ConfigForm): ConfigForm {
  const purpose = nextTemplatePurpose(form);
  const label = templatePurposeOptions.find((option) => option.value === purpose)?.label ?? '验证码';
  return {
    ...form,
    verificationCodeTemplates: [
      ...form.verificationCodeTemplates,
      {
        content: cloneRichTextValue(defaultTemplateContent),
        enabled: true,
        key: nextTemplateKey(form, purpose),
        name: `${label}模板`,
        purpose
      }
    ]
  };
}

export function updateTemplate(
  form: ConfigForm,
  index: number,
  patch: Partial<VerificationTemplateForm>
): ConfigForm {
  return {
    ...form,
    verificationCodeTemplates: form.verificationCodeTemplates.map((template, templateIndex) =>
      templateIndex === index ? { ...template, ...patch } : template
    )
  };
}

export function updateTemplatePurpose(form: ConfigForm, index: number, purpose: string): ConfigForm {
  const template = form.verificationCodeTemplates[index];
  if (!template) return form;
  return updateTemplate(form, index, {
    purpose,
    key: template.key === template.purpose ? nextTemplateKey(form, purpose) : template.key
  });
}

export function removeTemplate(form: ConfigForm, index: number): ConfigForm {
  if (form.verificationCodeTemplates.length <= 1) return form;
  return {
    ...form,
    verificationCodeTemplates: form.verificationCodeTemplates.filter(
      (_template, templateIndex) => templateIndex !== index
    )
  };
}

export function legacyCompatibilityDescription(
  loading: boolean,
  legacyReadUnavailable: boolean,
  legacyConfig: SmtpConfig | null,
  configs: SmtpConfig[]
): { description: string; warning: boolean } {
  const included = Boolean(legacyConfig && configs.some((config) => config.id === legacyConfig.id));
  if (loading) {
    return { description: '正在读取旧版默认单例的兼容状态。', warning: false };
  }
  if (legacyReadUnavailable) {
    return {
      description: '具名配置仍可正常管理；旧版默认单例状态读取失败，请核对迁移后仅维护具名配置。',
      warning: true
    };
  }
  if (!legacyConfig) {
    return {
      description: '未检测到旧版默认单例，无需迁移。后续请仅维护具名发信配置，旧接口仅保留兼容读取。',
      warning: false
    };
  }
  if (included) {
    return {
      description: `旧版默认单例“${legacyConfig.name}”（ID ${legacyConfig.id}）已包含在具名配置列表中。本页仅通过 /smtp/configs 与 /smtp/configs/:id 保存，旧 /smtp/config 仅作兼容读取。`,
      warning: false
    };
  }
  return {
    description: `检测到旧版默认单例“${legacyConfig.name}”（ID ${legacyConfig.id}）尚未出现在具名列表。请新建具名配置完成迁移，凭据需重新输入；本页不向旧单例写入。`,
    warning: true
  };
}
