import { Typography } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';

const { Title, Text } = Typography;

type PageHeaderProps = {
  actions?: ReactNode;
  eyebrow?: string;
  title: string;
};

export function PageHeader({ actions, eyebrow = 'Admin Console', title }: PageHeaderProps) {
  return (
    <header className="page-header">
      <div>
        <Text className="page-header-eyebrow">{eyebrow}</Text>
        <Title heading={2}>{title}</Title>
      </div>
      {actions ? <div className="page-header-actions">{actions}</div> : null}
    </header>
  );
}
