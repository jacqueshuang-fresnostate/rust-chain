import { IconSearch } from '@douyinfe/semi-icons';
import { Button, Input, Switch } from '@douyinfe/semi-ui';
import { type FormEvent, type ReactNode, useEffect, useState } from 'react';

import { AdminSelect } from './SemiFormControls';

export type FilterOption = {
  label: string;
  value: string;
};

export type FilterField = {
  key: string;
  label: string;
  optionLabelKey?: string;
  options?: FilterOption[];
  optionsFromRows?: boolean;
  placeholder?: string;
  type?: 'input' | 'select' | 'switch';
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

  function handleReset() {
    setDraftValues({});
    onChange({});
  }

  const controls: ReactNode[] = fields.map((field) => {
    const currentValue = draftValues[field.key] ?? '';
    const selectOptions = field.options ?? [];

    return (
      <label className="admin-filter-control" key={field.key}>
        <span className="admin-filter-label">{field.label}</span>
        {field.type === 'switch' ? (
          <Switch
            aria-label={field.label}
            checked={currentValue === 'true'}
            checkedText="开"
            disabled={loading}
            onChange={(checked) => updateField(field, checked ? 'true' : '')}
            uncheckedText="关"
          />
        ) : field.type === 'select' ? (
          <AdminSelect
            ariaLabel={field.label}
            disabled={loading}
            onChange={(nextValue) => updateField(field, nextValue)}
            optionList={[{ value: '', label: `全部${field.label}` }, ...selectOptions]}
            placeholder={field.placeholder ?? field.label}
            value={currentValue}
          />
        ) : (
          <Input
            aria-label={field.label}
            onChange={(nextValue) => updateField(field, nextValue)}
            placeholder={field.placeholder ?? field.label}
            prefix={<IconSearch aria-hidden="true" />}
            showClear
            style={{ width: '100%' }}
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
      <Button disabled={loading} htmlType="button" onClick={handleReset}>
        重置
      </Button>
    </form>
  );
}
