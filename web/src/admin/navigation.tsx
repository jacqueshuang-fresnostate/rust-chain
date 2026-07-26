import {
  IconBell,
  IconBookOpenStroked,
  IconBriefcaseStroked,
  IconCandlestickChartStroked,
  IconCoinMoneyStroked,
  IconGlobeStroked,
  IconGridView,
  IconHomeStroked,
  IconHornStroked,
  IconLineChartStroked,
  IconList,
  IconPieChartStroked,
  IconSettingStroked,
  IconShieldStroked,
  IconStopwatchStroked,
  IconUserGroup
} from '@douyinfe/semi-icons';
import type { ReactNode } from 'react';

export type AdminNavItem = {
  icon?: ReactNode;
  label: string;
  path?: string;
  children?: AdminNavItem[];
};

export const adminNavItems: AdminNavItem[] = [
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
      { path: '/admin/wallet/deposit-network-configs', label: '充值网络配置' },
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
    icon: <IconCandlestickChartStroked aria-hidden="true" />,
    children: [
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
      { path: '/admin/market/pairs', label: '交易对配置' },
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
    icon: <IconStopwatchStroked aria-hidden="true" />,
    children: [
      { path: '/admin/seconds-contract/products', label: '秒合约产品' },
      { path: '/admin/seconds-contract/orders', label: '秒合约订单' }
    ]
  },
  {
    label: '杠杆交易',
    icon: <IconLineChartStroked aria-hidden="true" />,
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
    icon: <IconHornStroked aria-hidden="true" />,
    children: [{ path: '/admin/news', label: '新闻中心' }]
  },
  {
    label: '风控中心',
    icon: <IconShieldStroked aria-hidden="true" />,
    children: [
      { path: '/admin/risk', label: '风控规则' },
      { path: '/admin/risk/events', label: '风控事件' }
    ]
  },
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
