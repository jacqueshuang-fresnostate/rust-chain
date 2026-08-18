import { Button, Card, Descriptions, SideSheet, Space, Spin, Tag, TextArea, Toast } from '@douyinfe/semi-ui';
import { useCallback, useEffect, useRef, useState } from 'react';

import { apiRequest } from '../../api/client';
import { ResizableTable } from '../../shared/ResizableTable';

type GapRange = {
  one_minute_count: number;
  range_end: number;
  range_start: number;
};

type GapsResponse = {
  config_version: number;
  gaps: GapRange[];
  strategy_id: number;
  total_1m_count: number;
};

type RecoverySample = {
  close: string;
  high: string;
  low: string;
  open: string;
  open_time: number;
  volume: string;
};

type RecoveryPreview = {
  aggregate_intervals: string[];
  config_version: number;
  expires_at: number;
  first_price: string;
  last_price: string;
  one_minute_count: number;
  preview_token: string;
  range_end: number;
  range_start: number;
  samples: RecoverySample[];
  strategy_id: number;
};

type RecoveryJob = {
  actual_1m_count: number;
  actual_aggregate_count: number;
  completed_at?: number | null;
  config_version: number;
  created_at: number;
  error_message?: string | null;
  expected_1m_count: number;
  id: number;
  range_end: number;
  range_start: number;
  reason: string;
  status: string;
};

type RecoveryJobsResponse = {
  jobs: RecoveryJob[];
  total: number;
};

function formatTime(value: number | null | undefined): string {
  return value ? new Date(value).toLocaleString('zh-CN', { hour12: false }) : '—';
}

function statusLabel(status: string): string {
  return ({ pending: '等待中', running: '执行中', completed: '已完成', failed: '失败' } as Record<string, string>)[status] ?? status;
}

function statusColor(status: string): 'blue' | 'green' | 'red' | 'grey' {
  if (status === 'completed') return 'green';
  if (status === 'failed') return 'red';
  if (status === 'pending' || status === 'running') return 'blue';
  return 'grey';
}

