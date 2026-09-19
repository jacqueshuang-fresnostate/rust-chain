import { Space, TabPane, Tabs, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { AmountText } from '../../shared/AmountText';
import { ResizableTable } from '../../shared/ResizableTable';
import { TimestampText } from '../../shared/TimestampText';
import {
  accountLabels, issueLabels, obligationLabels, type JournalDifference, type JournalMovement,
  type OpenObligation, type ReconciliationReport, type WalletDifference, type WalletEvidence
} from './financialReconciliationApi';

export function FinancialReconciliationReportView({ report }: { report: ReconciliationReport }) {
  const [tab, setTab] = useState('wallets');
  const amount = (value: string | null) => <span title={value ?? '缺少证据'}>
    <AmountText value={value} asset={report.asset.symbol} precision={report.asset.precision_scale} appendAsset={false} />
  </span>;
  const note = (shown: number, total: number) => `显示 ${shown} / ${total} 项${shown < total ? '，明细已截断，计数覆盖全部记录' : ''}`;
  return <section aria-label={`${report.asset.symbol} 对账证据`}>
    <Typography.Title heading={4}>{report.asset.symbol} · 单资产证据</Typography.Title>
    <Tabs activeKey={tab} onChange={setTab} type="line">
      <TabPane itemKey="wallets" tab="钱包快照">
        <Space vertical align="start" style={{ width: '100%', marginBlock: 16 }}>
          <Typography.Text>当前余额与各账户最新流水 ID 的桶快照比较；可比差额为当前减快照，差额合计为零仍可能有异常。</Typography.Text>
          <Typography.Text strong>异常账户 {report.wallet_difference_count} 个</Typography.Text>
        </Space>
        <ResizableTable<WalletEvidence> aria-label="钱包桶汇总" rowKey="account_type" pagination={false}
          dataSource={report.wallets} columns={[
            { key: 'account_type', title: '账户', width: 140, render: (_, row) => accountLabels[row.account_type] },
            { dataIndex: 'wallet_count', title: '钱包数', width: 100 },
            ...(['available', 'frozen', 'locked'] as const).map((field, i) => ({
              key: field, title: ['当前可用', '当前冻结', '当前锁定'][i], width: 180,
              render: (_: unknown, row: WalletEvidence) => amount(row[field])
            })),
            { dataIndex: 'mismatch_count', title: '桶差异账户', width: 130 },
            { dataIndex: 'missing_ledger_count', title: '缺少流水', width: 130 },
            { dataIndex: 'missing_wallet_count', title: '缺少钱包', width: 130 },
            ...(['comparable_available_delta', 'comparable_frozen_delta', 'comparable_locked_delta'] as const).map((field, i) => ({
              key: field, title: ['可比可用差额', '可比冻结差额', '可比锁定差额'][i], width: 180,
              render: (_: unknown, row: WalletEvidence) => amount(row[field])
            }))
          ]} />
        <Typography.Paragraph style={{ marginTop: 16 }}>{note(report.wallet_differences.length, report.wallet_difference_count)}</Typography.Paragraph>
        <ResizableTable<WalletDifference> aria-label="钱包差异明细" pagination={false}
          rowKey={(row) => row ? `${row.account_type}:${row.user_id}` : ''} dataSource={report.wallet_differences}
          empty="当前证据未发现钱包桶差异；历史覆盖仍不完整" columns={[
            { key: 'account_type', title: '账户', width: 140, render: (_, row) => accountLabels[row.account_type] },
            { dataIndex: 'user_id', title: '用户 ID', width: 120 },
            { key: 'issue', title: '差异类型', width: 160, render: (_, row) => issueLabels[row.issue] },
            { dataIndex: 'ledger_id', title: '最新流水 ID', width: 150, render: (value) => value ?? '-' },
            ...(['available', 'available_after', 'frozen', 'frozen_after', 'locked', 'locked_after'] as const).map((field, i) => ({
              key: field, title: ['当前可用', '流水可用', '当前冻结', '流水冻结', '当前锁定', '流水锁定'][i],
              width: 180, render: (_: unknown, row: WalletDifference) => amount(row[field])
            }))
          ]} />
      </TabPane>
      <TabPane itemKey="journal" tab="分录差异">
        <Space wrap style={{ marginBlock: 16 }}>
          <Typography.Text>分录 {report.journal.entry_count} 条</Typography.Text>
          <Typography.Text>交易组 {report.journal.transaction_count} 个</Typography.Text>
          <Typography.Text strong>不平衡交易 {report.journal.imbalanced_transaction_count} 个</Typography.Text>
          <Typography.Text>首条 <TimestampText value={report.journal.first_entry_at} /></Typography.Text>
          <Typography.Text>末条 <TimestampText value={report.journal.last_entry_at} /></Typography.Text>
        </Space>
        <Typography.Paragraph>{note(report.journal_differences.length, report.journal.imbalanced_transaction_count)}</Typography.Paragraph>
        <ResizableTable<JournalDifference> aria-label="逐交易分录差异" pagination={false} rowKey="transaction_key"
          dataSource={report.journal_differences} empty="已有分录中未发现不平衡交易；不代表业务记账完整"
          columns={[
            { dataIndex: 'transaction_key', title: '交易键', width: 400 },
            { dataIndex: 'entry_count', title: '分录数', width: 120 },
            { key: 'net_amount', title: '本资产交易差额', width: 240, render: (_, row) => amount(row.net_amount) }
          ]} />
      </TabPane>
      <TabPane itemKey="movements" tab="科目净变动">
        <Typography.Paragraph style={{ marginTop: 16 }}>
          仅为已存在分录的带符号净变动，不是期末余额或实际库存。{note(report.journal_movements.length, report.journal_movement_count)}
        </Typography.Paragraph>
        <ResizableTable<JournalMovement> aria-label="科目净变动" pagination={false}
          rowKey={(row) => row ? JSON.stringify([row.context, row.account_code]) : ''}
          dataSource={report.journal_movements} empty="暂无分录；不能据此认定无业务" columns={[
            { dataIndex: 'context', title: '业务上下文（原始代码）', width: 240 },
            { dataIndex: 'account_code', title: '科目（原始代码）', width: 360 },
            { dataIndex: 'entry_count', title: '分录数', width: 100 },
            { key: 'net_movement', title: '本资产净变动', width: 220, render: (_, row) => amount(row.net_movement) }
          ]} />
      </TabPane>
      <TabPane itemKey="obligations" tab="未结义务">
        <Typography.Paragraph style={{ marginTop: 16 }}>
          各项可能与钱包冻结、锁定及其他指标重叠，不可相加。条件赔付按每单获胜情形列示，不是同时必然到期应付。
        </Typography.Paragraph>
        <ResizableTable<OpenObligation> aria-label="未结业务指标" pagination={false} rowKey="kind"
          dataSource={report.obligations} columns={[
            { key: 'kind', title: '指标口径', width: 420, render: (_, row) => obligationLabels[row.kind] },
            { dataIndex: 'record_count', title: '业务记录数', width: 140 },
            { key: 'amount', title: `${report.asset.symbol} 金额 / 数量`, width: 260, render: (_, row) => amount(row.amount) }
          ]} />
      </TabPane>
      <TabPane itemKey="coverage" tab="覆盖边界">
        <Typography.Title heading={5}>历史覆盖不完整</Typography.Title>
        <ul aria-label="对账覆盖限制">{report.limitations.map((item) => <li key={item} style={{ marginBlock: 12 }}>{item}</li>)}</ul>
      </TabPane>
    </Tabs>
  </section>;
}
