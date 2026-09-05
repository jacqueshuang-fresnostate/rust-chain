import { IconDownload, IconRefresh, IconSearch } from '@douyinfe/semi-icons';
import { Button, Card, Empty, Pagination, Space, Spin, Tag, Tooltip, Typography } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { type FormEvent, useMemo, useState } from 'react';
import { Link, useLocation } from 'react-router-dom';

import { ApiError } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { AdminTextInput } from '../../shared/SemiFormControls';
import { safeSingleLineText } from '../../shared/sensitiveText';
import { TimestampText } from '../../shared/TimestampText';
import {
  type AdminAuditLog,
  type AdminAuditLogsQuery,
  getAdminAuditLogs,
  localDateTimeToUnixMillis
} from './auditApi';
import { downloadCurrentAuditLogs } from './auditExport';
import {
  auditActionLabel,
  auditTargetHref,
  auditTargetLabel,
  buildAuditFieldChanges,
  redactAuditFreeText
} from './auditPresentation';

import './AuditLogsPage.css';

const { Text, Title } = Typography;
const DEFAULT_PAGE_SIZE = 20;
const PAGE_SIZE_OPTIONS = [10, 20, 50, 100];

type AuditFilterDraft = {
  action: string;
  adminId: string;
  createdFrom: string;
  createdTo: string;
  targetId: string;
  targetType: string;
};

type AppliedAuditFilters = Omit<AdminAuditLogsQuery, 'limit' | 'offset'>;

const EMPTY_FILTERS: AuditFilterDraft = {
  action: '',
  adminId: '',
  createdFrom: '',
  createdTo: '',
  targetId: '',
  targetType: ''
};

function normalizeAuditFilters(draft: AuditFilterDraft): { error: string | null; filters: AppliedAuditFilters } {
  const adminId = draft.adminId.trim();
  if (adminId && !/^[1-9]\d*$/u.test(adminId)) {
    return { error: '管理员 ID 必须是大于 0 的整数。', filters: {} };
  }

  const createdFrom = localDateTimeToUnixMillis(draft.createdFrom);
  const createdTo = localDateTimeToUnixMillis(draft.createdTo);
  if (draft.createdFrom && createdFrom === undefined) {
    return { error: '起始时间格式无效，请重新选择。', filters: {} };
  }
  if (draft.createdTo && createdTo === undefined) {
    return { error: '结束时间格式无效，请重新选择。', filters: {} };
  }
  if (createdFrom !== undefined && createdTo !== undefined && createdFrom > createdTo) {
    return { error: '起始时间不得晚于结束时间。', filters: {} };
  }

  const action = draft.action.trim();
  const targetType = draft.targetType.trim();
  const targetId = draft.targetId.trim();
  return {
    error: null,
    filters: {
      ...(adminId ? { admin_id: adminId } : {}),
      ...(action ? { action } : {}),
      ...(targetType ? { target_type: targetType } : {}),
      ...(targetId ? { target_id: targetId } : {}),
      ...(createdFrom !== undefined ? { created_from: createdFrom } : {}),
      ...(createdTo !== undefined ? { created_to: createdTo } : {})
    }
  };
}

function safeErrorMessage(error: unknown): string {
  const message = error instanceof ApiError || error instanceof Error ? error.message : '审计日志加载失败';
  return safeSingleLineText(message, '审计日志加载失败');
}

