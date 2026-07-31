import { Typography } from '@douyinfe/semi-ui';
import { type ReactNode, useEffect } from 'react';

const { Text, Title } = Typography;

type PageHeaderProps = {
  actions?: ReactNode;
  description?: ReactNode;
  title: string;
};

export function PageHeader({ actions, description, title }: PageHeaderProps) {
  useEffect(() => {
    document.title = `${title} · HIPPO Operations`;
  }, [title]);

  return (
    <header className="page-header">
      <div className="page-header-copy">
        <Text className="page-header-kicker">HIPPO OPERATIONS</Text>
        <Title heading={2}>{title}</Title>
        {description ? (
          <Text className="page-header-description" type="tertiary">
            {description}
          </Text>
        ) : null}
      </div>
      {actions ? <div className="page-header-actions">{actions}</div> : <div aria-hidden="true" className="page-header-actions page-header-actions-empty" />}
    </header>
  );
}
