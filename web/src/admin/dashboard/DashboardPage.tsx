import { Card, Typography } from '@douyinfe/semi-ui';

import { PageHeader } from '../../layouts/PageHeader';

const { Text, Title } = Typography;

const cards = [
  { title: '资金安全', description: '钱包、流水、锁仓与解禁数据统一通过后台只读页面核对。' },
  { title: '交易运营', description: '现货、闪兑、秒合约、杠杆与理财产品按模块分区管理。' },
  { title: '审计闭环', description: '敏感动作必须填写原因并通过二次确认提交，后端写入审计日志。' }
];

export function DashboardPage() {
  return (
    <main className="exchange-page admin-dashboard-page">
      <PageHeader title="总览仪表盘" description="Rust Chain Exchange 管理后台操作入口。" />
      <section className="exchange-card-grid">
        {cards.map((card) => (
          <Card bordered={false} className="admin-dashboard-card" key={card.title} shadows="always">
            <Title heading={4}>{card.title}</Title>
            <Text type="secondary">{card.description}</Text>
          </Card>
        ))}
      </section>
    </main>
  );
}
