import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';

import { apiRequest } from '../../../../api/client';
import { MarketStrategyPreviewAction } from './MarketStrategyPreviewAction';
import { MarketStrategyPreviewChart } from './MarketStrategyPreviewChart';
import { initialMarketStrategy } from './model';

vi.mock('../../../../api/client', () => ({ apiRequest: vi.fn() }));

beforeEach(() => {
  vi.mocked(apiRequest).mockReset();
});

it('previews actual high/low wicks, bodies and sparse sampling rather than closes alone', async () => {
  vi.mocked(apiRequest).mockResolvedValue({
    one_minute_count: 60,
    sample_count: 2,
    preview_version: 1,
    preview_seed: 'shape-fixture',
    samples: [
      { open_time: 1788679800000, open: '0.096527', high: '0.393109', low: '0.000001', close: '0.103197', volume: '10' },
      { open_time: 1788679860000, open: '0.103197', high: '0.12', low: '0.09', close: '0.1', volume: '20' }
    ]
  });
  render(<MarketStrategyPreviewAction disabled={false} values={{
    ...initialMarketStrategy,
    pairId: '3', startPrice: '0.1', targetPrice: '0.12',
    startTime: '2026-09-06T15:30', endTime: '2026-09-06T16:30'
  }} />);
  fireEvent.click(screen.getByRole('button', { name: '生成 OHLCV 预览' }));

  const chart = await screen.findByRole('img', { name: '预览 K 线（开高低收）' });
  expect(chart.querySelectorAll('[data-preview-wick]')).toHaveLength(2);
  expect(chart.querySelectorAll('[data-preview-body]')).toHaveLength(2);
  const wick = chart.querySelector('[data-preview-wick="1788679800000"]');
  expect(wick).toHaveAttribute('data-high', '0.393109');
  expect(wick).toHaveAttribute('data-low', '0.000001');
  expect(Number(wick?.getAttribute('y1'))).toBeLessThan(Number(wick?.getAttribute('y2')));
  expect(screen.getByText(/采样 K 线，不代表连续分钟/)).toBeInTheDocument();
  expect(screen.getByRole('table', { name: 'OHLCV 预览样本' })).toBeInTheDocument();
  expect(apiRequest).toHaveBeenCalledTimes(1);
});

it('keeps flat and tiny valid candles visible without non-finite geometry, and reports invalid samples', () => {
  const samples = [
    { open_time: 1, open: '0.00000001', high: '0.00000001', low: '0.00000001', close: '0.00000001', volume: '0' }
  ];
  const { rerender } = render(<MarketStrategyPreviewChart samples={samples} />);
  const body = screen.getByRole('img').querySelector('[data-preview-body]');
  expect(body).toHaveAttribute('y', '100');
  expect(body).toHaveAttribute('height', '1');
  expect(screen.getByText(/样本最高 0.00000001 · 样本最低 0.00000001/)).toBeInTheDocument();
  rerender(<MarketStrategyPreviewChart samples={[
    ...samples,
    { ...samples[0], open_time: 2, high: 'invalid' },
    { ...samples[0], open_time: 3, low: '0.1' }
  ]} />);
  expect(screen.getByRole('img').querySelectorAll('[data-preview-wick]')).toHaveLength(1);
  expect(screen.getByRole('alert')).toHaveTextContent('部分样本价格无效');
  expect(screen.getByRole('img').outerHTML).not.toMatch(/NaN|Infinity/);
  rerender(<MarketStrategyPreviewChart samples={[]} />);
  expect(screen.getByText('暂无预览样本')).toBeInTheDocument();
  expect(screen.getByRole('img').querySelectorAll('[data-preview-body]')).toHaveLength(0);
});
