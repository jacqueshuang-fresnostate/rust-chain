import { IconExit } from '@douyinfe/semi-icons';
import { Avatar, Button, Layout, Space, Typography } from '@douyinfe/semi-ui';
import { type KeyboardEvent, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent, useEffect, useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';

import { authStore } from '../auth/authStore';

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

const DEFAULT_SIDER_WIDTH = 288;
const MIN_SIDER_WIDTH = 240;
const MAX_SIDER_WIDTH = 420;

type AdminNavItem = {
  label: string;
  path?: string;
  children?: AdminNavItem[];
};

const navItems: AdminNavItem[] = [
  { path: '/admin/dashboard', label: '总览仪表盘' },
  {
    label: '用户与代理',
    children: [
      { path: '/admin/users', label: '用户管理' },
      { path: '/admin/agents', label: '代理管理' },
      { path: '/admin/agent-commissions', label: '代理佣金' }
    ]
  },
  {
    label: '钱包资产',
    children: [
      { path: '/admin/assets', label: '资产管理' },
      { path: '/admin/wallet/accounts', label: '钱包账户' },
      { path: '/admin/wallet/ledger', label: '钱包流水' }
    ]
  },
  {
    label: '现货交易',
    children: [
      { path: '/admin/market/pairs', label: '交易对配置' },
      { path: '/admin/spot/actions', label: '现货动作' },
      { path: '/admin/spot/orders', label: '现货订单' },
      { path: '/admin/spot/trades', label: '现货成交' }
    ]
  },
  {
    label: '新币生命周期',
    children: [
      { path: '/admin/new-coins/projects', label: '新币项目' },
      { path: '/admin/new-coins/actions', label: '生命周期动作' },
      { path: '/admin/new-coins/subscriptions', label: '发行申购' },
      { path: '/admin/new-coins/distributions', label: '派发记录' },
      { path: '/admin/new-coins/purchases', label: '上市认购' },
      { path: '/admin/new-coins/lock-positions', label: '锁仓仓位' },
      { path: '/admin/new-coins/unlocks', label: '解禁记录' }
    ]
  },
  {
    label: '行情市场',
    children: [
      { path: '/admin/market/pairs', label: '交易对' },
      { path: '/admin/market/strategies', label: '行情策略' },
      { path: '/admin/market/strategies/actions', label: '策略动作' },
      { path: '/admin/market/feed-config', label: '行情订阅' }
    ]
  },
  {
    label: '闪兑管理',
    children: [
      { path: '/admin/convert/pairs', label: '闪兑交易对' },
      { path: '/admin/convert/rules', label: '新币闪兑规则' },
      { path: '/admin/convert/orders', label: '闪兑订单' }
    ]
  },
  {
    label: '秒合约',
    children: [
      { path: '/admin/seconds-contract/products', label: '秒合约产品' },
      { path: '/admin/seconds-contract/orders', label: '秒合约订单' },
      { path: '/admin/seconds-contract/actions', label: '秒合约动作' }
    ]
  },
  {
    label: '杠杆交易',
    children: [
      { path: '/admin/margin/products', label: '杠杆产品' },
      { path: '/admin/margin/positions', label: '杠杆仓位' },
      { path: '/admin/margin/liquidations', label: '强平记录' },
      { path: '/admin/margin/interest', label: '利息汇总' },
      { path: '/admin/margin/actions', label: '杠杆动作' }
    ]
  },
  {
    label: '理财 Earn',
    children: [
      { path: '/admin/earn/products', label: '理财产品' },
      { path: '/admin/earn/subscriptions', label: '理财申购' },
      { path: '/admin/earn/actions', label: '理财动作' }
    ]
  },
  { path: '/admin/risk', label: '风控中心' },
  {
    label: '系统配置',
    children: [{ path: '/admin/system/smtp', label: 'SMTP 邮件配置' }]
  },
  { path: '/admin/audit-logs', label: '审计日志' }
];

function normalizePath(pathname: string): string {
  return pathname === '/admin' ? '/admin/dashboard' : pathname;
}

function containsActivePath(item: AdminNavItem, activePath: string) {
  return item.path === activePath || Boolean(item.children?.some((child) => child.path === activePath));
}

function clampSiderWidth(width: number) {
  return Math.min(MAX_SIDER_WIDTH, Math.max(MIN_SIDER_WIDTH, width));
}

export function AdminLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const session = authStore.getSession();
  const subject = session?.subject ?? 'admin';
  const activePath = normalizePath(location.pathname);
  const [expandedGroups, setExpandedGroups] = useState(() =>
    navItems.reduce<Record<string, boolean>>((groups, item) => {
      if (item.children) {
        groups[item.label] = containsActivePath(item, activePath);
      }
      return groups;
    }, {})
  );
  const [siderWidth, setSiderWidth] = useState(DEFAULT_SIDER_WIDTH);
  const [isResizing, setIsResizing] = useState(false);

  useEffect(() => {
    const activeGroup = navItems.find((item) => item.children && containsActivePath(item, activePath));
    if (!activeGroup) {
      return;
    }

    setExpandedGroups((groups) => ({ ...groups, [activeGroup.label]: true }));
  }, [activePath]);

  useEffect(() => {
    if (!isResizing) {
      return undefined;
    }

    const handleDragMove = (event: MouseEvent | PointerEvent) => {
      setSiderWidth(clampSiderWidth(event.clientX));
    };
    const handleDragEnd = () => setIsResizing(false);

    window.addEventListener('mousemove', handleDragMove);
    window.addEventListener('mouseup', handleDragEnd);
    window.addEventListener('pointermove', handleDragMove);
    window.addEventListener('pointerup', handleDragEnd);
    window.addEventListener('pointercancel', handleDragEnd);

    return () => {
      window.removeEventListener('mousemove', handleDragMove);
      window.removeEventListener('mouseup', handleDragEnd);
      window.removeEventListener('pointermove', handleDragMove);
      window.removeEventListener('pointerup', handleDragEnd);
      window.removeEventListener('pointercancel', handleDragEnd);
    };
  }, [isResizing]);

  const toggleGroup = (label: string) => {
    setExpandedGroups((groups) => ({ ...groups, [label]: !groups[label] }));
  };

  const startResizing = () => setIsResizing(true);

  const handleResizeStart = (event: ReactMouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    startResizing();
  };

  const handleResizePointerStart = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    startResizing();
  };

  const handleResizeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      setSiderWidth((width) => clampSiderWidth(width - 16));
    }
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      setSiderWidth((width) => clampSiderWidth(width + 16));
    }
  };

  return (
    <Layout className={isResizing ? 'admin-shell admin-shell-resizing' : 'admin-shell'}>
      <Sider aria-label="后台侧边栏" className="admin-shell-sider" style={{ width: siderWidth }}>
        <div className="admin-shell-sider-inner">
          <div className="admin-shell-brand">
            <span className="admin-shell-brand-mark">RC</span>
            <div>
              <Text strong>Rust Chain</Text>
              <Text type="tertiary">管理后台</Text>
            </div>
          </div>
          <nav className="admin-shell-nav" aria-label="后台导航">
            {navItems.map((group) => {
              const hasChildren = Boolean(group.children?.length);
              const groupExpanded = expandedGroups[group.label] ?? false;

              return (
                <div className="admin-shell-nav-group" key={group.label}>
                  {hasChildren ? (
                    <button
                      aria-expanded={groupExpanded}
                      className={
                        containsActivePath(group, activePath)
                          ? 'admin-shell-nav-title active'
                          : 'admin-shell-nav-title'
                      }
                      onClick={() => toggleGroup(group.label)}
                      type="button"
                    >
                      <span>{group.label}</span>
                      <span aria-hidden="true" className="admin-shell-nav-caret">
                        {groupExpanded ? '−' : '+'}
                      </span>
                    </button>
                  ) : (
                    <button
                      className={group.path === activePath ? 'admin-shell-nav-link active' : 'admin-shell-nav-link'}
                      onClick={() => navigate(group.path ?? '/admin/dashboard')}
                      type="button"
                    >
                      {group.label}
                    </button>
                  )}
                  {hasChildren && groupExpanded ? (
                    <div className="admin-shell-nav-children">
                      {group.children?.map((item) => (
                        <button
                          className={item.path === activePath ? 'admin-shell-nav-link active' : 'admin-shell-nav-link'}
                          key={item.path}
                          onClick={() => navigate(item.path ?? '/admin/dashboard')}
                          type="button"
                        >
                          {item.label}
                        </button>
                      ))}
                    </div>
                  ) : null}
                </div>
              );
            })}
          </nav>
        </div>
        <div
          aria-label="调整导航宽度"
          aria-valuemax={MAX_SIDER_WIDTH}
          aria-valuemin={MIN_SIDER_WIDTH}
          aria-valuenow={siderWidth}
          className="admin-shell-sider-resizer"
          onKeyDown={handleResizeKeyDown}
          onMouseDown={handleResizeStart}
          onPointerDown={handleResizePointerStart}
          role="separator"
          tabIndex={0}
        />
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
              authStore.clearSession();
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
