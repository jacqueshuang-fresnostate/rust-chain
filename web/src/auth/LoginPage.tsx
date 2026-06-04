import { IconLock, IconShield } from '@douyinfe/semi-icons';
import { Button, Card, Form, Radio, RadioGroup, Toast, Typography } from '@douyinfe/semi-ui';
import { useMutation } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';

import { adminLogin } from '../api/adminAuth';
import { ApiError } from '../api/client';
import { authStore } from './authStore';

const { Title, Text } = Typography;

type LoginFormValues = {
  username?: string;
  password?: string;
};

export function LoginPage() {
  const navigate = useNavigate();
  const loginMutation = useMutation({
    mutationFn: adminLogin,
    onSuccess: (response) => {
      if (response.scope !== 'admin') {
        Toast.error('当前账号不是管理员');
        return;
      }

      authStore.setSession({
        accessToken: response.access_token,
        refreshToken: response.refresh_token,
        scope: response.scope,
        subject: response.subject ?? 'admin'
      });
      navigate('/admin/dashboard', { replace: true });
    },
    onError: (error) => {
      Toast.error(error instanceof ApiError ? error.message : '登录失败，请稍后重试');
    }
  });

  return (
    <main className="admin-login-page">
      <section className="admin-login-hero" aria-label="交易所管理后台登录">
        <div className="admin-login-copy">
          <Text className="admin-login-eyebrow">RUST CHAIN EXCHANGE</Text>
          <Title heading={1}>管理后台入口</Title>
          <Text className="admin-login-description">
            面向管理员的资产、行情、新币、代理与审计控制台。代理入口保留，当前阶段暂未开放。
          </Text>
        </div>
        <Card bordered={false} shadows="always" className="admin-login-card">
          <div className="admin-login-badge">
            <IconShield />
          </div>
          <Title heading={3}>登录管理后台</Title>
          <Text type="tertiary">请选择管理员身份并输入账号密码</Text>
          <Form<LoginFormValues>
            className="admin-login-form"
            onSubmit={(values) => {
              loginMutation.mutate({
                username: values.username ?? '',
                password: values.password ?? ''
              });
            }}
          >
            <Form.Slot label="登录身份">
              <RadioGroup value="admin" type="button">
                <Radio value="admin">管理员</Radio>
                <Radio value="agent" disabled>
                  代理暂未开放
                </Radio>
              </RadioGroup>
            </Form.Slot>
            <Form.Input
              field="username"
              label="管理员账号"
              prefix={<IconShield />}
              placeholder="请输入管理员账号"
              rules={[{ required: true, message: '请输入管理员账号' }]}
            />
            <Form.Input
              field="password"
              label="密码"
              mode="password"
              prefix={<IconLock />}
              placeholder="请输入密码"
              rules={[{ required: true, message: '请输入密码' }]}
            />
            <Button htmlType="submit" theme="solid" type="primary" block loading={loginMutation.isPending}>
              登录
            </Button>
          </Form>
        </Card>
      </section>
    </main>
  );
}
