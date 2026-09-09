import { Banner, Button, Card, Descriptions, Space, Typography } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';

import { apiRequest, ContractError } from '../../api/client';
import { authStore } from '../../auth/authStore';
import { PageHeader } from '../../layouts/PageHeader';
import { adminErrorMessage } from '../../shared/adminErrorMessage';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';

const { Text } = Typography;

const AUDIT_PATH = '/admin/api/v1/governance/financial-idempotency';
const AUDIT_STATUSES = new Set(['attention', 'balanced']);
const COUNT_FIELDS = [
  'duplicate_idempotency_groups',
  'missing_idempotency_keys',
  'orphan_ledger_entries',
  'expired_seconds_orders',
  'expired_prediction_orders',
  'pending_loan_orders',
  'pending_new_coin_subscriptions',
  'anomaly_count'
] as const;

export type FinancialIdempotencyAudit = {
  status: 'attention' | 'balanced';
  duplicate_idempotency_groups: number;
  missing_idempotency_keys: number;
  orphan_ledger_entries: number;
  expired_seconds_orders: number;
  expired_prediction_orders: number;
  pending_loan_orders: number;
  pending_new_coin_subscriptions: number;
  anomaly_count: number;
  anomalies: string[];
  checked_at: number;
};

function auditContractError(): never {
  throw new ContractError('资金与结算幂等审计响应不完整，请刷新或联系管理员', {
    path: AUDIT_PATH
  });
}

/** 对审计响应失败关闭，避免字段缺失时把未知状态误显示为“平衡”。 */
export function parseFinancialIdempotencyAudit(value: unknown): FinancialIdempotencyAudit {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return auditContractError();
  const record = value as Record<string, unknown>;

  if (typeof record.status !== 'string' || !AUDIT_STATUSES.has(record.status)) {
    return auditContractError();
  }
  for (const field of COUNT_FIELDS) {
    if (!Number.isSafeInteger(record[field]) || Number(record[field]) < 0) {
      return auditContractError();
    }
  }
  if (
    !Array.isArray(record.anomalies) ||
    !record.anomalies.every(
      (anomaly) => typeof anomaly === 'string' && anomaly.trim().length > 0
    ) ||
    record.anomaly_count !== record.anomalies.length ||
    (record.status === 'balanced') !== (record.anomaly_count === 0) ||
    !Number.isSafeInteger(record.checked_at) ||
    Number(record.checked_at) <= 0
  ) {
    return auditContractError();
  }

  return record as FinancialIdempotencyAudit;
}

export async function loadFinancialIdempotencyAudit(
  signal?: AbortSignal
): Promise<FinancialIdempotencyAudit> {
  const response = await apiRequest<unknown>(AUDIT_PATH, { signal });
  return parseFinancialIdempotencyAudit(response);
}

function auditQueryKey() {
  const session = authStore.getSession('admin');
  return [
    'admin-financial-idempotency-audit',
    session ? `${session.subject}:${session.generation}` : 'anonymous:none'
  ];
}

export function FinancialIdempotencyPage() {
  const query = useQuery({
    queryFn: ({ signal }) => loadFinancialIdempotencyAudit(signal),
    queryKey: auditQueryKey(),
    refetchOnWindowFocus: false,
    retry: false,
    throwOnError: false
  });

  if (query.isPending) {
    return (
      <main className="exchange-page">
        <PageHeader title="资金与结算幂等审计" />
        <section aria-live="polite" className="admin-table-state">
          正在读取资金审计快照…
        </section>
      </main>
    );
  }

  const audit = query.data;
  if (!audit) {
    return (
      <main className="exchange-page">
        <PageHeader title="资金与结算幂等审计" />
        <Banner
          closeIcon={null}
          description={adminErrorMessage(query.error, '资金审计快照加载失败')}
          icon={null}
          type="danger"
        />
        <Button loading={query.isFetching} onClick={() => void query.refetch()}>
          重新加载
        </Button>
      </main>
    );
  }

  const integrityData = [
    { key: '重复幂等键分组', value: audit.duplicate_idempotency_groups },
    { key: '缺失或空白幂等键', value: audit.missing_idempotency_keys },
    { key: '无法关联业务对象的钱包流水', value: audit.orphan_ledger_entries }
  ];
  const settlementData = [
    { key: '已过期未结算秒合约', value: audit.expired_seconds_orders },
    { key: '市场已结束未结算竞猜订单', value: audit.expired_prediction_orders }
  ];
  const queueData = [
    { key: '待审核贷款订单', value: audit.pending_loan_orders },
    { key: '待派发或退款新币申购', value: audit.pending_new_coin_subscriptions }
  ];

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <Button loading={query.isFetching} onClick={() => void query.refetch()}>
            刷新审计
          </Button>
        }
        description={
          <>
            检查时间 <TimestampText value={audit.checked_at} />
          </>
        }
        title="资金与结算幂等审计"
      />

      {query.error ? (
        <Banner
          closeIcon={null}
          description={`${adminErrorMessage(query.error, '刷新资金审计失败')}；已保留上一份快照。`}
          icon={null}
          type="warning"
        />
      ) : null}

      <Card bordered={false} shadows="always">
        <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
          <Space wrap>
            <Text strong>审计结果</Text>
            <StatusTag value={audit.status} />
            <Text type="secondary">异常项 {audit.anomaly_count} 条</Text>
          </Space>
          <Banner
            closeIcon={null}
            description={
              audit.status === 'balanced'
                ? '未发现幂等键、钱包流水关联或到期结算异常。待办业务队列仅作信息展示。'
                : `发现 ${audit.anomaly_count} 类资金完整性异常，请先核对订单、流水和结算 worker。`
            }
            icon={null}
            type={audit.status === 'balanced' ? 'success' : 'warning'}
          />
          <Text type="tertiary">
            此页为只读快照；刷新不会重放订单、补记流水、自动结算或改动用户余额。
          </Text>
        </Space>
      </Card>

      <div className="exchange-card-grid" style={{ marginTop: 24 }}>
        <Card bordered={false} title="幂等与流水完整性">
          <Descriptions align="plain" column={1} data={integrityData} layout="horizontal" />
        </Card>
        <Card bordered={false} title="到期结算队列">
          <Descriptions align="plain" column={1} data={settlementData} layout="horizontal" />
        </Card>
        <Card bordered={false} title="正常业务待办">
          <Descriptions align="plain" column={1} data={queueData} layout="horizontal" />
        </Card>
      </div>

      <Card bordered={false} style={{ marginTop: 24 }} title={`异常项（${audit.anomaly_count}）`}>
        {audit.anomalies.length > 0 ? (
          <ul aria-label="资金审计异常项">
            {audit.anomalies.map((anomaly, index) => (
              <li key={`${index}:${anomaly}`}>{anomaly}</li>
            ))}
          </ul>
        ) : (
          <Text type="secondary">未发现需要人工核查的资金完整性异常。</Text>
        )}
      </Card>
    </main>
  );
}
