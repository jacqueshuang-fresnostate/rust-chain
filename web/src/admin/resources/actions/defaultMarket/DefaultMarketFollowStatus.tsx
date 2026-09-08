import { adminErrorMessage } from '../../../../shared/adminErrorMessage';
import type { DefaultMarketResponse } from './types';

const time = (value: number | null) => value === null ? '暂无记录' : new Date(value).toLocaleString('zh-CN', { hour12: false });
export function followFallbackReason(reason: string | null): string {
  return reason ? adminErrorMessage(reason, '参考行情不可用，正在独立震荡') : '参考行情不可用，正在独立震荡';
}

export function DefaultMarketFollowStatus({ value }: { value: DefaultMarketResponse }) {
  const follow = value.runtime.follow;
  const configuredMode = value.config.mode ?? 'independent';
  return <section aria-label="跟随行情读取状态">
    <p>已保存模式：{configuredMode === 'follow' ? '跟随外部行情' : configuredMode === 'independent' ? '独立震荡' : configuredMode}{value.reference_pair ? ` · 参考交易对：${value.reference_pair.symbol}（ID: ${value.reference_pair.id}）` : ''}</p>
    {value.config.mode === 'follow' && !follow ? <p>暂无已记录的跟随运行状态；保存配置不代表已经开始跟随。</p> : null}
    {follow ? <>
      <p>最近默认生成状态：{follow.mode === 'following' ? '正在跟随参考行情' : follow.mode === 'fallback' ? '独立震荡兜底' : follow.mode} · 参考：{follow.reference_symbol}（ID: {follow.reference_pair_id}）</p>
      <p>参考价格：{follow.reference_price ?? '暂无价格'} · 参考采集观察时间：{time(follow.reference_observed_at)} · 状态切换时间：{time(follow.switched_at)}</p>
      {follow.mode === 'fallback' ? <p role="alert">参考行情不可用，已切换独立震荡：{followFallbackReason(follow.fallback_reason)}</p> : null}
      {value.runtime.active_source !== 'default' || value.all_market_paused ? <p>当前未由默认行情生成；以上为最近一次记录，不代表持续更新。</p> : null}
    </> : null}
  </section>;
}
