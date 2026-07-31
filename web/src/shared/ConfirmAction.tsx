import { Button, Modal, TextArea, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

const { Text } = Typography;

type ConfirmActionProps = {
  actionText?: string;
  confirmText?: string;
  dangerous?: boolean;
  disabled?: boolean;
  onConfirm: (reason: string) => Promise<void> | void;
  title: string;
};

export function ConfirmAction({ actionText = '执行', confirmText = '确认', dangerous, disabled, onConfirm, title }: ConfirmActionProps) {
  const [reason, setReason] = useState('');
  const [visible, setVisible] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const destructive = dangerous ?? /删除|拒绝|驳回|作废|禁用|停用|标记失败|注销|移除|冲正|回收|撤单|重置|归档/.test(`${actionText}${title}`);

  function closeModal() {
    if (submitting) {
      return;
    }
    setVisible(false);
    setReason('');
  }

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
      <Button disabled={disabled} onClick={() => setVisible(true)} theme={destructive ? 'light' : 'solid'} type={destructive ? 'danger' : 'primary'}>
        {actionText}
      </Button>
      <Modal
        cancelButtonProps={{ 'aria-label': '取消', disabled: submitting }}
        closeOnEsc={!submitting}
        confirmLoading={submitting}
        maskClosable={false}
        motion={false}
        okButtonProps={{ 'aria-label': confirmText, disabled: reason.trim().length === 0, type: destructive ? 'danger' : 'primary' }}
        okText={confirmText}
        onCancel={closeModal}
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
