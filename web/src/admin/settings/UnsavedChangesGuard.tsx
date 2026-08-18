import { Modal, Typography } from '@douyinfe/semi-ui';
import { useCallback, useEffect } from 'react';
import { type BlockerFunction, useBlocker } from 'react-router-dom';

const { Text } = Typography;

export const UNSAVED_CHANGES_MESSAGE = '你有未保存的更改，离开当前页面将丢失这些内容。';

export function useBeforeUnloadGuard(enabled: boolean, message = UNSAVED_CHANGES_MESSAGE) {
  useEffect(() => {
    if (!enabled) {
      return;
    }

    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      event.returnValue = message;
      event.preventDefault();
      return message;
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => window.removeEventListener('beforeunload', handleBeforeUnload);
  }, [enabled, message]);
}

type UnsavedChangesGuardProps = {
  enabled: boolean;
  message?: string;
};

/** 同时覆盖浏览器关闭/刷新和 React Router 站内跳转。 */
export function UnsavedChangesGuard({ enabled, message = UNSAVED_CHANGES_MESSAGE }: UnsavedChangesGuardProps) {
  useBeforeUnloadGuard(enabled, message);

  const shouldBlock = useCallback(
    ({ currentLocation, nextLocation }: Parameters<BlockerFunction>[0]) =>
      enabled &&
      `${currentLocation.pathname}${currentLocation.search}${currentLocation.hash}` !==
        `${nextLocation.pathname}${nextLocation.search}${nextLocation.hash}`,
    [enabled]
  );
  const blocker = useBlocker(shouldBlock);

  function keepEditing() {
    if (blocker.state === 'blocked') {
      blocker.reset();
    }
  }

  function leavePage() {
    if (blocker.state === 'blocked') {
      blocker.proceed();
    }
  }

  return (
    <Modal
      cancelButtonProps={{ 'aria-label': '继续编辑' }}
      cancelText="继续编辑"
      closeOnEsc
      maskClosable={false}
      motion={false}
      okButtonProps={{ 'aria-label': '放弃未保存更改并离开', type: 'danger' }}
      okText="放弃更改并离开"
      onCancel={keepEditing}
      onOk={leavePage}
      title="确认离开当前页面"
      visible={blocker.state === 'blocked'}
    >
      <Text>{message}</Text>
    </Modal>
  );
}
