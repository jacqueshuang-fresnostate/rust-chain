import { Button, Card, SideSheet, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../../api/client';
import { errorMessage, requiredPositiveInteger } from '../shared';
import { formatPreviewTime, marketStrategyBasePayload } from './model';
import { MarketStrategyPreviewChart } from './MarketStrategyPreviewChart';
import type {
  MarketStrategyPreviewResponse,
  MarketStrategyValues
} from './types';

export function MarketStrategyPreviewAction({
  disabled,
  strategyId,
  values
}: {
  disabled: boolean;
  strategyId?: string;
  values: MarketStrategyValues;
}) {
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [preview, setPreview] = useState<MarketStrategyPreviewResponse | null>(null);

  async function runPreview() {
    setVisible(true);
    setLoading(true);
    setError('');
    setPreview(null);
    try {
      const result = await apiRequest<MarketStrategyPreviewResponse>('/admin/api/v1/market-strategies/preview', {
        method: 'POST',
        body: JSON.stringify({
          pair_id: requiredPositiveInteger(values.pairId, '交易对ID'),
          ...(strategyId ? { strategy_id: requiredPositiveInteger(strategyId, '策略ID') } : {}),
          ...marketStrategyBasePayload(values),
          status: values.status,
          sample_count: 120
        })
      });
      setPreview(result);
    } catch (previewError) {
      setError(errorMessage(previewError));
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <Button disabled={disabled} onClick={() => void runPreview()} theme="light" type="primary">
        生成 OHLCV 预览
      </Button>
      <SideSheet
        maskClosable={!loading}
        onCancel={() => setVisible(false)}
        title="模拟行情预览"
        visible={visible}
        width={860}
      >
        <div aria-busy={loading} aria-live="polite" className="admin-market-preview-sheet">
          <div className="admin-market-preview-heading">
            <div>
              <Typography.Title heading={5}>无副作用预览</Typography.Title>
              <Typography.Text type="tertiary">只读取交易对并在内存生成采样，不写入行情、缓存或运行检查点。</Typography.Text>
            </div>
            <Button disabled={loading} loading={loading} onClick={() => void runPreview()} size="small">重新生成</Button>
          </div>
          {error ? <div className="admin-inline-error" role="alert">{error}</div> : null}
          {loading ? <div className="admin-market-preview-state">正在生成确定性行情样本…</div> : null}
          {!loading && preview ? (
            <>
              <div className="admin-market-preview-metrics">
                <Card bordered><span>完整分钟数</span><strong>{preview.one_minute_count}</strong></Card>
                <Card bordered><span>返回采样数</span><strong>{preview.sample_count}</strong></Card>
                <Card bordered><span>预览版本</span><strong>V{preview.preview_version}</strong></Card>
                <Card bordered className="admin-market-preview-seed"><span>本次预览随机种子</span><strong>{preview.preview_seed}</strong></Card>
              </div>
              {values.seedMode === 'auto' && values.regenerateSeed ? (
                <Typography.Text type="tertiary">
                  当前选择了重新生成随机种子；本次随机种子只用于预览，正式提交新版本时会再次生成。
                </Typography.Text>
              ) : null}
              {preview.sample_count < preview.one_minute_count ? (
                <Typography.Text type="tertiary">采样 K 线，不代表连续分钟；完整区间共 {preview.one_minute_count} 分钟，当前展示 {preview.sample_count} 根。</Typography.Text>
              ) : null}
              <MarketStrategyPreviewChart samples={preview.samples ?? []} />
              <div aria-label="OHLCV 预览样本" className="admin-market-preview-grid" role="table">
                <div className="admin-market-preview-grid__row admin-market-preview-grid__head" role="row">
                  <span role="columnheader">时间</span><span role="columnheader">开</span><span role="columnheader">高</span><span role="columnheader">低</span><span role="columnheader">收</span><span role="columnheader">成交量</span>
                </div>
                {(preview.samples ?? []).map((sample) => (
                  <div className="admin-market-preview-grid__row" key={sample.open_time} role="row">
                    <time role="cell">{formatPreviewTime(sample.open_time)}</time>
                    <span role="cell">{sample.open}</span><span role="cell">{sample.high}</span><span role="cell">{sample.low}</span><strong role="cell">{sample.close}</strong><span role="cell">{sample.volume}</span>
                  </div>
                ))}
              </div>
            </>
          ) : null}
        </div>
      </SideSheet>
    </>
  );
}
