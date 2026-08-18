import { IconRefresh, IconSearch } from '@douyinfe/semi-icons';
import { Button, Card, Empty, Space, Spin, Tag, Typography } from '@douyinfe/semi-ui';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';

import { apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import { safeSingleLineText } from '../../shared/sensitiveText';

import './ConfigCenterPage.css';

const { Text, Title } = Typography;

type ConfigCenterItem = {
  applied_version: number | null;
  code: string;
  config_path: string;
  config_status: string;
  configured_count: number;
  group: string;
  group_name: string;
  last_applied_at: number | null;
  last_error_summary: string | null;
  last_modified_at: number | null;
  last_tested_at: number | null;
  name: string;
  operation_path: string | null;
  published_version: number | null;
  runtime_status: string;
};

type ConfigCenterSummary = {
  normal: number;
  pending_apply: number;
  runtime_error: number;
  total: number;
  unconfigured: number;
};

type ConfigCenterResponse = {
  items: ConfigCenterItem[];
  summary: ConfigCenterSummary;
  total: number;
};

type ConfigCenterFilters = {
  group: string;
  query: string;
  status: string;
};

const emptyFilters: ConfigCenterFilters = { group: '', query: '', status: '' };

const configStatusMeta: Record<string, { color: 'green' | 'orange' | 'red' | 'grey'; label: string }> = {
  normal: { color: 'green', label: '正常' },
  pending_apply: { color: 'orange', label: '待应用' },
  runtime_error: { color: 'red', label: '运行异常' },
  unconfigured: { color: 'grey', label: '未配置' }
};

const runtimeStatusLabels: Record<string, string> = {
  error: '运行异常',
  healthy: '健康',
  not_applicable: '无需运行态',
  running: '运行中',
  stopped: '已停止',
  unknown: '状态未知'
};

function buildConfigCenterPath(filters: ConfigCenterFilters): string {
  const params = new URLSearchParams();
  if (filters.query.trim()) {
    params.set('query', filters.query.trim());
  }
  if (filters.group) {
    params.set('group', filters.group);
  }
  if (filters.status) {
    params.set('status', filters.status);
  }
  const query = params.toString();
  return `/admin/api/v1/config-center${query ? `?${query}` : ''}`;
}

function safeErrorMessage(error: unknown): string {
  return safeSingleLineText(
    error instanceof Error ? error.message : '',
    '配置中心加载失败'
  );
}

function formatTime(value: number | null): string {
  if (!value) {
    return '暂无记录';
  }
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  }).format(new Date(value));
}

function versionText(item: ConfigCenterItem): string {
  if (item.published_version === null && item.applied_version === null) {
    return '不适用版本发布';
  }
  return `发布 v${item.published_version ?? '-'} / 已应用 v${item.applied_version ?? '-'}`;
}

