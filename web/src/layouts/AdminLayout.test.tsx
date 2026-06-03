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
          { path: 'users', element: <div>用户内容</div> }
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
      expect(screen.getByRole('button', { name: label })).toBeInTheDocument();
    });

    [
      { group: '用户与代理', children: ['用户管理', '代理管理', '代理佣金'] },
      { group: '钱包资产', children: ['资产管理', '钱包账户', '钱包流水'] },
      { group: '现货交易', children: ['交易对配置', '现货动作', '现货订单', '现货成交'] },
      {
        group: '新币生命周期',
        children: ['新币项目', '生命周期动作', '发行申购', '派发记录', '上市认购', '锁仓仓位', '解禁记录']
      },
      { group: '行情市场', children: ['交易对', '行情策略', '策略动作', '行情订阅'] },
      { group: '闪兑管理', children: ['闪兑交易对', '新币闪兑规则', '闪兑订单'] },
      { group: '秒合约', children: ['秒合约产品', '秒合约订单', '秒合约动作'] },
      { group: '杠杆交易', children: ['杠杆产品', '杠杆仓位', '强平记录', '利息汇总', '杠杆动作'] },
      { group: '理财 Earn', children: ['理财产品', '理财申购', '理财动作'] },
      { group: '系统配置', children: ['SMTP 邮件配置'] }
    ].forEach(({ group, children }) => {
      const groupButton = screen.getByRole('button', { name: new RegExp(group) });
      expect(groupButton).toHaveAttribute('aria-expanded', 'false');

      fireEvent.click(groupButton);

      children.forEach((label) => {
        expect(screen.getByRole('button', { name: label })).toBeInTheDocument();
      });
    });
    expect(screen.getByText('root-admin')).toBeInTheDocument();
  });

  it('uses expandable second-level navigation and opens the active group', () => {
    renderAdminLayout('/admin/users');

    const groupButton = screen.getByRole('button', { name: /用户与代理/ });
    expect(groupButton).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('button', { name: '用户管理' })).toHaveClass('active');

    fireEvent.click(groupButton);

    expect(groupButton).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('button', { name: '用户管理' })).not.toBeInTheDocument();
  });

  it('exposes a sidebar drag handle for resizing', () => {
    renderAdminLayout();

    const sider = screen.getByLabelText('后台侧边栏');
    const resizeHandle = screen.getByRole('separator', { name: '调整导航宽度' });

    expect(sider).toHaveStyle({ width: '288px' });
    fireEvent.mouseDown(resizeHandle, { clientX: 288 });
    fireEvent.mouseMove(window, { clientX: 360 });
    fireEvent.mouseUp(window);

    expect(sider).toHaveStyle({ width: '360px' });
  });

  it('resizes the sidebar with pointer drag events', () => {
    renderAdminLayout();

    const sider = screen.getByLabelText('后台侧边栏');
    const resizeHandle = screen.getByRole('separator', { name: '调整导航宽度' });

    expect(sider).toHaveStyle({ width: '288px' });
    fireEvent.pointerDown(resizeHandle, { clientX: 288, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 360, pointerId: 1 });
    fireEvent.pointerUp(window, { pointerId: 1 });

    expect(sider).toHaveStyle({ width: '360px' });
  });
});
