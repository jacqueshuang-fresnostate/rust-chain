import { Button, Checkbox, Input, Select, Switch, TextArea } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';

export type SemiSelectOption = {
  label: string;
  value: string;
};

type TextInputProps = {
  ariaLabel: string;
  disabled?: boolean;
  onChange: (value: string) => void;
  placeholder?: string;
  readOnly?: boolean;
  type?: string;
  value: string;
};

export function AdminTextInput({ ariaLabel, disabled, onChange, placeholder, readOnly, type, value }: TextInputProps) {
  return (
    <Input
      aria-label={ariaLabel}
      disabled={disabled}
      onChange={(nextValue) => onChange(String(nextValue))}
      placeholder={placeholder}
      readOnly={readOnly}
      style={{ width: '100%' }}
      type={type}
      value={value}
    />
  );
}

export function AdminPasswordInput({ ariaLabel, disabled, onChange, placeholder, readOnly, value }: TextInputProps) {
  return (
    <Input
      aria-label={ariaLabel}
      disabled={disabled}
      mode="password"
      onChange={(nextValue) => onChange(String(nextValue))}
      placeholder={placeholder}
      readOnly={readOnly}
      style={{ width: '100%' }}
      value={value}
    />
  );
}

type SelectProps = {
  ariaLabel: string;
  disabled?: boolean;
  filter?: boolean;
  loading?: boolean;
  onChange: (value: string) => void;
  optionList: SemiSelectOption[];
  placeholder?: string;
  showClear?: boolean;
  value: string;
};

export function AdminSelect({ ariaLabel, disabled, filter, loading, onChange, optionList, placeholder, showClear, value }: SelectProps) {
  return (
    <Select
      aria-label={ariaLabel}
      disabled={disabled}
      filter={filter}
      loading={loading}
      onChange={(nextValue) => onChange(String(nextValue))}
      onSelect={(nextValue) => onChange(String(nextValue))}
      optionList={optionList}
      placeholder={placeholder}
      showClear={showClear}
      style={{ width: '100%' }}
      value={value}
    />
  );
}

type MultiSelectProps = {
  ariaLabel: string;
  disabled?: boolean;
  loading?: boolean;
  onChange: (value: string[]) => void;
  optionList: SemiSelectOption[];
  placeholder?: string;
  value: string[];
};

export function AdminMultiSelect({ ariaLabel, disabled, loading, onChange, optionList, placeholder, value }: MultiSelectProps) {
  return (
    <Select
      aria-label={ariaLabel}
      disabled={disabled}
      loading={loading}
      multiple
      onChange={(nextValue) => onChange(Array.isArray(nextValue) ? nextValue.map(String) : [])}
      optionList={optionList}
      placeholder={placeholder}
      style={{ width: '100%' }}
      value={value}
    />
  );
}

type TextAreaProps = {
  ariaLabel: string;
  autosize?: boolean;
  onChange: (value: string) => void;
  placeholder?: string;
  value: string;
};

export function AdminTextArea({ ariaLabel, autosize, onChange, placeholder, value }: TextAreaProps) {
  return <TextArea aria-label={ariaLabel} autosize={autosize} onChange={onChange} placeholder={placeholder} style={{ width: '100%' }} value={value} />;
}

type CheckboxProps = {
  checked: boolean;
  children: ReactNode;
  onChange: (checked: boolean) => void;
};

export function AdminCheckbox({ checked, children, onChange }: CheckboxProps) {
  return <Checkbox checked={checked} onChange={(event) => onChange(Boolean(event.target.checked))}>{children}</Checkbox>;
}

type SwitchProps = {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
};

export function AdminSwitch({ checked, label, onChange }: SwitchProps) {
  return (
    <label>
      {label}
      <Switch aria-label={label} checked={checked} onChange={onChange} />
    </label>
  );
}

type ModalTriggerButtonProps = {
  children: ReactNode;
  onClick: () => void;
};

export function AdminModalTriggerButton({ children, onClick }: ModalTriggerButtonProps) {
  return (
    <Button onClick={onClick} theme="solid" type="primary">
      {children}
    </Button>
  );
}
