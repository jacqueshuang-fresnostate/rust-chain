import { fireEvent, render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { AdminLayout } from './AdminLayout';
import { authStore } from '../auth/authStore';

function renderAdminLayout(initialEntry = '/admin/dashboard') {
  const router = createMemoryRouter(
    [
      {
        path: '/admin',
        element: <AdminLayout />,
        children: [
          { path: 'dashboard', element: <div>仪表盘内容</div> },
          { path: 'users', element: <div>用户内容</div> },
          { path: 'users/kyc', element: <div>KYC 审核内容</div> },
          { path: 'news', element: <div>新闻内容</div> },
          { path: 'wallet/deposit-network-configs', element: <div>充值网络配置内容</div> },
          { path: 'wallet/deposit-address-pool', element: <div>充值地址池内容</div> },
          { path: 'loan/products', element: <div>贷款产品内容</div> },
          { path: 'loan/orders', element: <div>贷款订单内容</div> },
          { path: 'system/countries', element: <div>国家配置内容</div> },
          { path: 'system/security-policy', element: <div>安全策略内容</div> },
          { path: 'system/brand', element: <div>PC 品牌配置内容</div> }
        ]
      }
    ],
    { initialEntries: [initialEntry] }
  );

  return render(<RouterProvider router={router} />);
}

describe('AdminLayout', () => {
  beforeEach(() => {
    authStore.setSession({ accessToken: 'token', refreshToken: 'refresh', scope: 'admin', subject: 'root-admin' });
  });

  afterEach(() => {
    authStore.clearSession();
  });

  it('renders the Chinese admin navigation labels', () => {
    renderAdminLayout();

    ['总览仪表盘', '风控中心', '审计日志'].forEach((label) => {
      expect(screen.getByRole('menuitem', { name: label })).toBeInTheDocument();
    });

    [
      { group: '用户与代理', children: ['用户管理', 'KYC 审核', '代理管理', '代理佣金', '佣金规则'] },
      { group: '钱包资产', children: ['资产管理', '钱包账户', '充值网络配置', '充值地址池', '钱包流水'] },
      { group: '贷款管理', children: ['贷款产品', '贷款订单'] },
      { group: '现货交易', children: ['交易对配置', '现货订单', '现货成交'] },
      {
        group: '新币生命周期',
        children: ['新币项目', '生命周期动作', '发行申购', '派发记录', '上市认购', '锁仓仓位', '解禁记录']
      },
      { group: '行情市场', children: ['交易对', '行情策略', '策略动作', '行情订阅'] },
      { group: '闪兑管理', children: ['闪兑交易对', '闪兑订单'] },
      { group: '秒合约', children: ['秒合约产品', '秒合约订单'] },
      { group: '杠杆交易', children: ['杠杆产品', '杠杆仓位', '强平记录', '利息汇总'] },
      { group: '理财 Earn', children: ['理财分类', '理财产品', '理财申购'] },
      { group: '内容运营', children: ['新闻中心'] },
      { group: '系统配置', children: ['国家配置', '安全策略', 'PC 品牌配置', 'SMTP 邮件配置', '上传配置'] }
    ].forEach(({ group, children }) => {
      const groupButton = screen.getByRole('menuitem', { name: new RegExp(group) });
      expect(groupButton).toHaveAttribute('aria-expanded', 'false');

      fireEvent.click(groupButton);

      children.forEach((label) => {
        expect(screen.getByRole('menuitem', { name: label })).toBeInTheDocument();
      });
    });
    expect(screen.queryByRole('menuitem', { name: '新币闪兑规则' })).not.toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: '现货动作' })).not.toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: '秒合约动作' })).not.toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: '理财动作' })).not.toBeInTheDocument();
    expect(screen.getByText('root-admin')).toBeInTheDocument();
  });

  it('uses Semi theme and Navigation defaults instead of admin shell style classes', () => {
    const { container } = renderAdminLayout();

    expect(container.querySelector('.semi-always-light')).toBeInTheDocument();
    expect(container.querySelector('.admin-shell')).not.toBeInTheDocument();
    expect(container.querySelector('.admin-shell-nav')).not.toBeInTheDocument();
  });

  it('keeps the Semi Navigation list scrollable within the sidebar', () => {
    const { container } = renderAdminLayout();

    const listWrapper = container.querySelector('.semi-navigation-list-wrapper');

    expect(listWrapper).toHaveStyle({ height: 'calc(100% - 116px)', overflowY: 'auto' });
  });

  it('activates the news center navigation entry', () => {
    renderAdminLayout('/admin/news');

    const groupButton = screen.getByRole('menuitem', { name: /内容运营/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: '新闻中心' })).toHaveClass('semi-navigation-item-selected');
    expect(screen.getByText('新闻内容')).toBeInTheDocument();
  });

  it('activates the country configuration navigation entry', () => {
    renderAdminLayout('/admin/system/countries');

    const groupButton = screen.getByRole('menuitem', { name: /系统配置/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: '国家配置' })).toHaveClass('semi-navigation-item-selected');
    expect(screen.getByText('国家配置内容')).toBeInTheDocument();
  });

  it('activates the deposit address pool navigation entry', () => {
    renderAdminLayout('/admin/wallet/deposit-address-pool');

    const groupButton = screen.getByRole('menuitem', { name: /钱包资产/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: '充值地址池' })).toHaveClass('semi-navigation-item-selected');
    expect(screen.getByText('充值地址池内容')).toBeInTheDocument();
  });

  it('activates the loan product navigation entry', () => {
    renderAdminLayout('/admin/loan/products');

    const groupButton = screen.getByRole('menuitem', { name: /贷款管理/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: '贷款产品' })).toHaveClass('semi-navigation-item-selected');
    expect(screen.getByText('贷款产品内容')).toBeInTheDocument();
  });

  it('activates the security policy navigation entry', () => {
    renderAdminLayout('/admin/system/security-policy');

    const groupButton = screen.getByRole('menuitem', { name: /系统配置/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: '安全策略' })).toHaveClass('semi-navigation-item-selected');
    expect(screen.getByText('安全策略内容')).toBeInTheDocument();
  });

  it('activates the PC brand configuration navigation entry', () => {
    renderAdminLayout('/admin/system/brand');

    const groupButton = screen.getByRole('menuitem', { name: /系统配置/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: 'PC 品牌配置' })).toHaveClass('semi-navigation-item-selected');
    expect(screen.getByText('PC 品牌配置内容')).toBeInTheDocument();
  });

  it('uses expandable second-level navigation and opens the active group', () => {
    renderAdminLayout('/admin/users');

    const groupButton = screen.getByRole('menuitem', { name: /用户与代理/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: '用户管理' })).toHaveClass('semi-navigation-item-selected');

    fireEvent.click(groupButton);

    expect(groupButton).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('menuitem', { name: '用户管理' })).not.toBeInTheDocument();
  });

  it('activates the KYC review navigation entry', () => {
    renderAdminLayout('/admin/users/kyc');

    const groupButton = screen.getByRole('menuitem', { name: /用户与代理/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: 'KYC 审核' })).toHaveClass('semi-navigation-item-selected');
    expect(screen.getByText('KYC 审核内容')).toBeInTheDocument();
  });

  it('uses the Semi Navigation footer collapse control', () => {
    const { container } = renderAdminLayout();

    const sider = screen.getByLabelText('后台侧边栏');
    const collapseButton = container.querySelector('.semi-navigation-footer button') as HTMLElement | null;

    expect(sider).toHaveStyle({ width: '272px' });
    expect(collapseButton).toBeInTheDocument();

    fireEvent.click(collapseButton as HTMLElement);

    expect(sider).toHaveStyle({ width: '72px' });
    expect(screen.queryByText('Rust Chain')).not.toBeInTheDocument();
  });
});
