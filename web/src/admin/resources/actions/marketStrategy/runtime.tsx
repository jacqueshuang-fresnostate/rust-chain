import { useMemo } from 'react';
import type { ApiRecord } from '../../../../api/types';
import { TimestampText } from '../../../../shared/TimestampText';
import { adminErrorMessage } from '../../../../shared/adminErrorMessage';
import type { MarketStrategyValues } from './types';

/** 运行状态是观察结果，不以 active 或初始化的 last_generated_at 冒充已推送。 */
export function marketStrategyRuntime(record: ApiRecord, now = Date.now()): { label: string; detail: string } {
  if (record.status !== 'active') {
    const labels: Record<string, string> = { draft: '草稿', paused: '已暂停', disabled: '已禁用' };
    return { label: labels[String(record.status)] ?? '状态待确认', detail: marketStrategyActivationError(record, now) ?? '启用后在有效时间范围内生成行情' };
  }
  if (typeof record.end_time === 'number' && record.end_time <= now) return { label: '已结束', detail: '当前时间已超出策略范围，修改时间后再启用' };
  if (typeof record.start_time === 'number' && record.start_time > now) return { label: '待开始', detail: '到达开始时间后自动生成行情' };
  if (!record.run_status) return { label: '运行记录缺失', detail: '请检查策略运行记录与配置版本' };
  if (typeof record.error_message === 'string' && record.error_message.trim()) return { label: '运行异常', detail: adminErrorMessage(record.error_message, '行情生成异常') };
  if (record.run_status !== 'running' && record.run_status !== 'live') return { label: '运行状态异常', detail: '业务状态已启用但运行状态未就绪，请检查策略运行记录' };
  if (record.recovery_status !== 'live' || typeof record.last_tick_at !== 'number' || !Number.isFinite(record.last_tick_at)) {
    return { label: '待首笔行情', detail: '尚未确认成功推送，请检查后台行情任务与 MySQL / MongoDB / Redis 连接' };
  }
  if (now - record.last_tick_at > 15_000) return { label: '推送延迟', detail: '最近一次成功推送已超过 15 秒，请检查运行日志' };
  return { label: '推送正常', detail: '最新行情、1m K 线、模拟盘口/成交及高周期生成轮次已成功' };
}

export function marketStrategyCreateActivationError(values: MarketStrategyValues, now = Date.now()): string | null {
  return values.status === 'active' ? marketStrategyActivationError({ end_time: Date.parse(values.endTime) }, now) : null;
}

export function marketStrategyActivationError(record: ApiRecord, now = Date.now()): string | null {
  return typeof record.end_time === 'number' && record.end_time <= now
    ? '策略已结束，请先修改结束时间再启用' : null;
}

export function MarketStrategyRuntimeStatus({ record }: { record: ApiRecord }) {
  // 列表未重新加载时不把旧快照自动老化成故障，诊断时间绑定本次 API 行对象。
  const runtime = useMemo(() => marketStrategyRuntime(record), [record]);
  return (
    <div className="admin-market-strategy-runtime" title={`${runtime.detail}；点击列表刷新获取最新诊断`}>
      <strong>{runtime.label}（加载时）</strong>
      <span>{runtime.detail}</span>
      <small>最近推送：<TimestampText value={typeof record.last_tick_at === 'number' ? record.last_tick_at : null} /></small>
    </div>
  );
}
