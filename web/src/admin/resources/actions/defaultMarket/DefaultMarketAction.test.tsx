import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, expect, it, vi } from 'vitest';
import { apiRequest, ApiError } from '../../../../api/client';
import { AdminAccessProvider } from '../../../access';
import { DefaultMarketAction } from './DefaultMarketAction';
import { MarketPairRowActions } from '../market';
import { responseFixture } from './testFixtures';
import type { DefaultMarketResponse } from './types';

vi.mock('../../../../api/client', async (original) => ({ ...await original<typeof import('../../../../api/client')>(), apiRequest: vi.fn() }));
const record = { id: 3, symbol: 'TESTUSDT', market_type: 'strategy', price_precision: 18, qty_precision: 18 };
const helpers = { loadDetail: vi.fn(), reload: vi.fn(), openDetail: vi.fn() };
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>((done) => { resolve = done; }); return { promise, resolve }; }
function setup(write = true, read = true) {
  const router = createMemoryRouter([{ path: '/', element: <AdminAccessProvider access={{ admin_id: 1, username: 'tester', role_id: 1, role_name: '测试', is_super_admin: false, permissions: [...(read ? ['market.pairs.read'] : []), ...(write ? ['market.pairs.write'] : [])] }}>
    <DefaultMarketAction record={record} helpers={helpers} />
  </AdminAccessProvider> }, { path: '/next', element: <div>其他页面</div> }]);
  render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><RouterProvider router={router} /></QueryClientProvider>);
  return router;
}
async function open() {
  fireEvent.click(screen.getByRole('button', { name: '默认行情' }));
  await screen.findByRole('heading', { name: '读取时的运行状态' });
}
async function confirm(action: string, reason = '测试配置原因') {
  fireEvent.click(screen.getByRole('button', { name: action }));
  fireEvent.change(await screen.findByRole('textbox', { name: '操作原因' }), { target: { value: reason } });
  fireEvent.click(screen.getByRole('button', { name: '确认' }));
}

beforeEach(() => { vi.mocked(apiRequest).mockReset(); helpers.reload.mockReset(); });

it('loads only on open, allows read-only inspection and wires only synthetic pair types', async () => {
  vi.mocked(apiRequest).mockResolvedValue(responseFixture());
  setup(false);
  expect(apiRequest).not.toHaveBeenCalled();
  await open();
  expect(apiRequest).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('textbox', { name: '初始价格' })).toBeDisabled();
  expect(screen.queryByRole('button', { name: '保存默认行情配置' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '暂停全部行情' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '关闭默认行情' }));
  const external = render(<MarketPairRowActions record={{ ...record, market_type: 'external' }} helpers={helpers} />);
  expect(within(external.container).queryByRole('button', { name: '默认行情' })).not.toBeInTheDocument();
  external.rerender(<MarketPairRowActions record={{ ...record, market_type: 'internal' }} helpers={helpers} />);
  expect(within(external.container).getByRole('button', { name: '默认行情' })).toBeInTheDocument();
  expect(apiRequest).toHaveBeenCalledTimes(1);
});

it('saves exact strings and the loaded version with an explicit enable checkbox without clearing pause-all', async () => {
  const loaded = responseFixture({ version: 4, configured: true, all_market_paused: true });
  vi.mocked(apiRequest).mockResolvedValueOnce(loaded).mockResolvedValueOnce({ ...loaded, version: 5, enabled: true, initial_price: '0.000000000000000017' });
  setup(); await open();
  expect(screen.getByRole('checkbox', { name: '启用默认行情' })).not.toBeChecked();
  fireEvent.click(screen.getByRole('checkbox', { name: '启用默认行情' }));
  fireEvent.change(screen.getByRole('textbox', { name: '初始价格' }), { target: { value: '0.000000000000000017' } });
  await confirm('保存默认行情配置', '  启用接续  ');
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledOnce());
  const [path, options] = vi.mocked(apiRequest).mock.calls[1];
  expect(path).toBe('/admin/api/v1/market-pairs/3/default-generator');
  expect(options?.method).toBe('PATCH');
  const body = JSON.parse(String(options?.body));
  expect(body).toMatchObject({ expected_version: 4, enabled: true, initial_price: '0.000000000000000017', reason: '启用接续', config: { volatility: '0.0015', depth_levels: 20 } });
  expect(body).not.toHaveProperty('all_market_paused');
  expect(screen.getByText(/配置版本 V5/)).toHaveTextContent('全部行情已暂停');
});

