import { Banner, Button, Card, Descriptions, Space, Typography } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { Link, useParams } from 'react-router-dom';

import { apiRequest, ContractError } from '../../api/client';
import { authStore } from '../../auth/authStore';
import { PageHeader } from '../../layouts/PageHeader';
import { adminErrorMessage } from '../../shared/adminErrorMessage';
import { AmountText } from '../../shared/AmountText';
import { canonicalDecimalText } from '../../shared/decimal';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';
import { useCanAdminRequest } from '../access';

const { Text } = Typography;

const DECIMAL_FIELDS = [
  'total_supply',
  'reserved_supply',
  'allocated_supply',
  'remaining_supply',
  'supply_delta',
  'requested_quantity',
  'subscription_allocated_quantity',
  'distribution_quantity',
  'linked_distribution_quantity',
  'unlinked_distribution_quantity',
  'distribution_ledger_quantity',
  'subscription_distribution_delta',
  'distribution_ledger_delta',
  'manual_quote_amount',
  'manual_frozen_quote_amount',
  'manual_settled_quote_amount',
  'manual_refunded_quote_amount',
  'manual_quote_delta'
] as const;

const COUNT_FIELDS = [
  'subscription_count',
  'pending_manual_count',
  'invalid_subscription_link_count',
  'anomaly_count'
] as const;

const LIFECYCLE_STATUSES = new Set(['preheat', 'subscription', 'distribution', 'listed']);
const RECONCILIATION_STATUSES = new Set(['attention', 'balanced']);

export type NewCoinReconciliation = {
  project_id: number;
  symbol: string;
  lifecycle_status: string;
  total_supply: string;
  reserved_supply: string;
  allocated_supply: string;
  remaining_supply: string;
  supply_delta: string;
  subscription_count: number;
  pending_manual_count: number;
  requested_quantity: string;
  subscription_allocated_quantity: string;
  distribution_quantity: string;
  linked_distribution_quantity: string;
  unlinked_distribution_quantity: string;
  distribution_ledger_quantity: string;
  invalid_subscription_link_count: number;
  subscription_distribution_delta: string;
  distribution_ledger_delta: string;
  manual_quote_amount: string;
  manual_frozen_quote_amount: string;
  manual_settled_quote_amount: string;
  manual_refunded_quote_amount: string;
  manual_quote_delta: string;
  anomaly_count: number;
  anomalies: string[];
  status: 'attention' | 'balanced';
  checked_at: number;
};

function reconciliationPath(projectId: string): string {
  return `/admin/api/v1/new-coins/${projectId}/reconciliation`;
}

function reconciliationContractError(path: string): never {
  throw new ContractError('新币派发与退款对账响应不完整，请刷新或联系管理员', { path });
}

/**
 * 对账页不会把金额转为 JavaScript Number：这里只验证协议形状，
 * 并原样保留后端 Decimal 文本和 Unix 毫秒时间。
 */
export function parseNewCoinReconciliation(
  value: unknown,
  projectId: string,
  path = reconciliationPath(projectId)
): NewCoinReconciliation {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return reconciliationContractError(path);
  const record = value as Record<string, unknown>;

  if (
    !Number.isSafeInteger(record.project_id) ||
    Number(record.project_id) <= 0 ||
    String(record.project_id) !== projectId ||
    typeof record.symbol !== 'string' ||
    record.symbol.trim().length === 0 ||
    typeof record.lifecycle_status !== 'string' ||
    !LIFECYCLE_STATUSES.has(record.lifecycle_status) ||
    typeof record.status !== 'string' ||
    !RECONCILIATION_STATUSES.has(record.status)
  ) {
    return reconciliationContractError(path);
  }

  for (const field of DECIMAL_FIELDS) {
    if (typeof record[field] !== 'string' || canonicalDecimalText(record[field]) === null) {
      return reconciliationContractError(path);
    }
  }
  for (const field of COUNT_FIELDS) {
    if (!Number.isSafeInteger(record[field]) || Number(record[field]) < 0) {
      return reconciliationContractError(path);
    }
  }
  if (
    !Array.isArray(record.anomalies) ||
    !record.anomalies.every((anomaly) => typeof anomaly === 'string' && anomaly.trim().length > 0) ||
    record.anomaly_count !== record.anomalies.length ||
    (record.status === 'balanced') !== (record.anomaly_count === 0) ||
    !Number.isSafeInteger(record.checked_at) ||
    Number(record.checked_at) <= 0
  ) {
    return reconciliationContractError(path);
  }

  return record as NewCoinReconciliation;
}

