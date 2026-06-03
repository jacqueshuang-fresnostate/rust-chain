import { Button, Modal, TextArea, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

const { Text } = Typography;

type ConfirmActionProps = {
  actionText?: string;
  confirmText?: string;
  disabled?: boolean;
  onConfirm: (reason: string) => Promise<void> | void;
  title: string;
};

export function ConfirmAction({ actionText = '执行', confirmText = '确认', disabled, onConfirm, title }: ConfirmActionProps) {
  const [reason, setReason] = useState('');
  const [visible, setVisible] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  async function handleConfirm() {
    const trimmed = reason.trim();
    if (!trimmed) {
      return;
    }

    setSubmitting(true);
    try {
      await onConfirm(trimmed);
      setVisible(false);
      setReason('');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <>
      <Button disabled={disabled} onClick={() => setVisible(true)} type="danger">
        {actionText}
      </Button>
      <Modal
        confirmLoading={submitting}
        motion={false}
        okButtonProps={{ 'aria-label': confirmText, disabled: reason.trim().length === 0 }}
        okText={confirmText}
        onCancel={() => setVisible(false)}
        onOk={handleConfirm}
        title={title}
        visible={visible}
      >
        <Text type="secondary">请输入非空原因后继续。</Text>
        <TextArea
          aria-label="操作原因"
          autosize
          onChange={setReason}
          placeholder="请输入操作原因"
          style={{ marginTop: 12 }}
          value={reason}
        />
      </Modal>
    </>
  );
}
