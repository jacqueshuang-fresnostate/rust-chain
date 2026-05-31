import { Button, Form } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';

export type FilterField = {
  key: string;
  label: string;
  placeholder?: string;
};

export type FilterValues = Record<string, string>;

type FilterBarProps = {
  fields?: FilterField[];
  loading?: boolean;
  onChange: (values: FilterValues) => void;
  value: FilterValues;
};

function pruneEmpty(values: FilterValues) {
  return Object.fromEntries(Object.entries(values).filter(([, value]) => value.trim().length > 0));
}

export function FilterBar({ fields = [], loading, onChange, value }: FilterBarProps) {
  if (fields.length === 0) {
    return null;
  }

  const controls: ReactNode[] = fields.map((field) => (
    <Form.Input
      aria-label={field.label}
      field={field.key}
      initValue={value[field.key] ?? ''}
      key={field.key}
      label={field.label}
      placeholder={field.placeholder ?? `请输入${field.label}`}
      style={{ minWidth: 180 }}
    />
  ));

  return (
    <Form<FilterValues>
      allowEmpty
      initValues={value}
      layout="horizontal"
      onSubmit={(values) => onChange(pruneEmpty(values))}
      style={{ alignItems: 'end', gap: 12 }}
    >
      {controls}
      <Button htmlType="submit" loading={loading} theme="solid" type="primary">
        查询
      </Button>
    </Form>
  );
}
