import { IconRefresh } from '@douyinfe/semi-icons';
import { Button, Space } from '@douyinfe/semi-ui';
import { useNavigate } from 'react-router-dom';

type WorkflowPageActionsProps = {
  loading: boolean;
  onRefresh: () => void;
  shortcutLabel: string;
  shortcutPath: string;
};

/** 配置与运营工作区共用的双向入口与刷新操作。 */
export function WorkflowPageActions({ loading, onRefresh, shortcutLabel, shortcutPath }: WorkflowPageActionsProps) {
  const navigate = useNavigate();

  return (
    <Space>
      <Button onClick={() => navigate(shortcutPath)} theme="light" type="primary">
        {shortcutLabel}
      </Button>
      <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={onRefresh} theme="borderless">
        刷新
      </Button>
    </Space>
  );
}