it('uses a separate reason-confirmed versioned pause-all endpoint and keeps fields out of its body', async () => {
  vi.mocked(apiRequest).mockResolvedValueOnce(responseFixture({ version: 8 })).mockResolvedValueOnce(responseFixture({ version: 9, all_market_paused: true }));
  setup(); await open();
  fireEvent.click(screen.getByRole('button', { name: '暂停全部行情' }));
  expect(screen.getByRole('button', { name: '确认' })).toBeDisabled();
  fireEvent.change(screen.getByRole('textbox', { name: '操作原因' }), { target: { value: '维护暂停' } });
  fireEvent.click(screen.getByRole('button', { name: '确认' }));
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledOnce());
  expect(vi.mocked(apiRequest).mock.calls[1][0]).toBe('/admin/api/v1/market-pairs/3/default-generator/pause-all');
  expect(JSON.parse(String(vi.mocked(apiRequest).mock.calls[1][1]?.body))).toEqual({ expected_version: 8, all_market_paused: true, reason: '维护暂停' });
  expect(screen.getByRole('button', { name: '恢复全部行情' })).toBeEnabled();
});

it('invalidates in-flight and settled previews on every edit and renders server OHLCV only', async () => {
  const delayed = deferred<unknown>();
  const preview = { pair_id: 3, version: 1, seed: 'default-market:3', start_price: '0.1', samples: [{ open_time: 1788700020000, open: '0.1', high: '0.11', low: '0.09', close: '0.105', volume: '10' }] };
  vi.mocked(apiRequest).mockResolvedValueOnce(responseFixture()).mockReturnValueOnce(delayed.promise).mockResolvedValueOnce(preview);
  setup(); await open();
  fireEvent.click(screen.getByRole('button', { name: '生成默认行情预览' }));
  fireEvent.change(screen.getByRole('textbox', { name: '波动率' }), { target: { value: '0.002' } });
  await act(async () => delayed.resolve(preview));
  expect(screen.queryByRole('region', { name: '默认行情预览' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '生成默认行情预览' }));
  expect(await screen.findByRole('table', { name: '默认行情 OHLCV 预览样本' })).toHaveTextContent('0.105');
  const body = JSON.parse(String(vi.mocked(apiRequest).mock.calls[2][1]?.body));
  expect(body).toMatchObject({ expected_version: 0, initial_price: null, config: { volatility: '0.002' } });
  expect(body).not.toHaveProperty('enabled');
  fireEvent.change(screen.getByRole('textbox', { name: '盘口档数' }), { target: { value: '12' } });
  expect(screen.queryByRole('table', { name: '默认行情 OHLCV 预览样本' })).not.toBeInTheDocument();
});

it('retains draft and reason after 409, forbids silent retry and requires explicit discard/reload', async () => {
  vi.mocked(apiRequest).mockResolvedValueOnce(responseFixture({ version: 1 })).mockRejectedValueOnce(new ApiError(409, 'CONFLICT', '配置版本已变化')).mockResolvedValueOnce(responseFixture({ version: 3 }));
  setup(); await open();
  fireEvent.change(screen.getByRole('textbox', { name: '初始价格' }), { target: { value: '12.123456789012345678' } });
  await confirm('保存默认行情配置', '保留理由');
  await screen.findByText(/配置版本已变更，草稿已保留/);
  expect(screen.getByRole('textbox', { name: '操作原因' })).toHaveValue('保留理由');
  fireEvent.click(screen.getByRole('button', { name: '取消' }));
  expect(screen.getByRole('textbox', { name: '初始价格' })).toHaveValue('12.123456789012345678');
  expect(screen.getByRole('button', { name: '保存默认行情配置' })).toBeDisabled();
  expect(apiRequest).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole('button', { name: '重新加载最新配置' }));
  expect(await screen.findByText('放弃默认行情草稿？')).toBeInTheDocument();
  expect(apiRequest).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole('button', { name: '放弃草稿' }));
  await screen.findByText(/配置版本 V3/);
  expect(screen.getByRole('textbox', { name: '初始价格' })).toHaveValue('');
});

it('isolates an old GET after closing and reopening and aborts its transport', async () => {
  const old = deferred<DefaultMarketResponse>();
  vi.mocked(apiRequest).mockReturnValueOnce(old.promise).mockResolvedValueOnce(responseFixture({ version: 6 }));
  setup();
  fireEvent.click(screen.getByRole('button', { name: '默认行情' }));
  const signal = vi.mocked(apiRequest).mock.calls[0][1]?.signal;
  fireEvent.click(screen.getByRole('button', { name: '关闭默认行情' }));
  expect(signal?.aborted).toBe(true);
  await open();
  await act(async () => old.resolve(responseFixture({ version: 2 })));
  expect(screen.getByText(/配置版本 V6/)).toBeInTheDocument();
  expect(screen.queryByText(/配置版本 V2/)).not.toBeInTheDocument();
});