export function MarketStrategyRecoverySheet({ strategyId }: { strategyId: string }) {
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [gaps, setGaps] = useState<GapsResponse | null>(null);
  const [preview, setPreview] = useState<RecoveryPreview | null>(null);
  const [jobs, setJobs] = useState<RecoveryJob[]>([]);
  const [jobsLoaded, setJobsLoaded] = useState(false);
  const [jobsLoading, setJobsLoading] = useState(false);
  const jobsRequestIdRef = useRef(0);
  const [reason, setReason] = useState('');
  const [errorMessage, setErrorMessage] = useState('');

  const loadJobs = useCallback(async () => {
    const requestId = ++jobsRequestIdRef.current;
    setJobsLoading(true);
    try {
      const result = await apiRequest<RecoveryJobsResponse>(`/admin/api/v1/market-strategies/${strategyId}/kline-recovery/jobs?limit=20&offset=0`);
      if (jobsRequestIdRef.current !== requestId) return;
      setJobs(Array.isArray(result.jobs) ? result.jobs : []);
      setJobsLoaded(true);
    } catch (error) {
      if (jobsRequestIdRef.current !== requestId) return;
      const message = error instanceof Error ? error.message : '加载补偿任务失败';
      setJobsLoaded(false);
      setErrorMessage(message);
      Toast.error(message);
    } finally {
      if (jobsRequestIdRef.current === requestId) {
        setJobsLoading(false);
      }
    }
  }, [strategyId]);

  async function detectGaps() {
    setLoading(true);
    setPreview(null);
    setErrorMessage('');
    try {
      const result = await apiRequest<GapsResponse>(`/admin/api/v1/market-strategies/${strategyId}/kline-gaps`);
      setGaps({ ...result, gaps: Array.isArray(result.gaps) ? result.gaps : [] });
      await loadJobs();
    } catch (error) {
      const message = error instanceof Error ? error.message : '检测缺口失败';
      setGaps(null);
      setErrorMessage(message);
      Toast.error(message);
    } finally {
      setLoading(false);
    }
  }

  async function previewGap(gap: GapRange) {
    setLoading(true);
    setPreview(null);
    setErrorMessage('');
    try {
      const result = await apiRequest<RecoveryPreview>(`/admin/api/v1/market-strategies/${strategyId}/kline-recovery/preview`, {
        method: 'POST',
        body: JSON.stringify({ range_start: gap.range_start, range_end: gap.range_end })
      });
      setPreview({ ...result, samples: Array.isArray(result.samples) ? result.samples : [] });
    } catch (error) {
      const message = error instanceof Error ? error.message : '生成补偿预览失败';
      setErrorMessage(message);
      Toast.error(message);
    } finally {
      setLoading(false);
    }
  }

  async function executeRecovery() {
    if (!preview || !reason.trim()) return;
    setSubmitting(true);
    setErrorMessage('');
    try {
      await apiRequest(`/admin/api/v1/market-strategies/${strategyId}/kline-recovery/execute`, {
        method: 'POST',
        body: JSON.stringify({ preview_token: preview.preview_token, reason: reason.trim() })
      });
      Toast.success('K线补偿任务已提交');
      setPreview(null);
      setReason('');
      await detectGaps();
    } catch (error) {
      const message = error instanceof Error ? error.message : '提交补偿任务失败';
      setErrorMessage(message);
      Toast.error(message);
    } finally {
      setSubmitting(false);
    }
  }

  useEffect(() => {
    if (visible) {
      void loadJobs();
      return;
    }
    jobsRequestIdRef.current += 1;
    setGaps(null);
    setPreview(null);
    setJobs([]);
    setJobsLoaded(false);
    setJobsLoading(false);
    setReason('');
    setErrorMessage('');
  }, [loadJobs, visible]);

  return (
    <>
      <Button aria-label={`检测缺口/补偿K线（策略${strategyId}）`} disabled={!strategyId} onClick={() => setVisible(true)} size="small" theme="borderless">
        检测缺口/补偿K线
      </Button>
      <SideSheet
        className="admin-create-modal admin-create-modal-extra-wide admin-market-recovery-sheet"
        closeOnEsc={!submitting}
        maskClosable={false}
        onCancel={() => !submitting && setVisible(false)}
        title="检测缺口与补偿K线"
        visible={visible}
        width="min(1120px, calc(100vw - 48px))"
      >
        <Space spacing={18} vertical style={{ width: '100%' }}>
          <div className="admin-market-recovery-toolbar">
            <div>
              <h3>策略 #{strategyId}</h3>
              <p>先检测缺失的已闭合 1 分钟 K 线，再选择范围预览；执行前必须填写审计原因。</p>
            </div>
            <Button aria-label="重新检测K线缺口" disabled={submitting} loading={loading} onClick={detectGaps} theme="solid" type="primary">检测缺口</Button>
          </div>

          {errorMessage ? <div aria-live="assertive" className="admin-market-recovery-state admin-market-recovery-error" role="alert">{errorMessage}</div> : null}
          {loading && !gaps ? <div aria-live="polite" className="admin-market-recovery-state"><Spin /> 正在检测缺口…</div> : null}
          {gaps ? (
            <Card title={`缺口范围（共 ${gaps.total_1m_count} 根 1m）`}>
              {gaps.gaps.length === 0 ? (
                <div aria-live="polite" className="admin-market-recovery-state">当前范围没有 K 线缺口。</div>
              ) : (
                <ResizableTable<GapRange>
                  columns={[
                    { title: '开始时间（含）', dataIndex: 'range_start', render: (value) => formatTime(Number(value)) },
                    { title: '结束时间（不含）', dataIndex: 'range_end', render: (value) => formatTime(Number(value)) },
                    { title: '缺失根数', dataIndex: 'one_minute_count' },
                    {
                      title: '操作',
                      key: 'actions',
                      render: (_, gap) => <Button aria-label={`预览缺口${gap.range_start}`} disabled={loading || submitting} onClick={() => previewGap(gap)} size="small">预览补偿</Button>
                    }
                  ]}
                  dataSource={gaps.gaps}
                  pagination={false}
                  rowKey="range_start"
                  size="small"
                />
              )}
            </Card>
          ) : null}

          {preview ? (
            <Card title="补偿预览">
              <Space spacing={16} vertical style={{ width: '100%' }}>
                <Descriptions
                  data={[
                    { key: '配置版本', value: String(preview.config_version) },
                    { key: '范围 [开始, 结束)', value: `${formatTime(preview.range_start)} 至 ${formatTime(preview.range_end)}` },
                    { key: '1m 根数', value: String(preview.one_minute_count) },
                    { key: '聚合周期', value: preview.aggregate_intervals.join('、') || '—' },
                    { key: '首尾价格', value: `${preview.first_price} → ${preview.last_price}` },
                    { key: '令牌有效期', value: formatTime(preview.expires_at) }
                  ]}
                  row
                  size="small"
                />
                <ResizableTable<RecoverySample>
                  columns={[
                    { title: '开盘时间', dataIndex: 'open_time', render: (value) => formatTime(Number(value)) },
                    { title: '开', dataIndex: 'open' },
                    { title: '高', dataIndex: 'high' },
                    { title: '低', dataIndex: 'low' },
                    { title: '收', dataIndex: 'close' },
                    { title: '成交量', dataIndex: 'volume' }
                  ]}
                  dataSource={preview.samples}
                  pagination={false}
                  rowKey="open_time"
                  size="small"
                />
                <label className="admin-market-recovery-reason">
                  补偿原因
                  <TextArea aria-label="补偿原因" autosize disabled={submitting} onChange={setReason} placeholder="请输入本次手动补偿的审计原因" value={reason} />
                </label>
                <Button aria-label="确认执行K线补偿" disabled={submitting || !reason.trim()} loading={submitting} onClick={executeRecovery} theme="solid" type="primary">
                  确认执行补偿
                </Button>
              </Space>
            </Card>
          ) : null}

          <Card title="补偿任务历史">
            {jobsLoading && !jobsLoaded ? (
              <div aria-live="polite" className="admin-market-recovery-state"><Spin /> 正在加载补偿任务…</div>
            ) : jobsLoaded && jobs.length === 0 ? (
              <div aria-live="polite" className="admin-market-recovery-state">暂无补偿任务。</div>
            ) : jobsLoaded ? (
              <ResizableTable<RecoveryJob>
                columns={[
                  { title: '任务ID', dataIndex: 'id' },
                  { title: '状态', dataIndex: 'status', render: (value) => <Tag color={statusColor(String(value))}>{statusLabel(String(value))}</Tag> },
                  { title: '范围 [开始, 结束)', key: 'range', render: (_, job) => `${formatTime(job.range_start)} 至 ${formatTime(job.range_end)}` },
                  { title: '1m 进度', key: 'progress', render: (_, job) => `${job.actual_1m_count}/${job.expected_1m_count}` },
                  { title: '聚合根数', dataIndex: 'actual_aggregate_count' },
                  { title: '原因/错误', key: 'message', render: (_, job) => job.error_message || job.reason },
                  { title: '创建时间', dataIndex: 'created_at', render: (value) => formatTime(Number(value)) }
                ]}
                dataSource={jobs}
                pagination={false}
                rowKey="id"
                size="small"
              />
            ) : null}
          </Card>
        </Space>
      </SideSheet>
    </>
  );
}
