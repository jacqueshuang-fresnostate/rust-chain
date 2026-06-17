import {
  IconBell,
  IconBookOpenStroked,
  IconBriefcaseStroked,
  IconCoinMoneyStroked,
  IconExit,
  IconGlobeStroked,
  IconGridView,
  IconHomeStroked,
  IconList,
  IconPieChartStroked,
  IconServerStroked,
  IconSettingStroked,
  IconUserGroup
} from '@douyinfe/semi-icons';
import { Avatar, Button, Layout, Nav, Space, Typography } from '@douyinfe/semi-ui';
import type { NavItems, OnSelectedData } from '@douyinfe/semi-ui/lib/es/navigation';
import { type CSSProperties, type ReactNode, useEffect, useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';

import { authStore } from '../auth/authStore';

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

const EXPANDED_SIDER_WIDTH = 272;
const COLLAPSED_SIDER_WIDTH = 72;

const shellStyle: CSSProperties = {
  height: '100vh',
  minHeight: '100vh'
};

const navStyle: CSSProperties = {
  height: '100%',
  width: '100%'
};

const navBodyStyle: CSSProperties = {
  height: 'calc(100% - 116px)',
  overflowY: 'auto'
};

const mainStyle: CSSProperties = {
  minWidth: 0
};

const headerStyle: CSSProperties = {
  alignItems: 'center',
  display: 'flex',
  justifyContent: 'space-between',
  padding: '16px 24px',
  backgroundColor: 'var(--semi-color-bg-0)',
  borderBottom: '1px solid var(--semi-color-border)'
};

const contentStyle: CSSProperties = {
  minWidth: 0,
  overflow: 'auto',
  backgroundColor: 'var(--semi-color-bg-0)'
};

type AdminNavItem = {
  icon?: ReactNode;
  label: string;
  path?: string;
  children?: AdminNavItem[];
};

const navItems: AdminNavItem[] = [
  { path: '/admin/dashboard', label: '总览仪表盘', icon: <IconHomeStroked aria-hidden="true" /> },
  {
    label: '用户与代理',
    icon: <IconUserGroup aria-hidden="true" />,
    children: [
      { path: '/admin/users', label: '用户管理' },
      { path: '/admin/users/kyc', label: 'KYC 审核' },
      { path: '/admin/agents', label: '代理管理' },
      { path: '/admin/agent-commissions', label: '代理佣金' },
      { path: '/admin/agent-commission-rules', label: '佣金规则' }
    ]
  },
  {
    label: '钱包资产',
    icon: <IconCoinMoneyStroked aria-hidden="true" />,
    children: [
      { path: '/admin/assets', label: '资产管理' },
      { path: '/admin/wallet/accounts', label: '钱包账户' },
      { path: '/admin/wallet/deposit-address-pool', label: '充值地址池' },
      { path: '/admin/wallet/quick-recharge', label: '快速充值配置' },
      { path: '/admin/wallet/quick-recharge-orders', label: '快速充值订单' },
      { path: '/admin/wallet/ledger', label: '钱包流水' }
    ]
  },
  {
    label: '贷款管理',
    icon: <IconBriefcaseStroked aria-hidden="true" />,
    children: [
      { path: '/admin/loan/products', label: '贷款产品' },
      { path: '/admin/loan/orders', label: '贷款订单' }
    ]
  },
  {
    label: '竞猜管理',
    icon: <IconPieChartStroked aria-hidden="true" />,
    children: [
      { path: '/admin/prediction/settings', label: '竞猜配置' },
      { path: '/admin/prediction/assets', label: '下注资产' },
      { path: '/admin/prediction/markets', label: '竞猜市场' },
      { path: '/admin/prediction/orders', label: '竞猜订单' },
      { path: '/admin/prediction/sync-logs', label: '同步日志' }
    ]
  },
  {
    label: '现货交易',
    icon: <IconPieChartStroked aria-hidden="true" />,
    children: [
      { path: '/admin/market/pairs', label: '交易对配置' },
      { path: '/admin/spot/orders', label: '现货订单' },
      { path: '/admin/spot/trades', label: '现货成交' }
    ]
  },
  {
    label: '新币生命周期',
    icon: <IconBell aria-hidden="true" />,
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
    icon: <IconGlobeStroked aria-hidden="true" />,
    children: [
      { path: '/admin/market/pairs', label: '交易对' },
      { path: '/admin/market/strategies', label: '行情策略' },
      { path: '/admin/market/strategies/actions', label: '策略动作' },
      { path: '/admin/market/feed-config', label: '行情订阅' }
    ]
  },
  {
    label: '闪兑管理',
    icon: <IconGridView aria-hidden="true" />,
    children: [
      { path: '/admin/convert/pairs', label: '闪兑交易对' },
      { path: '/admin/convert/orders', label: '闪兑订单' }
    ]
  },
  {
    label: '秒合约',
    icon: <IconList aria-hidden="true" />,
    children: [
      { path: '/admin/seconds-contract/products', label: '秒合约产品' },
      { path: '/admin/seconds-contract/orders', label: '秒合约订单' }
    ]
  },
  {
    label: '杠杆交易',
    icon: <IconBriefcaseStroked aria-hidden="true" />,
    children: [
      { path: '/admin/margin/products', label: '杠杆产品' },
      { path: '/admin/margin/positions', label: '杠杆仓位' },
      { path: '/admin/margin/liquidations', label: '强平记录' },
      { path: '/admin/margin/interest', label: '利息汇总' }
    ]
  },
  {
    label: '理财 Earn',
    icon: <IconBookOpenStroked aria-hidden="true" />,
    children: [
      { path: '/admin/earn/categories', label: '理财分类' },
      { path: '/admin/earn/products', label: '理财产品' },
      { path: '/admin/earn/subscriptions', label: '理财申购' }
    ]
  },
  {
    label: '内容运营',
    icon: <IconServerStroked aria-hidden="true" />,
    children: [{ path: '/admin/news', label: '新闻中心' }]
  },
  { path: '/admin/risk', label: '风控中心', icon: <IconServerStroked aria-hidden="true" /> },
  {
    label: '系统配置',
    icon: <IconSettingStroked aria-hidden="true" />,
    children: [
      { path: '/admin/system/countries', label: '国家配置' },
      { path: '/admin/system/security-policy', label: '安全策略' },
      { path: '/admin/system/brand', label: 'PC 品牌配置' },
      { path: '/admin/system/smtp', label: 'SMTP 邮件配置' },
      { path: '/admin/system/uploads', label: '上传配置' }
    ]
  },
  { path: '/admin/audit-logs', label: '审计日志', icon: <IconList aria-hidden="true" /> }
];

function normalizePath(pathname: string): string {
  return pathname === '/admin' ? '/admin/dashboard' : pathname;
}

function containsActivePath(item: AdminNavItem, activePath: string) {
  return item.path === activePath || Boolean(item.children?.some((child) => child.path === activePath));
}

function activeGroupKeys(activePath: string) {
  return navItems.filter((item) => item.children && containsActivePath(item, activePath)).map((item) => item.label);
}

const semiNavItems: NavItems = navItems.map((item) =>
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
  const [openKeys, setOpenKeys] = useState<string[]>(() => activeGroupKeys(activePath));
  const [isCollapsed, setIsCollapsed] = useState(false);

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
    <Layout className="semi-always-light" style={shellStyle}>
      <Sider aria-label="后台侧边栏" style={{ width: isCollapsed ? COLLAPSED_SIDER_WIDTH : EXPANDED_SIDER_WIDTH }}>
        <Nav
          aria-label="后台导航"
          bodyStyle={navBodyStyle}
          footer={{
            collapseButton: true
          }}
          header={{
            logo: <Avatar size="small">RC</Avatar>,
            text: isCollapsed ? '' : 'Rust Chain'
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
          style={navStyle}
          subNavMotion={false}
        />
      </Sider>
      <Layout style={mainStyle}>
        <Header style={headerStyle}>
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
        <Content style={contentStyle}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}
