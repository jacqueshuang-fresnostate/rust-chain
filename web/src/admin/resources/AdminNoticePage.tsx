import { Banner, Card, Typography } from '@douyinfe/semi-ui';

const { Title, Paragraph } = Typography;

export function AdminNoticePage() {
  return (
    <main className="exchange-page">
      <Card bordered={false} shadows="always">
        <Title heading={3}>后台管理安全提示</Title>
        <Banner
          description="当前页面仅展示管理后台接入警示，不会请求任何接口。上线前请确认管理员权限、操作原因、二次确认和审计日志均已接入。"
          fullMode={false}
          title="敏感资源操作需谨慎"
          type="warning"
        />
        <Paragraph style={{ marginTop: 20 }}>
          请勿在未完成权限校验、风控审核和审计记录前开放资金、账户、锁仓、新币生命周期等后台资源的写操作。
        </Paragraph>
      </Card>
    </main>
  );
}
