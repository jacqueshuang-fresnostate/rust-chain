import { IconExit } from '@douyinfe/semi-icons';
import { Avatar, Button, Layout, Space, Typography } from '@douyinfe/semi-ui';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';

import { authStore } from '../auth/authStore';

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

type AgentNavItem = {
  label: string;
  path: string;
};

const navItems: AgentNavItem[] = [
  { path: '/agent/dashboard', label: '总览' },
  { path: '/agent/users', label: '团队用户' },
  { path: '/agent/invite-codes', label: '邀请码' },
  { path: '/agent/commissions', label: '佣金记录' },
  { path: '/agent/convert-stats', label: '闪兑统计' },
  { path: '/agent/team-tree', label: '团队树' }
];

function normalizePath(pathname: string): string {
  return pathname === '/agent' ? '/agent/dashboard' : pathname;
}

export function AgentLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const session = authStore.getSession('agent');
  const subject = session?.subject ?? 'agent';
  const activePath = normalizePath(location.pathname);

  return (
    <Layout className="admin-shell agent-shell">
      <Sider aria-label="代理侧边栏" className="admin-shell-sider" style={{ width: 240 }}>
        <div className="admin-shell-sider-inner">
          <div className="admin-shell-brand">
            <span className="admin-shell-brand-mark">AG</span>
            <div>
              <Text strong>Rust Chain</Text>
              <Text type="tertiary">代理门户</Text>
            </div>
          </div>
          <nav className="admin-shell-nav" aria-label="代理导航">
            {navItems.map((item) => (
              <button
                className={item.path === activePath ? 'admin-shell-nav-link active' : 'admin-shell-nav-link'}
                key={item.path}
                onClick={() => navigate(item.path)}
                type="button"
              >
                {item.label}
              </button>
            ))}
          </nav>
        </div>
      </Sider>
      <Layout className="admin-shell-main">
        <Header className="admin-shell-header">
          <Space>
            <Avatar size="small">{subject.slice(0, 1).toUpperCase()}</Avatar>
            <Text>{subject}</Text>
          </Space>
          <Button
            icon={<IconExit />}
            onClick={() => {
              authStore.clearSession('agent');
              navigate('/login', { replace: true });
            }}
            theme="borderless"
            type="tertiary"
          >
            退出登录
          </Button>
        </Header>
        <Content className="admin-shell-content">
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}
