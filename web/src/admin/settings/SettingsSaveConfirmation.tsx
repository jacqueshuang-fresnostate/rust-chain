import { Button, Modal, TextArea, Typography } from '@douyinfe/semi-ui';
import { useId, useState } from 'react';

import type { SettingsDifference, SettingsValidationIssue } from './differences';
import { settingsErrorMessage } from './query';
import './settings.css';

const { Text, Title } = Typography;

type SettingsSaveConfirmationProps = {
  actionText: string;
  differences: SettingsDifference[];
  disabled?: boolean;
  impactSummary: string;
  onConfirm: (reason: string) => Promise<unknown> | unknown;
  riskLevel?: 'high' | 'normal';
  title: string;
  validationIssues?: SettingsValidationIssue[];
};

/** 展示可审计的中文字段差异、影响范围和必填保存原因。 */
export function SettingsSaveConfirmation({
  actionText,
  differences,
  disabled,
  impactSummary,
  onConfirm,
  riskLevel = 'normal',
  title,
  validationIssues = []
}: SettingsSaveConfirmationProps) {
  const [reason, setReason] = useState('');
  const [submissionError, setSubmissionError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [visible, setVisible] = useState(false);
  const differenceTitleId = useId();
  const impactTitleId = useId();
  const highRisk = riskLevel === 'high';
  const triggerDisabled = disabled || differences.length === 0 || validationIssues.length > 0;

  function closeModal() {
    if (submitting) {
      return;
    }
    setVisible(false);
    setReason('');
    setSubmissionError(null);
  }

  async function confirmSave() {
    const trimmedReason = reason.trim();
    if (!trimmedReason || differences.length === 0) {
      return;
    }

    setSubmitting(true);
    setSubmissionError(null);
    try {
      await onConfirm(trimmedReason);
      setVisible(false);
      setReason('');
    } catch (error) {
      setSubmissionError(settingsErrorMessage(error));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <>
      <div className="admin-settings-save-trigger" data-risk-level={riskLevel}>
        <Button
          disabled={triggerDisabled}
          onClick={() => setVisible(true)}
          theme="solid"
          type={highRisk ? 'danger' : 'primary'}
        >
          {actionText}
        </Button>
        {validationIssues.length > 0 ? (
          <ul aria-label="配置校验错误" className="admin-settings-validation-errors">
            {validationIssues.map((issue) => (
              <li key={issue.key}><strong>{issue.field}：</strong>{issue.message}</li>
            ))}
          </ul>
        ) : null}
      </div>
      <Modal
        cancelButtonProps={{ 'aria-label': '取消保存', disabled: submitting }}
        cancelText="取消"
        closeOnEsc={!submitting}
        confirmLoading={submitting}
        maskClosable={false}
        motion={false}
        okButtonProps={{
          'aria-label': '确认保存',
          disabled: reason.trim().length === 0 || differences.length === 0 || validationIssues.length > 0,
          type: highRisk ? 'danger' : 'primary'
        }}
        okText="确认保存"
        onCancel={closeModal}
        onOk={confirmSave}
        title={title}
        visible={visible}
      >
        <div className="admin-settings-confirmation" data-risk-level={riskLevel}>
          <section aria-labelledby={differenceTitleId}>
            <Title heading={6} id={differenceTitleId}>字段差异（{differences.length} 项）</Title>
            <dl className="admin-settings-difference-list">
              {differences.map((difference) => (
                <div className="admin-settings-difference-row" key={difference.key}>
                  <dt>{difference.field}</dt>
                  <dd>
                    <span><Text type="tertiary">当前：</Text>{difference.before}</span>
                    <span aria-hidden="true" className="admin-settings-difference-arrow">→</span>
                    <span><Text type="tertiary">保存后：</Text>{difference.after}</span>
                  </dd>
                </div>
              ))}
            </dl>
          </section>

          <section
            aria-labelledby={impactTitleId}
            className="admin-settings-impact"
            data-risk-level={riskLevel}
          >
            <Title heading={6} id={impactTitleId}>影响摘要</Title>
            <Text>{impactSummary}</Text>
            {highRisk ? <Text strong>这是高风险配置变更，保存前请再次核对影响范围。</Text> : null}
          </section>

          <label className="admin-settings-reason-field">
            操作原因（必填）
            <TextArea
              aria-label="操作原因"
              autosize
              disabled={submitting}
              onChange={setReason}
              placeholder="请说明本次配置变更原因"
              value={reason}
            />
          </label>

          {submissionError ? <div className="admin-settings-feedback" data-kind="error" role="alert">{submissionError}</div> : null}
        </div>
      </Modal>
    </>
  );
}