export async function loadNewCoinReconciliation(
  projectId: string,
  signal?: AbortSignal
): Promise<NewCoinReconciliation> {
  const path = reconciliationPath(projectId);
  const response = await apiRequest<unknown>(path, { signal });
  return parseNewCoinReconciliation(response, projectId, path);
}

function reconciliationQueryKey(projectId: string) {
  const session = authStore.getSession('admin');
  return [
    'admin-new-coin-reconciliation',
    session ? `${session.subject}:${session.generation}` : 'anonymous:none',
    projectId
  ];
}

function MetricValue({ value }: { value: string }) {
  return <AmountText appendAsset={false} value={value} />;
}

function RelatedLink({ children, endpoint, to }: { children: ReactNode; endpoint: string; to: string }) {
  const canRead = useCanAdminRequest(endpoint, 'GET');
  return canRead ? <Link to={to}>{children}</Link> : null;
}

export function NewCoinReconciliationPage() {
  const { projectId = '' } = useParams();
  const validProjectId = /^[1-9]\d*$/.test(projectId);
  const query = useQuery({
    enabled: validProjectId,
    queryFn: ({ signal }) => loadNewCoinReconciliation(projectId, signal),
    queryKey: reconciliationQueryKey(projectId),
    refetchOnWindowFocus: false,
    retry: false,
    throwOnError: false
  });

  if (!validProjectId) {
    return (
      <main className="exchange-page">
        <PageHeader title="项目编号无效" />
        <RelatedLink endpoint="/admin/api/v1/new-coins" to="/admin/new-coins/projects">
          返回项目管理
        </RelatedLink>
      </main>
    );
  }

  if (query.isPending) {
    return (
      <main className="exchange-page">
        <PageHeader title="新币派发与退款对账" />
        <section aria-live="polite" className="admin-table-state">正在读取对账快照…</section>
      </main>
    );
  }

  const reconciliation = query.data;
  if (!reconciliation) {
    return (
      <main className="exchange-page">
        <PageHeader title="新币派发与退款对账" />
        <Banner
          closeIcon={null}
          description={adminErrorMessage(query.error, '对账快照加载失败')}
          icon={null}
          type="danger"
        />
        <Button loading={query.isFetching} onClick={() => void query.refetch()}>重新加载</Button>
      </main>
    );
  }

  const supplyData = [
    { key: '发行总量', value: <MetricValue value={reconciliation.total_supply} /> },
    { key: '已预留发行量', value: <MetricValue value={reconciliation.reserved_supply} /> },
    { key: '已分配发行量', value: <MetricValue value={reconciliation.allocated_supply} /> },
    { key: '剩余发行量', value: <MetricValue value={reconciliation.remaining_supply} /> },
    { key: '供给差额', value: <MetricValue value={reconciliation.supply_delta} /> }
  ];
  const subscriptionData = [
    { key: '申购笔数', value: reconciliation.subscription_count },
    { key: '待派发 / 退款', value: reconciliation.pending_manual_count },
    { key: '申购总量', value: <MetricValue value={reconciliation.requested_quantity} /> },
    { key: '申购实际获配', value: <MetricValue value={reconciliation.subscription_allocated_quantity} /> },
    { key: '关联申购的派发量', value: <MetricValue value={reconciliation.linked_distribution_quantity} /> },
    { key: '未关联申购的派发量', value: <MetricValue value={reconciliation.unlinked_distribution_quantity} /> },
    { key: '无效申购关联数', value: reconciliation.invalid_subscription_link_count },
    { key: '申购与派发差额', value: <MetricValue value={reconciliation.subscription_distribution_delta} /> }
  ];
  const distributionData = [
    { key: '派发总量', value: <MetricValue value={reconciliation.distribution_quantity} /> },
    { key: '钱包入账派发量', value: <MetricValue value={reconciliation.distribution_ledger_quantity} /> },
    { key: '派发与钱包入账差额', value: <MetricValue value={reconciliation.distribution_ledger_delta} /> }
  ];
  const quoteData = [
    { key: '人工申购计价总额', value: <MetricValue value={reconciliation.manual_quote_amount} /> },
    { key: '待结算冻结', value: <MetricValue value={reconciliation.manual_frozen_quote_amount} /> },
    { key: '实际扣款', value: <MetricValue value={reconciliation.manual_settled_quote_amount} /> },
    { key: '已退回差额', value: <MetricValue value={reconciliation.manual_refunded_quote_amount} /> },
    { key: '人工申购资金差额', value: <MetricValue value={reconciliation.manual_quote_delta} /> }
  ];

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <Space wrap>
            <RelatedLink
              endpoint={`/admin/api/v1/new-coins/${projectId}`}
              to={`/admin/new-coins/projects/${projectId}`}
            >
              返回项目中心
            </RelatedLink>
            <Button loading={query.isFetching} onClick={() => void query.refetch()}>刷新对账</Button>
          </Space>
        }
        description={
          <>
            项目 ID {reconciliation.project_id} · 生命周期 <StatusTag value={reconciliation.lifecycle_status} /> · 检查时间 <TimestampText value={reconciliation.checked_at} />
          </>
        }
        title={`${reconciliation.symbol} · 派发与退款对账`}
      />

      {query.error ? (
        <Banner
          closeIcon={null}
          description={`${adminErrorMessage(query.error, '刷新对账快照失败')}；已保留上一份快照。`}
          icon={null}
          type="warning"
        />
      ) : null}

      <Card bordered={false} shadows="always">
        <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
          <Space wrap>
            <Text strong>对账结果</Text>
            <StatusTag value={reconciliation.status} />
            <Text type="secondary">异常项 {reconciliation.anomaly_count} 条
            </Text>
          </Space>
          <Banner
            closeIcon={null}
            description={
              reconciliation.status === 'balanced'
                ? '本次快照中的发行供给、申购派发、钱包入账和人工申购资金均保持守恒。'
                : `发现 ${reconciliation.anomaly_count} 项差异，请按下方异常项核对原始申购、派发与钱包流水。`
            }
            icon={null}
            type={reconciliation.status === 'balanced' ? 'success' : 'warning'}
          />
          <Text type="tertiary">此页为只读项目快照；刷新不会触发派发、退款或生命周期变更。</Text>
        </Space>
      </Card>

      <div className="exchange-card-grid" style={{ marginTop: 24 }}>
        <Card bordered={false} title="发行供给守恒">
          <Descriptions align="plain" column={1} data={supplyData} layout="horizontal" />
        </Card>
        <Card bordered={false} title="申购与派发对账">
          <Descriptions align="plain" column={1} data={subscriptionData} layout="horizontal" />
        </Card>
        <Card bordered={false} title="派发与钱包入账">
          <Descriptions align="plain" column={1} data={distributionData} layout="horizontal" />
        </Card>
        <Card bordered={false} title="人工申购资金守恒">
          <Descriptions align="plain" column={1} data={quoteData} layout="horizontal" />
        </Card>
      </div>

      <Card bordered={false} style={{ marginTop: 24 }} title={`异常项（${reconciliation.anomaly_count}）`}>
        {reconciliation.anomalies.length > 0 ? (
          <ul aria-label="对账异常项">
            {reconciliation.anomalies.map((anomaly, index) => <li key={`${index}:${anomaly}`}>{anomaly}</li>)}
          </ul>
        ) : (
          <Text type="secondary">未发现需要人工核查的差异。</Text>
        )}
      </Card>

      <Card bordered={false} style={{ marginTop: 24 }} title="关联记录">
        <Space align="start" spacing={16} vertical>
          <RelatedLink
            endpoint="/admin/api/v1/new-coins/subscriptions"
            to={`/admin/new-coins/subscriptions?project_id=${projectId}`}
          >
            查看申购与配售记录
          </RelatedLink>
          <RelatedLink
            endpoint="/admin/api/v1/new-coins/distributions"
            to={`/admin/new-coins/distributions?project_id=${projectId}`}
          >
            查看派发与退款记录
          </RelatedLink>
          <RelatedLink
            endpoint="/admin/api/v1/audit-logs"
            to={`/admin/audit-logs?target_type=new_coin_project&target_id=${projectId}`}
          >
            查看项目审计日志
          </RelatedLink>
        </Space>
      </Card>
    </main>
  );
}
