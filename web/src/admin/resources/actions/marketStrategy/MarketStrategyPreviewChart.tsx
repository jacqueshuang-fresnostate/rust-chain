import { useMemo } from 'react';

import type { MarketStrategyPreviewSample } from './types';

export function MarketStrategyPreviewChart({ samples }: { samples: MarketStrategyPreviewSample[] }) {
  // Number 只用于 SVG 几何，接口小数文本原样保留在图中提示及下方 OHLCV 表。
  const bars = useMemo(() => samples.map((sample) => ({
    sample, open: Number(sample.open), high: Number(sample.high),
    low: Number(sample.low), close: Number(sample.close)
  })).filter(({ open, high, low, close }) => (
    [open, high, low, close].every((value) => Number.isFinite(value) && value > 0)
    && high >= Math.max(open, close) && low <= Math.min(open, close)
  )), [samples]);
  const minimum = Math.min(...bars.map((bar) => bar.low));
  const maximum = Math.max(...bars.map((bar) => bar.high));
  const range = maximum - minimum;
  const y = (price: number) => range > 0 ? 192 - ((price - minimum) / range) * 184 : 100;
  const spacing = 720 / Math.max(1, bars.length);
  const width = Math.min(12, spacing * 0.6);

  return (
    <div>
      {bars.length ? <p className="admin-market-volatility-hint">样本最高 {bars.find((bar) => bar.high === maximum)?.sample.high} · 样本最低 {bars.find((bar) => bar.low === minimum)?.sample.low}</p> : null}
      <svg aria-label="预览 K 线（开高低收）" className="admin-market-preview-chart" preserveAspectRatio="none" role="img" viewBox="0 0 720 200">
        {bars.map(({ sample, open, high, low, close }, index) => {
          const x = (index + 0.5) * spacing;
          const bodyTop = Math.min(y(open), y(close));
          const bodyHeight = Math.max(1, Math.abs(y(open) - y(close)));
          return (
            <g className={close >= open ? 'admin-market-preview-up' : 'admin-market-preview-down'} key={sample.open_time}>
              <title>{`开 ${sample.open} 高 ${sample.high} 低 ${sample.low} 收 ${sample.close}`}</title>
              <line data-preview-wick={sample.open_time} data-high={sample.high} data-low={sample.low} x1={x} x2={x} y1={y(high)} y2={y(low)} stroke="currentColor" vectorEffect="non-scaling-stroke" />
              <rect data-preview-body={sample.open_time} x={x - width / 2} y={bodyTop} width={width} height={bodyHeight} fill="currentColor" />
            </g>
          );
        })}
      </svg>
      <p className="admin-market-volatility-hint">纵轴包含完整最高价与最低价；每根蜡烛对应一条返回样本，精确数值见下表。</p>
      {bars.length !== samples.length ? <p role="alert">部分样本价格无效，请检查下方原始 OHLCV 数据。</p> : null}
      {!samples.length ? <p>暂无预览样本</p> : null}
    </div>
  );
}
