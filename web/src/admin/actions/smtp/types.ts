import type { RichTextValue } from '../../../shared/QuillRichTextEditor';

export type VerificationTemplateForm = {
  content: RichTextValue;
  enabled: boolean;
  key: string;
  name: string;
  purpose: string;
};

export type ConfigForm = {
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

export type SmtpModuleTab = 'configs' | 'templates' | 'strategy' | 'test';

export type VerificationCodeTemplateDto = {
  enabled: boolean;
  html: string;
  key: string;
  name: string;
  purpose?: string | null;
};

export type SmtpConfig = {
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

export type SmtpDeliverySettings = {
  strategy: string;
};

export type SmtpConfigListResponse = {
  configs: SmtpConfig[];
  delivery_settings: SmtpDeliverySettings;
};

export type SaveSmtpConfigPayload = {
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

export type SmtpTestResult = {
  configName: string;
  recipient: string;
};
