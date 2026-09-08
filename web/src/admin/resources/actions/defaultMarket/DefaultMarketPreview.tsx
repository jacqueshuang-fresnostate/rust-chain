import { adminErrorMessage } from '../../../../shared/adminErrorMessage';
import { MarketStrategyPreviewChart } from '../marketStrategy/MarketStrategyPreviewChart';
import { formatPreviewTime } from '../marketStrategy/model';
import type { DefaultMarketPreview as Preview } from './types';

export function DefaultMarketPreview({ preview, requestedMode = 'independent' }: { preview: Preview; requestedMode?: 'independent' | 'follow' }) {
  return <section aria-label="默认行情预览">
    <p>无副作用预览 · 版本 V{preview.version} · {preview.samples.length} 根一分钟 K 线 · 起始价格 {preview.start_price}</p>
    <p>随机种子：{preview.seed}。样本仅供检查，不写入行情、缓存或运行检查点；不代表实际用户成交。</p>
    {requestedMode === 'follow' && !preview.follow_preview ? <p role="alert">预览响应缺少参考依据，暂不确认跟随结果；请重新生成。样本不代表参考交易对未来价格。</p> : null}
    {preview.follow_preview ? <section aria-label="跟随预览依据">
      <p>{preview.follow_preview.kind === 'historical_replay' ? '历史参考回放' : '无参考证据的独立震荡样本'} · 参考交易对：{preview.follow_preview.reference_symbol}（ID: {preview.follow_preview.reference_pair_id}）</p>
      <p>参考采样区间：{formatPreviewTime(preview.follow_preview.range_start)} 至 {formatPreviewTime(preview.follow_preview.range_end)} · 参考采样数：{preview.follow_preview.reference_sample_count}</p>
      <p>仅使用已有历史参考采样，不预测 BTC 或其他参考交易对未来价格；参考观察时间沿用采集记录，不代表交易所成交时刻。</p>
      {preview.follow_preview.warning || preview.follow_preview.kind === 'independent_fallback' ? <p role="alert">{preview.follow_preview.warning ? adminErrorMessage(preview.follow_preview.warning) : '缺少可用于回放的参考证据，本次展示独立震荡样本。'}</p> : null}
    </section> : null}
    <MarketStrategyPreviewChart samples={preview.samples} />
    <div role="table" aria-label="默认行情 OHLCV 预览样本" className="admin-market-preview-grid">
      <div role="row" className="admin-market-preview-grid__row admin-market-preview-grid__head">
        {['时间', '开', '高', '低', '收', '成交量'].map((label) => <span role="columnheader" key={label}>{label}</span>)}
      </div>
      {preview.samples.map((sample) => <div role="row" key={sample.open_time} className="admin-market-preview-grid__row">
        <time role="cell">{formatPreviewTime(sample.open_time)}</time>
        <span role="cell">{sample.open}</span><span role="cell">{sample.high}</span><span role="cell">{sample.low}</span><strong role="cell">{sample.close}</strong><span role="cell">{sample.volume}</span>
      </div>)}
    </div>
  </section>;
}
