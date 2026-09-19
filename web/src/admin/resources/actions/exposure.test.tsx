import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactElement } from 'react';

import { listAdminResource } from '../../../api/adminResources';
import { apiRequest } from '../../../api/client';
import { EarnProductRowActions } from './earn';
import { ConvertPairRowActions } from './convert';
import { SecondsProductRowActions } from './secondsContract';

vi.mock('../../../api/adminResources', () => ({ listAdminResource: vi.fn() }));
vi.mock('../../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'),
  apiRequest: vi.fn()
}));
vi.mock('../../../shared/QuillRichTextEditor', () => ({
  QuillRichTextEditor: () => <div />
}));

const helpers = { reload: vi.fn(), openDetail: vi.fn(), loadDetail: vi.fn() };
const earn = {
  id: 8, asset_id: 12, name: '收益产品', category: 'fixed_term', term_days: 30,
  apr_rate: '0.1', min_subscribe: '1', status: 'active', principal_capacity: '0',
  liability_capacity: '9007199254740993.000000000000000001',
  introduction_json: { items: [{ locale: 'zh-CN', country: 'CN', title: '收益产品', content: [{ type: 'p', children: [{ text: '' }] }] }] }
};
const pair = {
  id: 7, from_asset_id: 12, to_asset_id: 13, pricing_mode: 'fixed',
  spread_rate: '0.01', fee_rate: '0', min_amount: '1', max_amount: null,
  target_min_amount: '0', target_max_amount: null, enabled: true,
  inventory_enabled: true, inventory_funded_amount: '9007199254740993.000000000000000001',
  inventory_consumed_amount: '7', inventory_revision: 3
};

function mount(element: ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}>{element}</QueryClientProvider>);
}

async function confirm(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: '提交修改' }));
  fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '核验配置' } });
  await user.click(screen.getByRole('button', { name: '确认' }));
  await waitFor(() => expect(apiRequest).toHaveBeenCalledTimes(1));
  return JSON.parse(String(vi.mocked(apiRequest).mock.calls[0][1]?.body));
}

describe('aggregate exposure admin forms', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(apiRequest).mockResolvedValue({});
    vi.mocked(listAdminResource).mockImplementation(async (path) => ({
      rows: path.endsWith('/assets') ? [
        { id: 12, symbol: 'USD', precision_scale: 18 }, { id: 13, symbol: 'BTC', precision_scale: 18 }
      ] : path.endsWith('/countries') ? [
        { country_code: 'CN', country_name: '中国', default_locale: 'zh-CN' }
      ] : [{ code: 'fixed_term', default_name: '定期' }],
      raw: {}
    }));
  });

  it('roundtrips zero and exact Earn caps', async () => {
    const user = userEvent.setup();
    mount(<EarnProductRowActions helpers={helpers} record={earn} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    expect(await screen.findByLabelText('产品本金容量')).toHaveValue('0');
    expect(screen.getByLabelText('产品毛兑付义务上限')).toHaveValue(earn.liability_capacity);
    await waitFor(() => expect(screen.getByRole('button', { name: '提交修改' })).toBeEnabled());
    expect(await confirm(user)).toMatchObject({ principal_capacity: '0', liability_capacity: earn.liability_capacity });
  });

  it('rejects invalid Earn caps and clears blanks to null', async () => {
    const user = userEvent.setup();
    mount(<EarnProductRowActions helpers={helpers} record={earn} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    for (const label of ['产品本金容量', '产品毛兑付义务上限']) {
      const input = await screen.findByLabelText(label);
      for (const value of ['-1', '1oops', '0.0000000000000000001', '100000000000000000000']) {
        fireEvent.change(input, { target: { value } });
        expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
      }
      fireEvent.change(input, { target: { value: '' } });
    }
    expect(await confirm(user)).toMatchObject({ principal_capacity: null, liability_capacity: null });
  });

  it('does not silently disable malformed Earn response policy', () => {
    mount(<EarnProductRowActions helpers={helpers} record={{ ...earn, principal_capacity: 123 }} />);
    expect(screen.getByRole('button', { name: '修改' })).toBeDisabled();
  });

  it('preserves Convert funds while disabling, with revision and funding reference', async () => {
    const user = userEvent.setup();
    mount(<ConvertPairRowActions helpers={helpers} record={pair} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    expect(await screen.findByLabelText('库存资金总额')).toHaveValue(pair.inventory_funded_amount);
    expect(screen.getByText('库存资金为显式配置预算，不代表已核验的托管余额。')).toBeInTheDocument();
    expect(screen.getByText('保护启用期间仅支持正向输出资产，反向兑换确认将被拒绝。')).toBeInTheDocument();
    await user.click(screen.getByRole('switch', { name: '输出库存保护' }));
    expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
    fireEvent.change(screen.getByLabelText('资金凭据'), { target: { value: 'custody-proof-42' } });
    await waitFor(() => expect(screen.getByRole('button', { name: '提交修改' })).toBeEnabled());
    expect(await confirm(user)).toMatchObject({ inventory: {
      enabled: false, funded_amount: pair.inventory_funded_amount, revision: 3, funding_reference: 'custody-proof-42'
    } });
  });

  it('rejects malformed Convert funding and never submits an implicit zero balance', async () => {
    const user = userEvent.setup();
    mount(<ConvertPairRowActions helpers={helpers} record={pair} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    const input = await screen.findByLabelText('库存资金总额');
    fireEvent.change(screen.getByLabelText('资金凭据'), { target: { value: 'proof' } });
    for (const value of ['', '-1', 'bad', '1e-19', '100000000000000000000']) {
      fireEvent.change(input, { target: { value } });
      expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
    }
    expect(apiRequest).not.toHaveBeenCalled();
  });

  it('roundtrips exact seconds gross capacity and clears it to null without changing rates', async () => {
    const user = userEvent.setup();
    mount(<SecondsProductRowActions helpers={helpers} record={{
      id: 9, pair_id: 7, stake_asset: 12, status: 'active',
      duration_seconds: 60, payout_rate: '1.4', min_stake: '1', max_stake: null,
      open_payout_capacity: '9007199254740993.000000000000000001'
    }} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    const input = await screen.findByLabelText('未结毛兑付容量');
    expect(input).toHaveValue('9007199254740993.000000000000000001');
    for (const value of ['-1', 'invalid', '1e-19', '100000000000000000000']) {
      fireEvent.change(input, { target: { value } });
      expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
    }
    fireEvent.change(input, { target: { value: '0' } });
    await waitFor(() => expect(screen.getByRole('button', { name: '提交修改' })).toBeEnabled());
    fireEvent.change(input, { target: { value: '' } });
    expect(await confirm(user)).toMatchObject({
      open_payout_capacity: null, cycles: [{ duration_seconds: 60, payout_rate: '1.4' }]
    });
  });
});