export function ConfigCenterPage() {
  const [appliedFilters, setAppliedFilters] = useState<ConfigCenterFilters>(emptyFilters);
  const [draftFilters, setDraftFilters] = useState<ConfigCenterFilters>(emptyFilters);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [response, setResponse] = useState<ConfigCenterResponse | null>(null);
  const [revision, setRevision] = useState(0);

  const load = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      setResponse(await apiRequest<ConfigCenterResponse>(buildConfigCenterPath(appliedFilters)));
    } catch (requestError) {
      setError(safeErrorMessage(requestError));
    } finally {
      setLoading(false);
    }
  }, [appliedFilters]);

  useEffect(() => {
    void load();
  }, [load, revision]);

  const groups = useMemo(() => {
    const result = new Map<string, ConfigCenterItem[]>();
    response?.items.forEach((item) => {
      result.set(item.group_name, [...(result.get(item.group_name) ?? []), item]);
    });
    return [...result.entries()];
  }, [response]);

  const summary = response?.summary ?? {
    normal: 0,
    pending_apply: 0,
    runtime_error: 0,
    total: 0,
    unconfigured: 0
  };

  return (
    <main className="exchange-page config-center-page">
      <PageHeader
        description="统一查看配置、发布版本、运行状态与最近验证结果"
        title="配置中心"
        actions={(
          <Button icon={<IconRefresh />} loading={loading} onClick={() => setRevision((value) => value + 1)}>
            刷新状态
          </Button>
        )}
      />

      <section aria-label="配置状态摘要" className="config-center-summary">
        {[
          ['配置域', summary.total, 'total'],
          ['正常', summary.normal, 'normal'],
          ['待应用', summary.pending_apply, 'pending'],
          ['运行异常', summary.runtime_error, 'error'],
          ['未配置', summary.unconfigured, 'empty']
        ].map(([label, value, tone]) => (
          <Card bordered={false} className={`config-center-summary__card is-${tone}`} key={String(label)}>
            <Text type="tertiary">{label}</Text>
            <strong>{value}</strong>
          </Card>
        ))}
      </section>

      <Card bordered={false} className="config-center-filter-card">
        <div className="config-center-filters">
          <label>
            搜索配置
            <AdminTextInput
              ariaLabel="搜索配置"
              onChange={(query) => setDraftFilters((current) => ({ ...current, query }))}
              placeholder="输入配置名称或代码"
              value={draftFilters.query}
            />
          </label>
          <label>
            业务分组
            <AdminSelect
              ariaLabel="业务分组"
              onChange={(group) => setDraftFilters((current) => ({ ...current, group }))}
              optionList={[
                { label: '全部分组', value: '' },
                { label: '行情与交易', value: 'market' },
                { label: '合规与安全', value: 'compliance' },
                { label: '产品配置', value: 'products' },
                { label: '平台集成', value: 'platform' }
              ]}
              value={draftFilters.group}
            />
          </label>
          <label>
            配置状态
            <AdminSelect
              ariaLabel="配置状态"
              onChange={(status) => setDraftFilters((current) => ({ ...current, status }))}
              optionList={[
                { label: '全部状态', value: '' },
                { label: '正常', value: 'normal' },
                { label: '待应用', value: 'pending_apply' },
                { label: '运行异常', value: 'runtime_error' },
                { label: '未配置', value: 'unconfigured' }
              ]}
              value={draftFilters.status}
            />
          </label>
          <Space className="config-center-filters__actions">
            <Button
              icon={<IconSearch />}
              onClick={() => setAppliedFilters({ ...draftFilters })}
              theme="solid"
              type="primary"
            >
              查询
            </Button>
            <Button
              onClick={() => {
                setDraftFilters(emptyFilters);
                setAppliedFilters(emptyFilters);
              }}
              theme="borderless"
            >
              重置
            </Button>
          </Space>
        </div>
      </Card>

      {loading && !response ? (
        <div aria-live="polite" className="config-center-state"><Spin size="large" /><Text>正在聚合配置状态…</Text></div>
      ) : null}
      {error ? (
        <Card bordered={false} className="config-center-state is-error">
          <Title heading={5}>配置状态加载失败</Title>
          <Text type="danger">{error}</Text>
          <Button onClick={() => setRevision((value) => value + 1)}>重新加载</Button>
        </Card>
      ) : null}
      {!loading && !error && response?.items.length === 0 ? (
        <Card bordered={false} className="config-center-state"><Empty description="没有符合条件的配置域" /></Card>
      ) : null}

      {!error ? groups.map(([groupName, items]) => (
        <section className="config-center-group" key={groupName}>
          <div className="config-center-group__heading">
            <Title heading={4}>{groupName}</Title>
            <Text type="tertiary">{items.length} 个配置域</Text>
          </div>
          <div className="config-center-grid">
            {items.map((item) => {
              const status = configStatusMeta[item.config_status] ?? configStatusMeta.unconfigured;
              return (
                <Card bordered={false} className="config-center-item" key={item.code}>
                  <div className="config-center-item__heading">
                    <div>
                      <Text className="config-center-item__code" type="tertiary">{item.code}</Text>
                      <Title heading={5}>{item.name}</Title>
                    </div>
                    <Tag color={status.color} data-config-status={item.config_status}>{status.label}</Tag>
                  </div>
                  <div className="config-center-item__metrics">
                    <span><Text type="tertiary">已配置</Text><strong>{item.configured_count}</strong></span>
                    <span><Text type="tertiary">运行状态</Text><strong>{runtimeStatusLabels[item.runtime_status] ?? item.runtime_status}</strong></span>
                  </div>
                  <div className="config-center-item__timeline">
                    <Text>{versionText(item)}</Text>
                    <Text type="tertiary">最后修改：{formatTime(item.last_modified_at)}</Text>
                    <Text type="tertiary">最后应用：{formatTime(item.last_applied_at)}</Text>
                    <Text type="tertiary">最近测试：{formatTime(item.last_tested_at)}</Text>
                  </div>
                  {item.last_error_summary ? <Text className="config-center-item__error" type="danger">{item.last_error_summary}</Text> : null}
                  <Space className="config-center-item__actions">
                    <Link className="semi-button semi-button-primary semi-button-light" to={item.config_path}>进入配置</Link>
                    {item.operation_path ? <Link className="semi-button semi-button-tertiary semi-button-borderless" to={item.operation_path}>运行与处置</Link> : null}
                  </Space>
                </Card>
              );
            })}
          </div>
        </section>
      )) : null}
    </main>
  );
}
