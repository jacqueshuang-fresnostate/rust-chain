import { Col, Row } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';

type FieldColumnSize = 'full' | 'half' | 'third';

const fieldColumnProps: Record<FieldColumnSize, { md?: number; xl?: number; xs: number }> = {
  full: { xs: 24 },
  half: { xs: 24, md: 12 },
  third: { xs: 24, md: 12, xl: 8 }
};

export function PredictionFieldLabel({ children, label }: { children: ReactNode; label: string }) {
  return (
    <label style={{ display: 'grid', gap: 6, width: '100%' }}>
      {label}
      {children}
    </label>
  );
}

export function PredictionFieldColumn({
  children,
  size = 'half'
}: {
  children: ReactNode;
  size?: FieldColumnSize;
}) {
  return <Col {...fieldColumnProps[size]}>{children}</Col>;
}

export function PredictionConfigGrid({ children }: { children: ReactNode }) {
  return (
    <Row gutter={[24, 18]} style={{ width: '100%' }}>
      {children}
    </Row>
  );
}
