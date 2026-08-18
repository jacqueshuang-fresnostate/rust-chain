import { Button, Card, Space, Typography } from '@douyinfe/semi-ui';

import { QuillRichTextEditor } from '../../../shared/QuillRichTextEditor';
import { AdminCheckbox, AdminSelect, AdminTextInput } from '../../../shared/SemiFormControls';
import {
  addTemplate,
  removeTemplate,
  templatePurposeOptions,
  updateTemplate,
  updateTemplatePurpose
} from './model';
import type { ConfigForm } from './types';

const { Title } = Typography;

export function VerificationTemplatesEditor({
  form,
  mode,
  onChange
}: {
  form: ConfigForm;
  mode: 'create' | 'edit';
  onChange: (form: ConfigForm) => void;
}) {
  const ariaPrefix = mode === 'create' ? '新增' : '';

  return (
    <section className="admin-action-panel">
      <div className="admin-earn-section-header">
        <Title heading={4}>验证码 HTML 模板</Title>
        <Button onClick={() => onChange(addTemplate(form))} theme="borderless">
          新增模板
        </Button>
      </div>
      <div className="admin-earn-introduction-list">
        {form.verificationCodeTemplates.map((template, index) => (
          <Card bordered className="admin-earn-introduction-card" key={`${template.key}-${index}`}>
            <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
              <Title heading={5}>邮件模板 {index + 1}</Title>
              <div className="admin-action-form admin-action-form-wide">
                <label>
                  模板名称
                  <AdminTextInput
                    ariaLabel={`${ariaPrefix}模板名称 ${index + 1}`}
                    value={template.name}
                    onChange={(name) => onChange(updateTemplate(form, index, { name }))}
                  />
                </label>
                <label>
                  模板用途
                  <AdminSelect
                    ariaLabel={`${ariaPrefix}模板用途 ${index + 1}`}
                    onChange={(purpose) => onChange(updateTemplatePurpose(form, index, purpose))}
                    optionList={templatePurposeOptions}
                    value={template.purpose}
                  />
                </label>
                <div className="admin-action-checkbox">
                  <AdminCheckbox
                    checked={template.enabled}
                    onChange={(enabled) => onChange(updateTemplate(form, index, { enabled }))}
                  >
                    启用模板
                  </AdminCheckbox>
                </div>
              </div>
              <QuillRichTextEditor
                ariaLabel={`${ariaPrefix}验证码 HTML 模板 ${index + 1}`}
                placeholder="请输入验证码邮件内容，可使用 {{subject}}、{{code}}、{{expires_minutes}}"
                value={template.content}
                onChange={(content) => onChange(updateTemplate(form, index, { content }))}
              />
              <Button
                disabled={form.verificationCodeTemplates.length === 1}
                onClick={() => onChange(removeTemplate(form, index))}
                theme="borderless"
              >
                删除模板
              </Button>
            </Space>
          </Card>
        ))}
      </div>
    </section>
  );
}
