import { Typography } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';

const { Title } = Typography;

type PageHeaderProps = {
  actions?: ReactNode;
  title: string;
};

export function PageHeader({ actions, title }: PageHeaderProps) {
  return (
    <header className="page-header">
      <div>
        <Title heading={2}>{title}</Title>
      </div>
      {actions ? <div className="page-header-actions">{actions}</div> : null}
    </header>
  );
}
