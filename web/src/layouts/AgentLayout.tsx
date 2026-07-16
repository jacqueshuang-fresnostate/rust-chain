import { IconCoinMoneyStroked, IconExit, IconHomeStroked, IconList, IconUserGroup } from '@douyinfe/semi-icons';
import { Avatar, Button, Layout, Nav, Space, Typography } from '@douyinfe/semi-ui';
import type { NavItems, OnSelectedData } from '@douyinfe/semi-ui/lib/es/navigation';
import type { CSSProperties, ReactNode } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';

import { authStore } from '../auth/authStore';

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

type AgentNavItem = {
  icon: ReactNode;
  label: string;
  path: string;
};

const navItems: AgentNavItem[] = [
  { path: '/agent/dashboard', label: '总览', icon: <IconHomeStroked aria-hidden="true" /> },
  { path: '/agent/users', label: '团队用户', icon: <IconUserGroup aria-hidden="true" /> },
  { path: '/agent/invite-codes', label: '邀请码', icon: <IconList aria-hidden="true" /> },
  { path: '/agent/commissions', label: '佣金记录', icon: <IconCoinMoneyStroked aria-hidden="true" /> },
  { path: '/agent/convert-stats', label: '闪兑统计', icon: <IconCoinMoneyStroked aria-hidden="true" /> },
  { path: '/agent/team-tree', label: '团队树', icon: <IconList aria-hidden="true" /> }
];

const shellStyle: CSSProperties = {
  height: '100vh',
  minHeight: '100vh'
};

const navStyle: CSSProperties = {
  height: '100%',
  width: '100%'
};

const navBodyStyle: CSSProperties = {
  height: 'calc(100% - 64px)',
  overflowY: 'auto'
};

const headerStyle: CSSProperties = {
  alignItems: 'center',
  backgroundColor: 'var(--semi-color-bg-0)',
  borderBottom: '1px solid var(--semi-color-border)',
  display: 'flex',
  justifyContent: 'space-between',
  padding: '16px 24px'
};

const contentStyle: CSSProperties = {
  backgroundColor: 'var(--semi-color-bg-0)',
  minWidth: 0,
  overflow: 'auto'
};

const semiNavItems: NavItems = navItems.map((item) => ({
  icon: item.icon,
  itemKey: item.path,
  text: item.label
}));

function normalizePath(pathname: string): string {
  return pathname === '/agent' ? '/agent/dashboard' : pathname;
}

export function AgentLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const session = authStore.getSession('agent');
  const subject = session?.subject ?? 'agent';
  const activePath = normalizePath(location.pathname);
  const handleNavSelect = ({ itemKey }: OnSelectedData) => {
    const nextPath = String(itemKey);
    if (nextPath.startsWith('/agent')) {
      navigate(nextPath);
    }
  };

  return (
    <Layout className="semi-always-light" style={shellStyle}>
      <Sider aria-label="代理侧边栏" style={{ width: 240 }}>
        <Nav
          aria-label="代理导航"
          bodyStyle={navBodyStyle}
          header={{
            logo: <Avatar size="small">AG</Avatar>,
            text: '代理门户'
          }}
          items={semiNavItems}
          mode="vertical"
          onSelect={handleNavSelect}
          selectedKeys={[activePath]}
          style={navStyle}
        />
      </Sider>
      <Layout style={{ minWidth: 0 }}>
        <Header style={headerStyle}>
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
        <Content style={contentStyle}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}
