import { AdminTextInput } from '../../shared/SemiFormControls';
import { isNonNegativeDecimalText, multiplyDecimalText } from '../../shared/decimal';

/** 沿用接口的小数比例，展示换算值而不重写输入草稿或历史配置。 */
export function MarketStrategyVolatilityField({
  ariaLabel = '波动率', disabled = false, label = '波动率', onChange, value
}: {
  ariaLabel?: string;
  disabled?: boolean;
  label?: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const percent = isNonNegativeDecimalText(value) ? multiplyDecimalText(value, '100') : null;
  return (
    <label>
      {label}（小数比例）
      <AdminTextInput ariaLabel={ariaLabel} disabled={disabled} onChange={onChange} placeholder="0.01 = 1%" value={value} />
      <span className="admin-market-volatility-hint">{percent === null ? '请输入非负小数比例' : `当前为 ${percent}%`}；0.01 表示 1%，1 表示 100%。</span>
    </label>
  );
}
