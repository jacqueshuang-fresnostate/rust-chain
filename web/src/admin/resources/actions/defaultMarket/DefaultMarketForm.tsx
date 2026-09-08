import { AdminCheckbox, AdminTextInput } from '../../../../shared/SemiFormControls';
import { MarketStrategyVolatilityField } from '../../../components/MarketStrategyVolatilityField';
import type { DefaultMarketDraft } from './types';

export function DefaultMarketForm({ draft, disabled, edit, pricePrecision, qtyPrecision }: {
  draft: DefaultMarketDraft;
  disabled: boolean;
  edit: <K extends keyof DefaultMarketDraft>(key: K, value: DefaultMarketDraft[K]) => void;
  pricePrecision: number;
  qtyPrecision: number;
}) {
  return (
    <>
      <section aria-label="通用生成参数">
        <h4>通用生成参数（独立与跟随均适用）</h4>
        <AdminCheckbox checked={draft.enabled} disabled={disabled} onChange={(enabled) => edit('enabled', enabled)}>启用默认行情</AdminCheckbox>
        <p className="admin-form-hint">价格精度 {pricePrecision} 位，数量精度 {qtyPrecision} 位。小数原样提交，不自动舍入。</p>
        <div className="admin-action-form admin-action-form-wide">
          <label>初始价格（可选）<AdminTextInput ariaLabel="初始价格" disabled={disabled} value={draft.initial_price} onChange={(value) => edit('initial_price', value)} placeholder="无已有价格时必须填写" /><span className="admin-form-hint">优先续接已有运行或历史价格；仅无可用价格时使用初始价格，不设隐式初始价。</span></label>
          <label>盘口档数<AdminTextInput ariaLabel="盘口档数" disabled={disabled} value={draft.depth_levels} onChange={(value) => edit('depth_levels', value)} placeholder="1 至 20" /></label>
          {([['price_min', '价格下限（可选）'], ['price_max', '价格上限（可选）'], ['volume_min', '分钟成交量下限'], ['volume_max', '分钟成交量上限']] as const).map(([key, label]) => (
            <label key={key}>{label}<AdminTextInput ariaLabel={label} disabled={disabled} value={draft[key]} onChange={(value) => edit(key, value)} /></label>
          ))}
        </div>
      </section>
      <section aria-label="独立震荡参数">
        <h4>{draft.mode === 'follow' ? '参考过期时的独立震荡配置' : '独立震荡配置'}</h4>
        <div className="admin-action-form admin-action-form-wide">
          {([['volatility', '波动率'], ['mean_reversion', '均值回归强度'], ['wick_strength', '影线强度']] as const).map(([key, label]) => (
            <MarketStrategyVolatilityField key={key} ariaLabel={label} label={label} disabled={disabled} value={draft[key]} onChange={(value) => edit(key, value)} />
          ))}
        </div>
      </section>
    </>
  );
}
