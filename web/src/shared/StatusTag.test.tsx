import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { StatusTag } from './StatusTag';

describe('StatusTag', () => {
  it.each([
    ['partially_filled', '部分成交'],
    ['cancelled', '已取消'],
    ['preheat', '预热中'],
    ['subscription', '发行申购'],
    ['distribution', '派发中'],
    ['listed', '已上市'],
    ['not_required', '无需支付'],
    ['unpaid', '未支付'],
    ['paid', '已支付'],
    ['opened', '持仓中'],
    ['settled', '已结算'],
    ['win', '盈利'],
    ['loss', '亏损'],
    ['liquidated', '已强平'],
    ['subscribed', '已申购'],
    ['redeemed', '已赎回'],
    ['review', '人工复核'],
    ['deny', '拒绝'],
    ['allow', '放行'],
    ['success', '成功'],
    ['skipped', '已跳过'],
    ['needs_reload', '待重载'],
    ['long', '做多'],
    ['short', '做空'],
    ['up', '看涨'],
    ['down', '看跌']
  ])('renders %s as %s', (value, label) => {
    render(<StatusTag value={value} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });
});
