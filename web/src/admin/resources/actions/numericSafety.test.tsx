import { act, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { apiRequest } from '../../../api/client';
import { listAdminResource } from '../../../api/adminResources';
import { ConvertPairRowActions, convertPairRequestBody } from './convert';
import { CreateAgentCommissionRuleAction } from './agents';
import { secondsProductRequestBody } from './secondsContract';
import { MarginProductRowActions, marginProductRequestBody } from './margin';
import { earnProductRequestBody } from './earn';
import { loanProductRequestBody } from './loan';
import { AssetRowActions, CreateAssetAction } from './wallet';
import { useSmtpConfigWorkspace } from '../../actions/smtp/useSmtpConfigWorkspace';

vi.mock('../../../api/adminResources', () => ({ listAdminResource: vi.fn() }));
vi.mock('../../../api/client', async () => ({
  ...await vi.importActual<typeof import('../../../api/client')>('../../../api/client'), apiRequest: vi.fn()
}));
vi.mock('../../../shared/QuillRichTextEditor', () => ({ QuillRichTextEditor: () => <div /> }));

const exact = '9007199254740993.000000000000000001';
const invalidAmounts = ['1e-19', '1.0000000000000000001', '100000000000000000000', '1e9999999', 'NaN', 'Infinity'];
const pair = {
  fromAssetId: '1', toAssetId: '2', pricingMode: 'fixed', spreadRate: '0.01', feeRate: '0',
  minAmount: exact, maxAmount: '', targetMinAmount: '1e-18', targetMaxAmount: '', enabled: 'true'
};
const seconds = {
  pairId: '1', stakeAsset: '2', logoUrl: '', status: 'active', openPayoutCapacity: '',
  periods: [{ rowId: 'a', durationSeconds: '60', payoutRate: '0.1234567800', minStake: exact, maxStake: '' }]
};
const margin = {
  pairId: '1', marginAsset: '2', logoUrl: '', marginModes: ['isolated' as const],
  defaultMarginMode: 'isolated' as const, leverageLevels: ['2'], customLeverageLevels: '',
  minMargin: exact, maxMargin: '', maintenanceMarginRate: '0.05', hourlyInterestRate: '0.00000001', status: 'active'
};
const earn = {
  assetId: '1', name: '理财', category: 'fixed_term', termDays: '30', aprRate: '0.1',
  redemptionFeeRate: '0', maturityProfitFeeRate: '0', earlyRedeemFeeBasis: 'none', earlyRedeemFeeRate: '',
  minSubscribe: exact, maxSubscribe: '', principalCapacity: '', liabilityCapacity: '', status: 'active',
  bannerUrl: '', smallLogoUrl: '', introductions: []
};
const loan = {
  assetId: '1', name: '借款', names: [], loanType: 'credit', interestCalculationMode: 'full_term',
  interestRate: '0.01', minAmount: exact, maxAmount: '', userPrincipalLimit: '',
  productPrincipalCapacity: '', denyBorrowingWhileOverdue: false, status: 'active', termDays: '30', minKycLevel: '0'
};
function mount(element: React.ReactElement) {
  render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{element}</QueryClientProvider>);
}

describe('real numeric payload builders', () => {
  it('retains exact source amounts and rate trailing zeros', () => {
    expect(convertPairRequestBody(pair, 'reason').min_amount).toBe(exact);
    expect(secondsProductRequestBody(seconds, 'reason').cycles[0]).toMatchObject({ min_stake: exact, payout_rate: '0.1234567800' });
    expect(marginProductRequestBody(margin, 'reason').min_margin).toBe(exact);
    expect(earnProductRequestBody(earn, 'reason').min_subscribe).toBe(exact);
    expect(loanProductRequestBody(loan, 'reason').min_amount).toBe(exact);
  });
  it.each(invalidAmounts)('rejects amount %s across product serializers', (amount) => {
    expect(() => convertPairRequestBody({ ...pair, minAmount: amount }, 'reason')).toThrow();
    expect(() => secondsProductRequestBody({ ...seconds, periods: [{ ...seconds.periods[0], minStake: amount }] }, 'reason')).toThrow();
    expect(() => marginProductRequestBody({ ...margin, minMargin: amount }, 'reason')).toThrow();
    expect(() => earnProductRequestBody({ ...earn, minSubscribe: amount }, 'reason')).toThrow();
    expect(() => loanProductRequestBody({ ...loan, minAmount: amount }, 'reason')).toThrow();
  });
  it('rejects overprecision rates and unsafe integer payloads', () => {
    expect(() => earnProductRequestBody({ ...earn, aprRate: '0.123456789' }, 'reason')).toThrow();
    expect(() => loanProductRequestBody({ ...loan, interestRate: '0.123456789' }, 'reason')).toThrow();
    expect(() => marginProductRequestBody({ ...margin, customLeverageLevels: '2.000000001' }, 'reason')).toThrow();
    expect(() => convertPairRequestBody({ ...pair, fromAssetId: '9007199254740993' }, 'reason')).toThrow();
    expect(() => secondsProductRequestBody({ ...seconds, periods: [{ ...seconds.periods[0], durationSeconds: '4294967296' }] }, 'reason')).toThrow();
  });
});

