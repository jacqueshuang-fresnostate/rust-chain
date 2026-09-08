import { Select } from '@douyinfe/semi-ui';
import { useId } from 'react';
import type { SemiSelectOption } from '../../../../shared/SemiFormControls';

/** 当前 Semi Select 不透传 aria-label；本编辑器用其支持的 aria-labelledby 关联可见标签。 */
export function DefaultMarketSelect({ label, onChange, ...props }: {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  filter?: boolean;
  showClear?: boolean;
  placeholder?: string;
  value: string;
  optionList: SemiSelectOption[];
  onChange: (value: string) => void;
}) {
  const labelId = useId();
  return <label>
    <span id={labelId}>{label}</span>
    <Select {...props} aria-labelledby={labelId} style={{ width: '100%' }}
      onChange={(value) => onChange(value === null || value === undefined ? '' : String(value))}
      onSelect={(value) => onChange(value === null || value === undefined ? '' : String(value))} />
  </label>;
}
