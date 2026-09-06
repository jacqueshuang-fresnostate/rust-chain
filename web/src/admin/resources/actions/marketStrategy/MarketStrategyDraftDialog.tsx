import { Modal } from '@douyinfe/semi-ui';

export function MarketStrategyDraftDialog({ action, onCancel, onConfirm }: {
  action: 'close' | 'reset' | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return <Modal
    visible={action !== null}
    title={action === 'close' ? '确认放弃行情策略修改' : '确认重置行情策略草稿'}
    maskClosable={false}
    motion={false}
    cancelText="继续编辑"
    cancelButtonProps={{ 'aria-label': '继续编辑' }}
    okText="放弃未保存修改"
    okButtonProps={{ 'aria-label': '放弃未保存修改', type: 'danger' }}
    onCancel={onCancel}
    onOk={onConfirm}
  >未保存的价格、节点和生成参数将被丢弃，已保存的后台配置不受影响。</Modal>;
}