it('guards dirty close, navigation and browser unload without sending writes', async () => {
  vi.mocked(apiRequest).mockResolvedValue(responseFixture());
  const router = setup(); await open();
  fireEvent.change(screen.getByRole('textbox', { name: '波动率' }), { target: { value: '0.02' } });
  const event = new Event('beforeunload', { cancelable: true });
  window.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
  fireEvent.click(screen.getByRole('button', { name: '关闭默认行情' }));
  fireEvent.click(await screen.findByRole('button', { name: '继续编辑' }));
  expect(screen.getByRole('textbox', { name: '波动率' })).toHaveValue('0.02');
  await act(async () => { await router.navigate('/next'); });
  expect(await screen.findByText('确认离开当前页面')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '继续编辑' }));
  expect(router.state.location.pathname).toBe('/');
  expect(apiRequest).toHaveBeenCalledTimes(1);
});

function selectControl(label: string): HTMLElement {
  return screen.getByRole('combobox', { name: label });
}
async function choose(label: string, option: string) {
  fireEvent.click(selectControl(label));
  fireEvent.click(await screen.findByText(option));
}
const btcReference = { id: 8, symbol: 'BTCUSDT', status: 'active', market_type: 'external' };
const followConfig = { ...responseFixture().config, mode: 'follow' as const, follow: { reference_pair_id: 8, multiplier: '1', max_move_ratio: '0.05', stale_after_seconds: 60 } };

it('loads references only after explicitly selecting follow and saves the chosen BTC ID with exact ratios', async () => {
  vi.mocked(apiRequest).mockResolvedValueOnce(responseFixture({ version: 4 })).mockResolvedValueOnce({ pairs: [btcReference], total: 1 }).mockResolvedValueOnce(responseFixture({ version: 5, config: followConfig, reference_pair: btcReference }));
  setup(); await open();
  expect(apiRequest).toHaveBeenCalledTimes(1);
  await choose('生成模式', '跟随外部行情');
  await waitFor(() => expect(apiRequest).toHaveBeenCalledTimes(2));
  expect(screen.getByRole('heading', { name: '参考过期时的独立震荡配置' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '保存默认行情配置' })).toBeDisabled();
  await waitFor(() => expect(selectControl('参考交易对')).not.toHaveClass('semi-select-disabled'));
  expect(selectControl('参考交易对')).toHaveTextContent('搜索并明确选择');
  await choose('参考交易对', 'BTCUSDT（ID: 8）');
  fireEvent.change(screen.getByRole('textbox', { name: '跟随倍率' }), { target: { value: '1.000000000000000017' } });
  await confirm('保存默认行情配置');
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledOnce());
  const body = JSON.parse(String(vi.mocked(apiRequest).mock.calls[2][1]?.body));
  expect(body.expected_version).toBe(4);
  expect(body.config).toMatchObject({ mode: 'follow', follow: { reference_pair_id: 8, multiplier: '1.000000000000000017', max_move_ratio: '0.05', stale_after_seconds: 60 } });
});

it('fails closed on reference directory errors without preventing an explicit independent save', async () => {
  vi.mocked(apiRequest).mockResolvedValueOnce(responseFixture()).mockRejectedValueOnce(new ApiError(403, 'FORBIDDEN', '缺少读取权限')).mockResolvedValueOnce(responseFixture({ version: 1 }));
  setup(); await open();
  await choose('生成模式', '跟随外部行情');
  await screen.findByText(/参考交易对目录加载失败/);
  expect(screen.getByRole('button', { name: '生成默认行情预览' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '保存默认行情配置' })).toBeDisabled();
  await choose('生成模式', '独立震荡');
  fireEvent.change(screen.getByRole('textbox', { name: '波动率' }), { target: { value: '0.03' } });
  await confirm('保存默认行情配置');
  await waitFor(() => expect(helpers.reload).toHaveBeenCalledOnce());
  expect(JSON.parse(String(vi.mocked(apiRequest).mock.calls[2][1]?.body)).config).toMatchObject({ mode: 'independent', follow: null, volatility: '0.03' });
});

