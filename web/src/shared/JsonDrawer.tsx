import { SideSheet, Typography } from '@douyinfe/semi-ui';

const { Text } = Typography;

type JsonDrawerProps = {
  data: unknown;
  onClose: () => void;
  title?: string;
  visible: boolean;
};

export function JsonDrawer({ data, onClose, title = 'JSON详情', visible }: JsonDrawerProps) {
  return (
    <SideSheet onCancel={onClose} title={title} visible={visible} width={560}>
      <pre
        style={{
          background: '#07111f',
          borderRadius: 12,
          color: '#e8edf7',
          margin: 0,
          overflow: 'auto',
          padding: 16,
          whiteSpace: 'pre-wrap'
        }}
      >
        <Text style={{ color: 'inherit' }}>{JSON.stringify(data, null, 2)}</Text>
      </pre>
    </SideSheet>
  );
}
