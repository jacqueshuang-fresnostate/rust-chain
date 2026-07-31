import { IconExit } from '@douyinfe/semi-icons';
import { Avatar, Button, Layout, Nav, Tag, Typography } from '@douyinfe/semi-ui';
import type { NavItems, OnSelectedData } from '@douyinfe/semi-ui/lib/es/navigation';
import { useEffect, useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';

import { adminNavItems, type AdminNavItem } from '../admin/navigation';
import hippoLogoCompact from '../assets/brand/hippo-logo-compact.png';
import { authStore } from '../auth/authStore';

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

function normalizePath(pathname: string): string {
  return pathname === '/admin' ? '/admin/dashboard' : pathname;
}

function containsActivePath(item: AdminNavItem, activePath: string) {
  return item.path === activePath || Boolean(item.children?.some((child) => child.path === activePath));
}

function activeGroupKeys(activePath: string) {
  return adminNavItems.filter((item) => item.children && containsActivePath(item, activePath)).map((item) => item.label);
}

function adminNavContext(activePath: string) {
  for (const item of adminNavItems) {
    if (item.path === activePath) {
      return { domain: '运营总览', page: item.label };
    }

    const child = item.children?.find((candidate) => candidate.path === activePath);
    if (child) {
      return { domain: item.label, page: child.label };
    }
  }

  return { domain: '运营后台', page: '管理工作台' };
}

const semiNavItems: NavItems = adminNavItems.map((item) =>
  item.children
    ? {
        icon: item.icon,
        itemKey: item.label,
        text: item.label,
        items: item.children.map((child) => ({
          itemKey: child.path ?? child.label,
          text: child.label
        }))
      }
    : {
        icon: item.icon,
        itemKey: item.path ?? item.label,
        text: item.label
      }
);

export function AdminLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const session = authStore.getSession();
  const subject = session?.subject ?? 'admin';
  const activePath = normalizePath(location.pathname);
  const navContext = adminNavContext(activePath);
  const [openKeys, setOpenKeys] = useState<string[]>(() => activeGroupKeys(activePath));
  const [isCollapsed, setIsCollapsed] = useState(false);

  useEffect(() => {
    document.title = `${navContext.page} · HIPPO Operations`;
  }, [navContext.page]);

  useEffect(() => {
    const activeGroups = activeGroupKeys(activePath);
    if (activeGroups.length === 0) {
      return;
    }

    setOpenKeys((keys) => Array.from(new Set([...keys, ...activeGroups])));
  }, [activePath]);

  const handleNavSelect = ({ itemKey }: OnSelectedData) => {
    const nextPath = String(itemKey);
    if (nextPath.startsWith('/admin')) {
      navigate(nextPath);
    }
  };

  return (
    <Layout className="semi-always-light admin-layout-shell">
      <Sider
        aria-label="后台侧边栏"
        className={isCollapsed ? 'admin-layout-sider admin-layout-sider-collapsed' : 'admin-layout-sider'}
      >
        <Nav
          aria-label="后台导航"
          className="admin-layout-nav"
          footer={{
            collapseButton: true
          }}
          header={{
            logo: <img alt="HIPPO" className="admin-brand-logo" src={hippoLogoCompact} />,
            text: isCollapsed ? (
              ''
            ) : (
              <span className="admin-brand-copy">
                <strong>HIPPO</strong>
                <small>OPERATIONS</small>
              </span>
            )
          }}
          isCollapsed={isCollapsed}
          items={semiNavItems}
          limitIndent={false}
          mode="vertical"
          onCollapseChange={setIsCollapsed}
          onOpenChange={({ openKeys: nextOpenKeys }) => setOpenKeys((nextOpenKeys ?? []).map((key) => String(key)))}
          onSelect={handleNavSelect}
          openKeys={openKeys}
          selectedKeys={[activePath]}
          subNavMotion={false}
        />
      </Sider>
      <Layout className="admin-layout-main">
        <Header className="admin-layout-header">
          <div className="admin-header-context">
            <Text className="admin-header-domain">{navContext.domain}</Text>
            <Text className="admin-header-page" strong>{navContext.page}</Text>
          </div>
          <div className="admin-header-account">
            <Tag className="admin-environment-tag" color="orange" size="large">生产环境</Tag>
            <div className="admin-header-identity">
              <Avatar className="admin-header-avatar" size="small">{subject.slice(0, 1).toUpperCase()}</Avatar>
              <span>
                <Text className="admin-header-role">管理员</Text>
                <Text className="admin-header-subject" strong>{subject}</Text>
              </span>
            </div>
            <Button
              aria-label="退出登录"
              icon={<IconExit />}
              onClick={() => {
                authStore.clearSession();
                navigate('/login', { replace: true });
              }}
              theme="borderless"
              type="tertiary"
            >
              退出
            </Button>
          </div>
        </Header>
        <Content className="admin-layout-content">
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}
