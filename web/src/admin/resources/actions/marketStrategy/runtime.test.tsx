import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MarketStrategyRuntimeStatus, marketStrategyRuntime, marketStrategyActivationError } from './runtime';

const now = 1_788_670_800_000;
const active = { status: 'active', run_status: 'running', start_time: now - 60_000, end_time: now + 60_000, recovery_status: 'live', last_tick_at: now - 1000 };

describe('strategy operational health', () => {
  it('distinguishes scheduled, expired, missing, pending, error, delayed and live states', () => {
    expect(marketStrategyRuntime(active, now).label).toBe('推送正常');
    expect(marketStrategyRuntime({ ...active, start_time: now + 1 }, now).label).toBe('待开始');
    expect(marketStrategyRuntime({ ...active, end_time: now }, now).label).toBe('已结束');
    expect(marketStrategyRuntime({ ...active, run_status: null }, now).label).toBe('运行记录缺失');
    expect(marketStrategyRuntime({ ...active, last_tick_at: null, last_generated_at: now }, now).label).toBe('待首笔行情');
    expect(marketStrategyRuntime({ ...active, run_status: 'paused' }, now).label).toBe('运行状态异常');
    expect(marketStrategyRuntime({ ...active, status: 'paused', end_time: now }, now).detail).toContain('已结束');
    expect(marketStrategyRuntime({ ...active, recovery_status: 'idle' }, now).label).toBe('待首笔行情');
    expect(marketStrategyRuntime({ ...active, error_message: 'Redis write failed' }, now)).toEqual({ label: '运行异常', detail: 'Redis write failed' });
    expect(marketStrategyRuntime({ ...active, last_tick_at: now - 15_000 }, now).label).toBe('推送正常');
    expect(marketStrategyRuntime({ ...active, last_tick_at: now - 15_001 }, now).label).toBe('推送延迟');
    expect(marketStrategyRuntime({ ...active, status: 'paused' }, now).label).toBe('已暂停');
  });
  it('blocks expired activation without blocking future schedules', () => {
    expect(marketStrategyActivationError({ end_time: now }, now)).toContain('已结束');
    expect(marketStrategyActivationError({ end_time: now + 1 }, now)).toBeNull();
  });
  it('renders backend error and last real push time instead of hiding diagnostic evidence', () => {
    render(<MarketStrategyRuntimeStatus record={{ ...active, start_time: Date.now() - 60_000, end_time: Date.now() + 60_000, error_message: 'Mongo write failed' }} />);
    expect(screen.getByText('运行异常（加载时）')).toBeVisible();
    expect(screen.getByText('Mongo write failed')).toBeVisible();
    expect(screen.getByText(/最近推送/)).toBeVisible();
  });
});