function AuditFieldChanges({ log }: { log: AdminAuditLog }) {
  const changes = useMemo(
    () => buildAuditFieldChanges(log.before_json, log.after_json),
    [log.after_json, log.before_json]
  );

  return (
    <section aria-label={`日志 ${log.id} 字段变化`} className="audit-log-changes">
      <div className="audit-log-section-heading">
        <Title heading={5}>字段变化</Title>
        <Tag color={changes.length > 0 ? 'light-blue' : 'grey'}>{changes.length} 项</Tag>
      </div>
      {changes.length === 0 ? (
        <div aria-live="polite" className="audit-log-no-diff">
          {log.before_json === null && log.after_json === null
            ? '未记录前后快照，暂无字段差异。'
            : '前后快照一致，未发现字段变化。'}
        </div>
      ) : (
        <ul className="audit-log-change-list">
          {changes.map((change) => (
            <li className="audit-log-change" key={`${change.path}-${change.before}-${change.after}`}>
              <div className="audit-log-change__field">
                <Text strong>{change.label}</Text>
                {change.sensitive ? <Tag color="orange">已遮罩</Tag> : null}
              </div>
              <div className="audit-log-change__values">
                <span className="audit-log-change__value is-before">
                  <small>旧值</small>
                  <span>{change.before}</span>
                </span>
                <span aria-hidden="true" className="audit-log-change__arrow">→</span>
                <span className="audit-log-change__value is-after">
                  <small>新值</small>
                  <span>{change.after}</span>
                </span>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function AuditLogCard({ log }: { log: AdminAuditLog }) {
  const targetLabel = auditTargetLabel(log.target_type);
  const targetHref = auditTargetHref(log.target_type);
  const reason = redactAuditFreeText(log.reason?.trim() ?? '');
  const actionCode = redactAuditFreeText(log.action);
  const targetId = redactAuditFreeText(log.target_id?.trim() || '-');
  const targetType = redactAuditFreeText(log.target_type);
  const ip = redactAuditFreeText(log.ip?.trim() || '未记录');
  const requestId = redactAuditFreeText(log.request_id?.trim() || '未记录');

  return (
    <li>
      <article aria-labelledby={`audit-log-${log.id}-title`} className="audit-log-card">
        <header className="audit-log-card__header">
          <div className="audit-log-card__title">
            <Tag color="orange">{targetLabel}</Tag>
            <Title heading={4} id={`audit-log-${log.id}-title`}>
              {auditActionLabel(log.action, log.target_type)}
            </Title>
            <Text className="audit-log-card__code" type="tertiary">动作代码：{actionCode}</Text>
          </div>
          <div className="audit-log-card__time">
            <Text type="tertiary">发生时间</Text>
            <TimestampText value={log.created_at} />
          </div>
        </header>

        <div className="audit-log-card__summary">
          <div className="audit-log-object">
            <Text type="tertiary">操作对象</Text>
            {targetHref ? (
              <Link aria-label={`查看${targetLabel} #${targetId}`} to={targetHref}>
                {targetLabel} #{targetId}
              </Link>
            ) : (
              <Text>{targetLabel} #{targetId}</Text>
            )}
            <Text className="audit-log-card__code" type="tertiary">对象类型：{targetType}</Text>
          </div>
          <div className="audit-log-reason">
            <Text type="tertiary">操作原因</Text>
            <Text>{reason || '未填写原因'}</Text>
          </div>
        </div>

        <AuditFieldChanges log={log} />

        <dl className="audit-log-trace" aria-label={`日志 ${log.id} 请求追踪`}>
          <div><dt>管理员</dt><dd>管理员 #{log.admin_id}</dd></div>
          <div><dt>来源 IP</dt><dd>{ip}</dd></div>
          <div><dt>Request ID</dt><dd><code>{requestId}</code></dd></div>
          <div><dt>日志 ID</dt><dd>#{log.id}</dd></div>
        </dl>
      </article>
    </li>
  );
}

export function AuditLogsPage() {
  const { search } = useLocation();
  const [initialTarget] = useState(() => {
    const params = new URLSearchParams(search);
    return { ...EMPTY_FILTERS, targetType: params.get('target_type') ?? '', targetId: params.get('target_id') ?? '' };
  });
  const [appliedFilters, setAppliedFilters] = useState<AppliedAuditFilters>(() => normalizeAuditFilters(initialTarget).filters);
  const [draftFilters, setDraftFilters] = useState<AuditFilterDraft>(initialTarget);
  const [filterError, setFilterError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);

  const query = useMemo<AdminAuditLogsQuery>(
    () => ({
      ...appliedFilters,
      limit: pageSize,
      offset: (page - 1) * pageSize
    }),
    [appliedFilters, page, pageSize]
  );
  const logsQuery = useQuery({
    queryKey: ['admin-audit-logs', query],
    queryFn: () => getAdminAuditLogs(query),
    retry: false
  });

  function updateDraft<Key extends keyof AuditFilterDraft>(key: Key, value: AuditFilterDraft[Key]) {
    setDraftFilters((current) => ({ ...current, [key]: value }));
  }

  function submitFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const normalized = normalizeAuditFilters(draftFilters);
    setFilterError(normalized.error);
    if (normalized.error) {
      return;
    }
    setAppliedFilters(normalized.filters);
    setPage(1);
  }

  function resetFilters() {
    setDraftFilters(EMPTY_FILTERS);
    setAppliedFilters({});
    setFilterError(null);
    setPage(1);
  }

  const response = logsQuery.data;
  const activeFilterCount = Object.keys(appliedFilters).length;
  const resultDescription = response
    ? `共 ${response.total} 条记录，${activeFilterCount > 0 ? `已启用 ${activeFilterCount} 个筛选` : '未启用筛选'}`
    : '按操作对象、管理员与发生时间检索审计记录';

  return (
    <main className="exchange-page audit-logs-page">
      <PageHeader
        actions={(
          <Space spacing={8}>
            <Tooltip content="导出当前页已加载的筛选结果；字段差异仅包含安全遮罩后的可读值">
              <Button
                disabled={!response?.logs.length}
                icon={<IconDownload aria-hidden="true" />}
                onClick={() => downloadCurrentAuditLogs(response?.logs ?? [])}
              >
                导出当前结果
              </Button>
            </Tooltip>
            <Button
              icon={<IconRefresh aria-hidden="true" />}
              loading={logsQuery.isFetching}
              onClick={() => void logsQuery.refetch()}
            >
              刷新日志
            </Button>
          </Space>
        )}
        description="以中文字段差异核对管理员操作，并保留完整请求追踪信息"
        title="审计日志"
      />

      <Card bordered={false} className="audit-log-filter-card">
        <div className="audit-log-filter-heading">
          <div>
            <Title heading={4}>筛选审计记录</Title>
            <Text type="tertiary">动作、对象与管理员采用精确匹配；起止时间边界均包含。</Text>
          </div>
          <Text type="tertiary">{resultDescription}</Text>
        </div>
        <form className="audit-log-filters" onSubmit={submitFilters}>
          <label>
            管理员 ID
            <AdminTextInput
              ariaLabel="管理员 ID"
              disabled={logsQuery.isFetching}
              onChange={(adminId) => updateDraft('adminId', adminId)}
              placeholder="如 1001"
              value={draftFilters.adminId}
            />
          </label>
          <label>
            动作
            <AdminTextInput
              ariaLabel="动作"
              disabled={logsQuery.isFetching}
              onChange={(action) => updateDraft('action', action)}
              placeholder="如 asset.config.update"
              value={draftFilters.action}
            />
          </label>
          <label>
            对象类型
            <AdminTextInput
              ariaLabel="对象类型"
              disabled={logsQuery.isFetching}
              onChange={(targetType) => updateDraft('targetType', targetType)}
              placeholder="如 asset"
              value={draftFilters.targetType}
            />
          </label>
          <label>
            对象 ID
            <AdminTextInput
              ariaLabel="对象 ID"
              disabled={logsQuery.isFetching}
              onChange={(targetId) => updateDraft('targetId', targetId)}
              placeholder="支持数字或业务标识"
              value={draftFilters.targetId}
            />
          </label>
          <label>
            起始时间（含）
            <input
              aria-describedby="audit-time-range-help"
              aria-label="起始时间"
              className="audit-log-native-input"
              disabled={logsQuery.isFetching}
              onChange={(event) => updateDraft('createdFrom', event.target.value)}
              step="1"
              type="datetime-local"
              value={draftFilters.createdFrom}
            />
          </label>
          <label>
            结束时间（含）
            <input
              aria-describedby="audit-time-range-help"
              aria-label="结束时间"
              className="audit-log-native-input"
              disabled={logsQuery.isFetching}
              onChange={(event) => updateDraft('createdTo', event.target.value)}
              step="1"
              type="datetime-local"
              value={draftFilters.createdTo}
            />
          </label>
          <p className="audit-log-time-help" id="audit-time-range-help">
            按当前浏览器时区输入，查询时转换为 Unix 毫秒；开始与结束时刻都计入结果。
          </p>
          <div className="audit-log-filter-actions">
            <Button
              disabled={logsQuery.isFetching}
              htmlType="submit"
              icon={<IconSearch aria-hidden="true" />}
              theme="solid"
              type="primary"
            >
              查询审计日志
            </Button>
            <Button disabled={logsQuery.isFetching} onClick={resetFilters} theme="borderless">
              重置
            </Button>
          </div>
        </form>
        {filterError ? <div className="audit-log-filter-error" role="alert">{filterError}</div> : null}
      </Card>

      <Card bordered={false} className="audit-log-results-card">
        {logsQuery.isPending ? (
          <div aria-live="polite" className="audit-log-state" role="status">
            <Spin size="large" />
            <Text>正在加载审计日志…</Text>
          </div>
        ) : null}
        {logsQuery.error ? (
          <div className="audit-log-state is-error" role="alert">
            <Title heading={4}>审计日志加载失败</Title>
            <Text type="danger">{safeErrorMessage(logsQuery.error)}</Text>
            <Button onClick={() => void logsQuery.refetch()}>重新加载</Button>
          </div>
        ) : null}
        {!logsQuery.isPending && !logsQuery.error && response?.logs.length === 0 ? (
          <div aria-live="polite" className="audit-log-state" role="status">
            <Empty description="没有符合条件的审计日志" />
            <Text type="tertiary">可放宽时间范围或清除动作、对象与管理员筛选后重试。</Text>
          </div>
        ) : null}
        {!logsQuery.error && response && response.logs.length > 0 ? (
          <>
            <ol aria-label="审计日志列表" className="audit-log-list">
              {response.logs.map((log) => <AuditLogCard key={log.id} log={log} />)}
            </ol>
            <div className="audit-log-pagination">
              <Pagination
                currentPage={page}
                disabled={logsQuery.isFetching}
                onPageChange={setPage}
                onPageSizeChange={(nextPageSize) => {
                  setPageSize(nextPageSize);
                  setPage(1);
                }}
                pageSize={pageSize}
                pageSizeOpts={PAGE_SIZE_OPTIONS}
                showSizeChanger
                showTotal
                total={response.total}
              />
            </div>
          </>
        ) : null}
      </Card>
    </main>
  );
}
