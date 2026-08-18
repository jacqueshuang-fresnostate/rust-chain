import { IconRefresh } from '@douyinfe/semi-icons';
import { Button, Card, Modal, Spin, Typography } from '@douyinfe/semi-ui';
import { type ReactNode, useState } from 'react';

import { PageHeader } from '../../layouts/PageHeader';
import { settingsErrorMessage } from './query';
import type { SettingsFeedback } from './useAdminSettingsEditor';
import { UnsavedChangesGuard } from './UnsavedChangesGuard';
import './settings.css';

const { Text } = Typography;

type AdminSettingsPageProps = {
  children: ReactNode;
  description?: string;
  feedback: SettingsFeedback | null;
  isDirty: boolean;
  isInitialLoading: boolean;
  isReady: boolean;
  isRefreshing: boolean;
  loadError: Error | null;
  onReload: () => Promise<void>;
  title: string;
};

/** 单例设置页共享壳：统一页头、加载/错误/成功/冲突状态、刷新保护与离开保护。 */
export function AdminSettingsPage({
  children,
  description,
  feedback,
  isDirty,
  isInitialLoading,
  isReady,
  isRefreshing,
  loadError,
  onReload,
  title
}: AdminSettingsPageProps) {
  const [reloadConfirmationVisible, setReloadConfirmationVisible] = useState(false);
  const [discarding, setDiscarding] = useState(false);

  async function reload() {
    try {
      await onReload();
      setReloadConfirmationVisible(false);
    } catch {
      // 共享编辑器会展示统一加载错误；失败时保留当前草稿。
    }
  }

  async function discardAndReload() {
    setDiscarding(true);
    try {
      await reload();
    } finally {
      setDiscarding(false);
    }
  }

  function requestReload() {
    if (isDirty) {
      setReloadConfirmationVisible(true);
      return;
    }
    void reload();
  }

  return (
    <main className="exchange-page admin-action-page admin-settings-page">
      <UnsavedChangesGuard enabled={isDirty} />
      <PageHeader
        actions={
          <Button
            aria-label="刷新配置"
            icon={<IconRefresh aria-hidden="true" />}
            loading={isRefreshing}
            onClick={requestReload}
            theme="borderless"
          >
            刷新
          </Button>
        }
        description={description}
        title={title}
      />

      <div aria-live="polite" className="admin-settings-status-stack">
        {isDirty ? <div className="admin-settings-feedback" data-kind="dirty" role="status">有未保存的变更</div> : null}
        {feedback ? (
          <div className="admin-settings-feedback" data-kind={feedback.kind} role={feedback.kind === 'success' ? 'status' : 'alert'}>
            <span>{feedback.message}</span>
            {feedback.kind === 'conflict' ? (
              <Button onClick={() => setReloadConfirmationVisible(true)} size="small" theme="borderless" type="danger">
                重新加载最新配置
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>

      {isInitialLoading ? (
        <Card bordered={false} className="admin-settings-state" shadows="always">
          <div aria-live="polite" role="status">
            <Spin size="large" />
            <Text>正在加载配置</Text>
          </div>
        </Card>
      ) : null}

      {loadError && !isReady ? (
        <Card bordered={false} className="admin-settings-state" shadows="always">
          <div role="alert">
            <Text strong>配置加载失败</Text>
            <Text>{settingsErrorMessage(loadError, '加载配置失败，请稍后重试。')}</Text>
            <Button loading={isRefreshing} onClick={() => void reload()} theme="solid" type="primary">重试加载</Button>
          </div>
        </Card>
      ) : null}

      {isReady ? children : null}

      <Modal
        cancelButtonProps={{ 'aria-label': '继续编辑', disabled: discarding }}
        cancelText="继续编辑"
        closeOnEsc={!discarding}
        confirmLoading={discarding}
        maskClosable={false}
        motion={false}
        okButtonProps={{ 'aria-label': '放弃未保存更改并重新加载', type: 'danger' }}
        okText="放弃更改并重新加载"
        onCancel={() => setReloadConfirmationVisible(false)}
        onOk={discardAndReload}
        title="确认重新加载配置"
        visible={reloadConfirmationVisible}
      >
        <Text>当前修改尚未保存。重新加载最新配置会丢弃这些更改，是否继续？</Text>
      </Modal>
    </main>
  );
}
