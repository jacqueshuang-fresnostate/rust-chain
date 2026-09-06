import { adminErrorMessage } from '../../shared/adminErrorMessage';
import { Button, Card, SideSheet, Space, Tag, Toast, Typography } from '@douyinfe/semi-ui';
import { useCallback, useEffect, useRef, useState } from 'react';

import { apiRequest } from '../../api/client';
import { AdminRequestActionBoundary } from '../access';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { adminEnumLabel } from '../../shared/adminPresentation';

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
  return adminErrorMessage(error, '版本历史加载失败');
}

function dateTime(value: number): string {
  const date = new Date(Number(value));
  return Number.isFinite(date.getTime()) ? date.toLocaleString('zh-CN', { hour12: false }) : '--';
}

export function MarketStrategyVersionSheet({ onRestored, strategyId, strategyStatus }: { onRestored?: () => void; strategyId: string; strategyStatus: string }) {
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [versions, setVersions] = useState<VersionRecord[]>([]);
  const requestId = useRef(0);
  const [restoring, setRestoring] = useState(false);
  const canRestore = ['draft', 'paused', 'disabled'].includes(strategyStatus);

  const loadVersions = useCallback(async () => {
    if (!strategyId) return;
    const currentRequest = ++requestId.current;
    setLoading(true);
    setError('');
    try {
      const result = await apiRequest<VersionsResponse>(`/admin/api/v1/market-strategies/${strategyId}/versions?limit=100&offset=0`);
      if (currentRequest !== requestId.current) return;
      setVersions(Array.isArray(result.versions) ? result.versions : []);
    } catch (loadError) {
      if (currentRequest !== requestId.current) return;
      setError(messageOf(loadError));
    } finally {
      if (currentRequest === requestId.current) setLoading(false);
    }
  }, [strategyId]);

  useEffect(() => {
    if (visible) void loadVersions();
    return () => { requestId.current += 1; };
  }, [loadVersions, visible]);

  return (
    <>
      <Button disabled={!strategyId} onClick={() => setVisible(true)} size="small" theme="borderless">
        版本历史
      </Button>
      <SideSheet
        maskClosable={false}
        closeOnEsc={!restoring}
        onCancel={() => !restoring && setVisible(false)}
        title="行情策略版本历史"
        visible={visible}
        width={760}
      >
        <div className="admin-market-version-sheet" aria-busy={loading} aria-live="polite">
          <div className="admin-market-version-sheet__intro">
            <div>
              <Typography.Title heading={5}>不可变配置版本</Typography.Title>
              <Typography.Text type="tertiary">复制旧快照为新的当前配置，不修改历史版本与已生成 K 线，也不会自动启用策略。</Typography.Text>
            </div>
            <Button disabled={restoring} loading={loading} onClick={() => void loadVersions()} size="small">刷新</Button>
          </div>
          {!canRestore ? <p role="note">请先在列表暂停或禁用策略，再复制历史版本。当前状态：{adminEnumLabel('status', strategyStatus) ?? strategyStatus}</p> : null}
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
                      {version.active ? <Tag color="green">当前配置</Tag> : null}
                    </div>
                    {!version.active ? (
                      <AdminRequestActionBoundary endpoint={`/admin/api/v1/market-strategies/${strategyId}/versions/${version.version}/restore`} method="POST">
                        <ConfirmAction
                          actionText="复制为新版本"
                          disabled={!canRestore || loading || restoring}
                          title={`确认复制版本 ${version.version} 为当前配置（不自动启用）`}
                          onConfirm={async (reason) => {
                            if (!canRestore) throw new Error('请先暂停或禁用行情策略后再复制历史版本');
                            setRestoring(true);
                            try {
                              await apiRequest(`/admin/api/v1/market-strategies/${strategyId}/versions/${version.version}/restore`, {
                                method: 'POST',
                                body: JSON.stringify({ reason })
                              });
                              Toast.success(`已复制版本 ${version.version} 为新的当前配置，策略未自动启用`);
                              await loadVersions();
                              onRestored?.();
                            } finally { setRestoring(false); }
                          }}
                        />
                      </AdminRequestActionBoundary>
                    ) : null}
                  </div>
                  <dl className="admin-market-version-card__meta">
                    <div><dt>场景</dt><dd>{adminEnumLabel('scenario', scenario) ?? scenario}</dd></div>
                    <div><dt>随机种子模式</dt><dd>{adminEnumLabel('seed_mode', version.generator?.seed_mode) ?? String(version.generator?.seed_mode ?? '未记录')}</dd></div>
                    <div><dt>实际随机种子</dt><dd className="admin-market-version-card__seed">{version.seed}</dd></div>
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