it('retains an invalid saved reference visibly rather than choosing the first eligible pair', async () => {
  vi.mocked(apiRequest).mockResolvedValueOnce(responseFixture({ config: followConfig, reference_pair: { ...btcReference, status: 'disabled' } })).mockResolvedValueOnce({ pairs: [{ ...btcReference, id: 9, symbol: 'ETHUSDT' }], total: 1 });
  setup(); await open();
  await screen.findByText(/请选择仍启用的外部参考交易对/);
  expect(selectControl('参考交易对')).toHaveTextContent('BTCUSDT（尚未通过目录校验）');
  fireEvent.change(screen.getByRole('textbox', { name: '波动率' }), { target: { value: '0.03' } });
  expect(screen.getByRole('button', { name: '保存默认行情配置' })).toBeDisabled();
  await choose('参考交易对', 'ETHUSDT（ID: 9）');
  expect(screen.getByRole('button', { name: '保存默认行情配置' })).toBeEnabled();
  expect(apiRequest).toHaveBeenCalledTimes(2);
});

it('read-only users see configured versus last effective follow state and its reference evidence without loading options', async () => {
  vi.mocked(apiRequest).mockResolvedValue(responseFixture({ config: followConfig, reference_pair: btcReference,
    runtime: { ...responseFixture().runtime, active_source: 'default', follow: { mode: 'fallback', reference_pair_id: 8, reference_symbol: 'BTCUSDT', reference_price: '67890.000000000000000017', reference_observed_at: 1788700020000, fallback_reason: '参考价格已过期', switched_at: 1788700080000 } } }));
  setup(false); await open();
  expect(screen.getByText(/已保存模式：跟随外部行情/)).toBeInTheDocument();
  expect(screen.getByText(/最近默认生成状态：独立震荡兜底/)).toBeInTheDocument();
  expect(screen.getByRole('alert')).toHaveTextContent('参考价格已过期');
  expect(screen.getByText(/参考价格：67890.000000000000000017/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '保存默认行情配置' })).not.toBeInTheDocument();
  expect(apiRequest).toHaveBeenCalledTimes(1);
});

it('does not query references without read permission and still permits an independent draft', async () => {
  vi.mocked(apiRequest).mockResolvedValue(responseFixture());
  setup(true, false); await open();
  await choose('生成模式', '跟随外部行情');
  expect(screen.getByText(/缺少参考交易对读取权限/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '保存默认行情配置' })).toBeDisabled();
  expect(apiRequest).toHaveBeenCalledTimes(1);
  await choose('生成模式', '独立震荡');
  fireEvent.change(screen.getByRole('textbox', { name: '波动率' }), { target: { value: '0.03' } });
  expect(screen.getByRole('button', { name: '保存默认行情配置' })).toBeEnabled();
});

it('names mode and reference comboboxes through their visible labels', async () => {
  vi.mocked(apiRequest).mockResolvedValueOnce(responseFixture({ config: followConfig, reference_pair: btcReference })).mockResolvedValueOnce({ pairs: [btcReference], total: 1 });
  setup(); await open();
  const mode = screen.getByRole('combobox', { name: '生成模式' });
  const reference = screen.getByRole('combobox', { name: '参考交易对' });
  for (const [control, label] of [[mode, '生成模式'], [reference, '参考交易对']] as const) {
    const labelledBy = control.getAttribute('aria-labelledby');
    expect(labelledBy).toBeTruthy();
    expect(document.getElementById(labelledBy!)).toHaveTextContent(label);
  }
  await choose('生成模式', '独立震荡');
  expect(screen.queryByRole('combobox', { name: '参考交易对' })).not.toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: '生成模式' })).toHaveTextContent('独立震荡');
});

it('separates always-applied price quantity depth and enable controls from fallback oscillation', async () => {
  vi.mocked(apiRequest).mockResolvedValue(responseFixture({ config: followConfig, reference_pair: btcReference }));
  setup(false); await open();
  const shared = screen.getByRole('region', { name: '通用生成参数' });
  const fallback = screen.getByRole('region', { name: '独立震荡参数' });
  expect(within(shared).getByRole('heading')).toHaveTextContent('独立与跟随均适用');
  expect(within(shared).getByRole('checkbox', { name: '启用默认行情' })).toBeInTheDocument();
  for (const label of ['初始价格', '盘口档数', '价格下限（可选）', '价格上限（可选）', '分钟成交量下限', '分钟成交量上限']) {
    expect(within(shared).getByRole('textbox', { name: label })).toBeInTheDocument();
    expect(within(fallback).queryByRole('textbox', { name: label })).not.toBeInTheDocument();
  }
  expect(within(fallback).getByRole('heading')).toHaveTextContent('参考过期时的独立震荡配置');
  for (const label of ['波动率', '均值回归强度', '影线强度']) {
    expect(within(fallback).getByRole('textbox', { name: label })).toBeInTheDocument();
    expect(within(shared).queryByRole('textbox', { name: label })).not.toBeInTheDocument();
  }
});