describe('Semi forms preserve raw financial input', () => {
  afterEach(() => vi.unstubAllGlobals());
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('matchMedia', vi.fn((media: string) => ({
      media, matches: false, onchange: null, addListener: vi.fn(), removeListener: vi.fn(),
      addEventListener: vi.fn(), removeEventListener: vi.fn(), dispatchEvent: vi.fn()
    })));
    vi.mocked(apiRequest).mockResolvedValue({});
    vi.mocked(listAdminResource).mockResolvedValue({ rows: [
      { id: 1, symbol: 'BTC', precision_scale: 18 }, { id: 2, symbol: 'USDT', precision_scale: 18 }
    ], raw: {} });
  });
  it('blocks overflow in the actual Convert form then submits the exact text', async () => {
    const user = userEvent.setup();
    mount(<ConvertPairRowActions helpers={{ reload: vi.fn(), loadDetail: vi.fn(), openDetail: vi.fn() }} record={{
      id: 7, from_asset_id: 1, to_asset_id: 2, pricing_mode: 'fixed', spread_rate: '0.01', fee_rate: '0',
      min_amount: '1', max_amount: null, target_min_amount: '0', target_max_amount: null, enabled: true
    }} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    const input = await screen.findByLabelText('源资产最小金额');
    for (const value of invalidAmounts) {
      fireEvent.change(input, { target: { value } });
      expect(input).toHaveValue(value);
      expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
    }
    expect(apiRequest).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: exact } });
    await user.click(screen.getByRole('button', { name: '提交修改' }));
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: '精度回归' } });
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => expect(apiRequest).toHaveBeenCalledTimes(1));
    expect(JSON.parse(String(vi.mocked(apiRequest).mock.calls[0][1]?.body)).min_amount).toBe(exact);
  });
  it('rejects unsafe agent IDs and overprecision commission rates before confirmation', async () => {
    const user = userEvent.setup();
    mount(<CreateAgentCommissionRuleAction />);
    await user.click(screen.getByRole('button', { name: '添加佣金规则' }));
    fireEvent.change(await screen.findByLabelText('代理ID'), { target: { value: '9007199254740993' } });
    fireEvent.change(screen.getByLabelText('佣金比例'), { target: { value: '0.1' } });
    expect(screen.getByRole('button', { name: '提交添加佣金规则' })).toBeDisabled();
    fireEvent.change(screen.getByLabelText('代理ID'), { target: { value: '1' } });
    fireEvent.change(screen.getByLabelText('佣金比例'), { target: { value: '0.100000001' } });
    expect(screen.getByRole('button', { name: '提交添加佣金规则' })).toBeDisabled();
    expect(apiRequest).not.toHaveBeenCalled();
  });
  it('rejects unsafe SMTP test identities before issuing a test request', async () => {
    const { result } = renderHook(() => useSmtpConfigWorkspace());
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(apiRequest).mockClear();
    for (const id of ['9007199254740993', '1.000000000000000001', 'NaN']) {
      act(() => result.current.setTestConfigChoice(id));
      await expect(result.current.sendTest('数值校验')).rejects.toThrow('安全整数');
    }
    expect(apiRequest).not.toHaveBeenCalled();
  });
  it('uses the configured asset precision without rounding amount controls', async () => {
    const user = userEvent.setup();
    mount(<CreateAssetAction />);
    await user.click(screen.getByRole('button', { name: '添加资产' }));
    fireEvent.change(await screen.findByLabelText('资产符号'), { target: { value: 'BTC' } });
    fireEvent.change(screen.getByLabelText('资产名称'), { target: { value: 'Bitcoin' } });
    fireEvent.change(screen.getByLabelText('充值手续费'), { target: { value: '0.000000001' } });
    expect(screen.getByRole('button', { name: '提交添加资产' })).toBeDisabled();
    fireEvent.change(screen.getByLabelText('充值手续费'), { target: { value: '0.0000000100' } });
    expect(screen.getByRole('button', { name: '提交添加资产' })).toBeEnabled();
    fireEvent.change(screen.getByLabelText('资产精度'), { target: { value: '19' } });
    expect(screen.getByRole('button', { name: '提交添加资产' })).toBeDisabled();
  });
  it('does not turn numeric leverage response entries into authoritative decimal strings', async () => {
    const user = userEvent.setup();
    mount(<MarginProductRowActions helpers={{ reload: vi.fn(), loadDetail: vi.fn(), openDetail: vi.fn() }} record={{
      id: 7, pair_id: 1, margin_asset: 2, margin_mode: 'isolated', margin_modes: ['isolated'],
      leverage_levels: ['5', JSON.parse('2.000000000000000001')],
      min_margin: '1', max_margin: null, maintenance_margin_rate: '0.05', hourly_interest_rate: '0', status: 'active'
    }} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    await user.click(await screen.findByRole('tab', { name: '杠杆档位' }));
    expect(screen.getByRole('button', { name: '下一步' })).toBeDisabled();
    expect(apiRequest).not.toHaveBeenCalled();
  });
  it.each([
    { min_amount: '0', max_amount: null, fee_rate_percent: JSON.parse('0.123456789123456789') },
    { min_amount: '0', max_amount: JSON.parse('1.000000000000000001'), fee_rate_percent: '1' },
    { min_amount: null, max_amount: null, fee_rate_percent: '1' }
  ])('retains an invalid withdrawal tier as a blocked draft instead of rounding or dropping it: %j', async (tier) => {
    const user = userEvent.setup();
    mount(<AssetRowActions helpers={{ reload: vi.fn(), loadDetail: vi.fn(), openDetail: vi.fn() }} record={{
      id: 1, symbol: 'BTC', name: 'Bitcoin', precision_scale: 18, asset_type: 'coin', status: 'active',
      min_deposit_amount: '0', deposit_fee: '0', withdraw_fee: '0', withdraw_fee_tiers: [tier]
    }} />);
    await user.click(screen.getByRole('button', { name: '修改' }));
    expect(await screen.findByLabelText('梯度最小金额 1')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '提交修改' })).toBeDisabled();
    expect(apiRequest).not.toHaveBeenCalled();
  });
});
