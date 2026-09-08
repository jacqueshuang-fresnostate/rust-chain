import { AdminTextInput } from '../../../../shared/SemiFormControls';
import { MarketStrategyVolatilityField } from '../../../components/MarketStrategyVolatilityField';
import { DefaultMarketSelect } from './DefaultMarketSelect';
import type { DefaultMarketDraft, DefaultMarketReferencePair } from './types';

export function DefaultMarketFollowForm({ draft, disabled, edit, pairId, references, referencePair, referenceLoading, canRead }: {
  draft: DefaultMarketDraft; disabled: boolean;
  edit: <K extends keyof DefaultMarketDraft>(key: K, value: DefaultMarketDraft[K]) => void;
  pairId: string; references: DefaultMarketReferencePair[]; referencePair: DefaultMarketReferencePair | null;
  referenceLoading: boolean; canRead: boolean;
}) {
  const options = references.filter((pair) => String(pair.id) !== pairId).map((pair) => ({ value: String(pair.id), label: `${pair.symbol}（ID: ${pair.id}）` }));
  if (draft.reference_pair_id && !options.some((option) => option.value === draft.reference_pair_id)) {
    options.unshift({ value: draft.reference_pair_id, label: `${referencePair && String(referencePair.id) === draft.reference_pair_id ? referencePair.symbol : `ID: ${draft.reference_pair_id}`}（尚未通过目录校验）` });
  }
  return <section aria-label="默认行情生成模式">
    <div className="admin-action-form admin-action-form-wide">
      <DefaultMarketSelect label="生成模式" disabled={disabled} value={draft.mode} optionList={[{ value: 'independent', label: '独立震荡' }, { value: 'follow', label: '跟随外部行情' }]}
        onChange={(mode) => { if (mode === 'independent' || mode === 'follow') edit('mode', mode); }} />
      {draft.mode === 'follow' ? <>
        <DefaultMarketSelect label="参考交易对" disabled={disabled || !canRead || referenceLoading} loading={referenceLoading} filter showClear value={draft.reference_pair_id}
          placeholder="搜索并明确选择，例如 BTC" optionList={options.map((option) => ({ ...option, disabled: option.value === pairId || !references.some((pair) => String(pair.id) === option.value) }))} onChange={(value) => edit('reference_pair_id', value)} />
        <label>跟随倍率<AdminTextInput ariaLabel="跟随倍率" disabled={disabled} value={draft.follow_multiplier} onChange={(value) => edit('follow_multiplier', value)} /><span className="admin-form-hint">大于 0 且不超过 3；1 表示参考价格变动 1% 时，本交易对目标同向变动 1%（受上限约束）。</span></label>
        <MarketStrategyVolatilityField ariaLabel="每分钟最大变动" label="每分钟最大变动" disabled={disabled} value={draft.follow_max_move_ratio} onChange={(value) => edit('follow_max_move_ratio', value)} />
        <label>参考过期时间（秒）<AdminTextInput ariaLabel="参考过期时间" disabled={disabled} value={draft.follow_stale_after_seconds} onChange={(value) => edit('follow_stale_after_seconds', value)} /><span className="admin-form-hint">15 至 300 秒；到期后自动切换独立震荡并记录提示。</span></label>
      </> : null}
    </div>
    {draft.mode === 'follow' ? <p>跟随参考价格的相对变化，不复制参考绝对价格或成交量。参考缺失、失效或过期时沿用当前价格独立震荡；恢复后重新建立基准，不补追断流期间的涨跌。手工策略优先和全部暂停规则不变。</p> : null}
  </section>;
}
