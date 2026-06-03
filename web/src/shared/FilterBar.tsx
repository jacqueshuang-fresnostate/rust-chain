import { Button } from '@douyinfe/semi-ui';
import { type FormEvent, type ReactNode, useEffect, useState } from 'react';

import { AdminSelect, AdminTextInput } from './SemiFormControls';

export type FilterOption = {
  label: string;
  value: string;
};

export type FilterField = {
  key: string;
  label: string;
  options?: FilterOption[];
  optionsFromRows?: boolean;
  placeholder?: string;
  type?: 'input' | 'select';
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
  const [draftValues, setDraftValues] = useState<FilterValues>(value);

  useEffect(() => {
    setDraftValues(value);
  }, [value]);

  if (fields.length === 0) {
    return null;
  }

  function updateField(field: FilterField, nextValue: string) {
    setDraftValues((current) => ({ ...current, [field.key]: nextValue }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onChange(pruneEmpty(draftValues));
  }

  const controls: ReactNode[] = fields.map((field) => {
    const currentValue = draftValues[field.key] ?? '';
    const selectOptions = field.options ?? [];

    return (
      <label className="admin-filter-control" key={field.key}>
        <span>{field.label}</span>
        {field.type === 'select' ? (
          <AdminSelect
            ariaLabel={field.label}
            onChange={(nextValue) => updateField(field, nextValue)}
            optionList={[{ value: '', label: `全部${field.label}` }, ...selectOptions]}
            value={currentValue}
          />
        ) : (
          <AdminTextInput
            ariaLabel={field.label}
            onChange={(nextValue) => updateField(field, nextValue)}
            placeholder={field.placeholder ?? `请输入${field.label}`}
            value={currentValue}
          />
        )}
      </label>
    );
  });

  return (
    <form className="admin-filter-bar" onSubmit={handleSubmit}>
      {controls}
      <Button htmlType="submit" loading={loading} theme="solid" type="primary">
        查询
      </Button>
    </form>
  );
}
