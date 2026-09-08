import { render, screen, within } from '@testing-library/react';
import { expect, it } from 'vitest';
import { DefaultMarketPreview } from './DefaultMarketPreview';
import type { DefaultMarketPreview as Preview } from './types';

const preview: Preview = { pair_id: 3, version: 2, seed: 'default-market:3', start_price: '0.1', samples: [{ open_time: 1788700020000, open: '0.1', high: '0.11', low: '0.09', close: '0.105', volume: '10' }] };
const metadata = { kind: 'historical_replay' as const, reference_pair_id: 8, reference_symbol: 'BTCUSDT', range_start: 1788700020000, range_end: 1788703620000, reference_sample_count: 123, warning: null };

it('distinguishes actual archived reference replay from fallback samples without forecasting future BTC', () => {
  const view = render(<DefaultMarketPreview requestedMode="follow" preview={{ ...preview, follow_preview: metadata }} />);
  const evidence = screen.getByRole('region', { name: '跟随预览依据' });
  expect(evidence).toHaveTextContent('历史参考回放');
  expect(evidence).toHaveTextContent('参考采样数：123');
  expect(evidence).toHaveTextContent('不预测 BTC');
  expect(evidence).toHaveTextContent('不代表交易所成交时刻');
  expect(screen.getByRole('table')).toHaveTextContent('0.105');
  view.rerender(<DefaultMarketPreview requestedMode="follow" preview={{ ...preview, follow_preview: { ...metadata, kind: 'independent_fallback', reference_sample_count: 0, warning: '历史参考采样缺失，使用独立震荡配置' } }} />);
  expect(within(screen.getByRole('region', { name: '跟随预览依据' })).getByRole('alert')).toHaveTextContent('历史参考采样缺失');
  expect(screen.getByText(/无参考证据的独立震荡样本/)).toBeInTheDocument();
  expect(screen.queryByText(/历史参考回放/)).not.toBeInTheDocument();
});

it('does not silently accept a follow preview without provenance and preserves legacy independent previews', () => {
  const view = render(<DefaultMarketPreview requestedMode="follow" preview={preview} />);
  expect(screen.getByRole('alert')).toHaveTextContent('预览响应缺少参考依据');
  view.rerender(<DefaultMarketPreview preview={preview} />);
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(screen.getByRole('table')).toHaveTextContent('0.105');
});
