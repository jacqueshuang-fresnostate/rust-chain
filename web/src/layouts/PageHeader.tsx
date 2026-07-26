import { Typography } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';

const { Text, Title } = Typography;

type PageHeaderProps = {
  actions?: ReactNode;
  description?: ReactNode;
  title: string;
};

export function PageHeader({ actions, description, title }: PageHeaderProps) {
  return (
    <header className="page-header">
      <div>
        <Title heading={2}>{title}</Title>
        {description ? (
          <Text className="page-header-description" type="tertiary">
            {description}
          </Text>
        ) : null}
      </div>
      {actions ? <div className="page-header-actions">{actions}</div> : null}
    </header>
  );
}
