import { Button, Card, SideSheet, Space, Tag, Toast, Typography } from '@douyinfe/semi-ui';
import { useCallback, useEffect, useState } from 'react';

import { apiRequest } from '../../api/client';
import { AdminRequestActionBoundary } from '../access';
import { ConfirmAction } from '../../shared/ConfirmAction';

type GeneratorRecord = {
  scenario?: unknown;
  seed_mode?: unknown;
  mean_reversion_strength?: unknown;
  noise_scale?: unknown;
  wick_scale?: unknown;
  volume_shape?: unknown;
};

type VersionRecord = {
  version: number;
  effective_time: number;
  seed: string;
  created_by: number | null;
  created_at: number;
  active: boolean;
  generator: GeneratorRecord;
};

type VersionsResponse = {
  versions: VersionRecord[];
  total: number;
};

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : '版本历史加载失败';
}

function dateTime(value: number): string {
  const date = new Date(Number(value));
  return Number.isFinite(date.getTime()) ? date.toLocaleString('zh-CN', { hour12: false }) : '--';
}

const scenarioLabels: Record<string, string> = {
  custom_path: '自定义路径',
  trend_up: '稳步上涨',
  trend_down: '缓慢下行',
  range: '区间震荡',
  high_volatility: '高波动',
  crash_recovery: '急跌修复',
  pump_then_dump: '拉升回落'
};

export function MarketStrategyVersionSheet({ onRestored, strategyId }: { onRestored?: () => void; strategyId: string }) {
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [versions, setVersions] = useState<VersionRecord[]>([]);

  const loadVersions = useCallback(async () => {
    if (!strategyId) return;
    setLoading(true);
    setError('');
    try {
      const result = await apiRequest<VersionsResponse>(`/admin/api/v1/market-strategies/${strategyId}/versions?limit=100&offset=0`);
      setVersions(Array.isArray(result.versions) ? result.versions : []);
    } catch (loadError) {
      setError(messageOf(loadError));
    } finally {
      setLoading(false);
    }
  }, [strategyId]);

  useEffect(() => {
    if (visible) void loadVersions();
  }, [loadVersions, visible]);

  return (
    <>
      <Button disabled={!strategyId} onClick={() => setVisible(true)} size="small" theme="borderless">
        版本历史
      </Button>
      <SideSheet
        maskClosable={!loading}
        onCancel={() => setVisible(false)}
        title="行情策略版本历史"
        visible={visible}
        width={760}
      >
        <div className="admin-market-version-sheet" aria-busy={loading} aria-live="polite">
          <div className="admin-market-version-sheet__intro">
            <div>
              <Typography.Title heading={5}>不可变配置版本</Typography.Title>
              <Typography.Text type="tertiary">回滚会复制旧快照为新的递增版本，不修改历史版本与已生成 K 线。</Typography.Text>
            </div>
            <Button loading={loading} onClick={() => void loadVersions()} size="small">刷新</Button>
          </div>
          {error ? <div className="admin-inline-error" role="alert">{error}</div> : null}
          {!loading && !error && versions.length === 0 ? <div className="admin-empty-state">暂无版本记录</div> : null}
          <Space spacing={12} vertical style={{ width: '100%' }}>
            {versions.map((version) => {
              const scenario = String(version.generator?.scenario ?? 'custom_path');
              return (
                <Card key={version.version} className="admin-market-version-card" bordered>
                  <div className="admin-market-version-card__header">
                    <div>
                      <strong>版本 {version.version}</strong>
                      {version.active ? <Tag color="green">当前激活</Tag> : null}
                    </div>
                    {!version.active ? (
                      <AdminRequestActionBoundary endpoint={`/admin/api/v1/market-strategies/${strategyId}/versions/${version.version}/restore`} method="POST">
                        <ConfirmAction
                          actionText="复制为新版本"
                          title={`确认回滚到版本 ${version.version}`}
                          onConfirm={async (reason) => {
                            await apiRequest(`/admin/api/v1/market-strategies/${strategyId}/versions/${version.version}/restore`, {
                              method: 'POST',
                              body: JSON.stringify({ reason })
                            });
                            Toast.success(`已复制版本 ${version.version} 为新的激活版本`);
                            await loadVersions();
                            onRestored?.();
                          }}
                        />
                      </AdminRequestActionBoundary>
                    ) : null}
                  </div>
                  <dl className="admin-market-version-card__meta">
                    <div><dt>场景</dt><dd>{scenarioLabels[scenario] ?? scenario}</dd></div>
                    <div><dt>Seed 模式</dt><dd>{String(version.generator?.seed_mode ?? 'auto') === 'fixed' ? '固定' : '自动'}</dd></div>
                    <div><dt>实际 Seed</dt><dd className="admin-market-version-card__seed">{version.seed}</dd></div>
                    <div><dt>生效时间</dt><dd>{dateTime(version.effective_time)}</dd></div>
                    <div><dt>创建时间</dt><dd>{dateTime(version.created_at)}</dd></div>
                    <div><dt>创建管理员</dt><dd>{version.created_by ?? '系统迁移'}</dd></div>
                  </dl>
                </Card>
              );
            })}
          </Space>
        </div>
      </SideSheet>
    </>
  );
}
