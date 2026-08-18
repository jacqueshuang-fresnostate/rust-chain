import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { listAdminResource } from '../../api/adminResources';
import { apiRequest } from '../../api/client';
import { MarketStrategyActions } from './MarketStrategyActions';

vi.mock('../../api/adminResources', () => ({
  listAdminResource: vi.fn()
}));

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const listAdminResourceMock = vi.mocked(listAdminResource);
const apiRequestMock = vi.mocked(apiRequest);

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function stubResizeObserver() {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'ResizeObserver');
  if (descriptor?.configurable === false) {
    if ('writable' in descriptor && descriptor.writable) {
      (globalThis as typeof globalThis & { ResizeObserver: typeof ResizeObserverMock }).ResizeObserver = ResizeObserverMock;
    }
    return;
  }
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
}

function semiSelectByLabel(root: HTMLElement, label: string): HTMLElement {
  const labelNode = [...root.querySelectorAll('label')].find(
    (item) => item.textContent?.trim().startsWith(label) && item.querySelector('.semi-select')
  );
  expect(labelNode).toBeDefined();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

async function selectSemiOption(
  user: ReturnType<typeof userEvent.setup>,
  root: HTMLElement,
  label: string,
  optionLabel: string,
  excludedOptionLabel?: string
) {
  await user.click(semiSelectByLabel(root, label));
  await waitFor(() => {
    expect([...document.querySelectorAll('.semi-select-option')].some((item) => item.textContent === optionLabel)).toBe(true);
  });
  if (excludedOptionLabel) {
    expect([...document.querySelectorAll('.semi-select-option')].some((item) => item.textContent === excludedOptionLabel)).toBe(false);
  }
  const option = [...document.querySelectorAll('.semi-select-option')]
    .filter((item) => item.textContent === optionLabel)
    .at(-1) as HTMLElement;
  expect(option).toBeDefined();
  fireEvent.mouseDown(option);
  fireEvent.mouseUp(option);
  fireEvent.click(option);
  await waitFor(() => expect(semiSelectByLabel(root, label)).toHaveTextContent(optionLabel));
}

describe('MarketStrategyActions', () => {
  beforeEach(() => {
    stubResizeObserver();
    listAdminResourceMock.mockReset();
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue({});
    listAdminResourceMock.mockImplementation(async (endpoint, responseKey) => {
      if (endpoint === '/admin/api/v1/market-pairs') {
        const pairs = [
          { id: 21, symbol: 'BTC-USDT', status: 'active', market_type: 'internal' },
          { id: 22, symbol: 'NEW-USDT', status: 'active', market_type: 'strategy' },
          { id: 23, symbol: 'ETH-USDT', status: 'active', market_type: 'external' }
        ];
        return { rows: pairs, raw: { pairs } };
      }

      const strategies = [
        {
          id: 91,
          pair_id: 21,
          symbol: 'BTC-USDT',
          strategy_type: 'price_path',
          start_price: '1.000000000000000000',
          target_price: '2.000000000000000000',
          status: 'paused',
          run_status: 'paused',
          created_at: 1_775_027_600_000
        }
      ];
      return { rows: strategies, raw: { [responseKey]: strategies } };
    });
  });

  it('renders strategy actions as a resource table page', async () => {
    render(<MarketStrategyActions />);

    expect(await screen.findByText('行情策略')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '创建策略' })).toBeInTheDocument();
    expect(screen.getByText('BTC-USDT', { selector: 'span' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '检测缺口/补偿K线（策略91）' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '版本历史' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '启用' })).toBeInTheDocument();
    expect(screen.queryByText('更新策略状态')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '查看JSON' })).not.toBeInTheDocument();
  });

  it('loads the strategy detail before editing so configured nodes are retained', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation(async (path, init) => {
      if (path === '/admin/api/v1/market-strategies/91') {
        return {
          id: 91,
          pair_id: 21,
          strategy_type: 'price_path',
          start_price: '1',
          target_price: '2',
          start_time: new Date('2026-08-12T10:00').getTime(),
          end_time: new Date('2026-08-12T11:00').getTime(),
          volatility: '0.01',
          volume_min: '10',
          volume_max: '20',
          status: 'paused',
          generator: {
            scenario: 'high_volatility',
            seed_mode: 'fixed',
            seed: 'stable-seed',
            mean_reversion_strength: '1.2',
            noise_scale: '2.4',
            wick_scale: '1.8',
            volume_shape: 'bell'
          },
          nodes: [
            {
              sequence_no: 0,
              target_time: new Date('2026-08-12T10:30').getTime(),
              target_type: 'absolute_price',
              target_value: '1.5',
              execution_mode: 'hard',
              tolerance: '0',
              volatility: '0.01',
              volume_min: '10',
              volume_max: '20'
            }
          ]
        };
      }
      if (path === '/admin/api/v1/market-strategies/preview' && init?.method === 'POST') {
        return {
          one_minute_count: 60,
          preview_seed: 'stable-seed',
          preview_version: 2,
          sample_count: 1,
          samples: [{ open_time: new Date('2026-08-12T10:00').getTime(), open: '1', high: '1.1', low: '0.9', close: '1.01', volume: '10' }]
        };
      }
      return {};
    });

    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '修改' }));

    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-strategies/91'));
    expect(await screen.findByText('节点1')).toBeInTheDocument();
    expect(screen.getByDisplayValue('1.5')).toBeInTheDocument();
    expect(semiSelectByLabel(document.body, '行情场景')).toHaveTextContent('高波动');
    expect(semiSelectByLabel(document.body, 'Seed 模式')).toHaveTextContent('固定 Seed');
    expect(screen.getByDisplayValue('stable-seed')).toBeInTheDocument();
    expect(screen.getByDisplayValue('2.4')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '生成 OHLCV 预览' }));
    expect(await screen.findByText('V2')).toBeInTheDocument();
    const previewCall = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/market-strategies/preview');
    expect(JSON.parse(String(previewCall?.[1]?.body))).toMatchObject({
      strategy_id: 91,
      pair_id: 21,
      generator: { seed_mode: 'fixed', seed: 'stable-seed' }
    });
    expect(JSON.parse(String(previewCall?.[1]?.body))).not.toHaveProperty('reason');
  });

  it('applies a backend preset and generates a side-effect-free OHLCV preview', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation(async (path, init) => {
      if (path === '/admin/api/v1/market-strategies/presets') {
        return {
          presets: [
            {
              code: 'custom_path',
              name: '自定义路径',
              description: '自定义预设说明',
              target_price_change_percent: '0',
              generator: {
                scenario: 'custom_path',
                seed_mode: 'auto',
                mean_reversion_strength: '0.55',
                noise_scale: '1',
                wick_scale: '0.75',
                volume_shape: 'uniform'
              },
              nodes: []
            },
            {
              code: 'trend_up',
              name: '稳步上涨',
              description: '后端预设说明',
              target_price_change_percent: '25',
              generator: {
                scenario: 'trend_up',
                seed_mode: 'auto',
                mean_reversion_strength: '0.45',
                noise_scale: '0.8',
                wick_scale: '0.6',
                volume_shape: 'trend'
              },
              nodes: [
                {
                  progress_percent: 50,
                  target_type: 'percent_from_start',
                  target_value: '12',
                  execution_mode: 'soft',
                  tolerance: '1',
                  volatility: '0.008',
                  volume_min: null,
                  volume_max: null
                }
              ]
            }
          ]
        };
      }
      if (path === '/admin/api/v1/market-strategies/preview' && init?.method === 'POST') {
        return {
          one_minute_count: 60,
          preview_seed: 'preview-seed',
          preview_version: 1,
          sample_count: 2,
          samples: [
            { open_time: 1_786_500_000_000, open: '100', high: '101', low: '99', close: '100.5', volume: '10' },
            { open_time: 1_786_503_540_000, open: '124', high: '126', low: '123', close: '125', volume: '20' }
          ]
        };
      }
      return {};
    });

    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '创建策略' }));
    const sheet = (await screen.findByText('创建策略', { selector: '.semi-sidesheet-title' })).closest('.semi-sidesheet-inner') as HTMLElement;
    await waitFor(() => {
      expect(listAdminResourceMock).toHaveBeenCalledWith('/admin/api/v1/market-pairs', 'pairs', { status: 'active', limit: 100 });
    });
    expect(semiSelectByLabel(sheet, '策略类型')).toHaveTextContent('价格路径（OHLCV）');
    await selectSemiOption(user, sheet, '交易对ID', 'BTC-USDT（ID: 21）', 'ETH-USDT（ID: 23）');
    fireEvent.change(within(sheet).getByLabelText('起始价'), { target: { value: '100' } });
    fireEvent.change(within(sheet).getByLabelText('目标价'), { target: { value: '100' } });
    fireEvent.change(within(sheet).getByLabelText('开始时间'), { target: { value: '2026-08-12T10:00' } });
    fireEvent.change(within(sheet).getByLabelText('结束时间'), { target: { value: '2026-08-12T11:00' } });

    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-strategies/presets'));
    await waitFor(() => expect(within(sheet).getByRole('button', { name: '应用场景预设' })).toBeEnabled());
    await selectSemiOption(user, sheet, '行情场景', '稳步上涨');
    await user.click(within(sheet).getByRole('button', { name: '应用场景预设' }));

    expect(within(sheet).getByLabelText('目标价')).toHaveValue('125');
    expect(within(sheet).getByLabelText('均值回归强度')).toHaveValue('0.45');
    expect(within(sheet).getByLabelText('噪声强度')).toHaveValue('0.8');
    expect(within(sheet).getByLabelText('影线强度')).toHaveValue('0.6');
    expect(within(sheet).getByText('节点1')).toBeInTheDocument();
    expect(semiSelectByLabel(sheet, '成交量形态')).toHaveTextContent('随时间递增');

    await user.click(within(sheet).getByRole('button', { name: '生成 OHLCV 预览' }));
    expect(await screen.findByText('无副作用预览')).toBeInTheDocument();
    expect(screen.getByText('preview-seed')).toBeInTheDocument();
    expect(screen.getByText('V1')).toBeInTheDocument();
    expect(screen.getByRole('table', { name: 'OHLCV 预览样本' })).toBeInTheDocument();
    const previewCall = apiRequestMock.mock.calls.find(([path]) => path === '/admin/api/v1/market-strategies/preview');
    expect(JSON.parse(String(previewCall?.[1]?.body))).toMatchObject({
      pair_id: 21,
      strategy_type: 'price_path',
      target_price: '125',
      sample_count: 120,
      generator: {
        scenario: 'trend_up',
        seed_mode: 'auto',
        seed: null,
        regenerate_seed: false,
        mean_reversion_strength: '0.45',
        noise_scale: '0.8',
        wick_scale: '0.6',
        volume_shape: 'trend'
      }
    });
    expect(JSON.parse(String(previewCall?.[1]?.body))).not.toHaveProperty('reason');
  });

  it('loads immutable versions and restores an old snapshot by copying it', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation(async (path) => {
      if (path === '/admin/api/v1/market-strategies/91/versions?limit=100&offset=0') {
        return {
          total: 2,
          versions: [
            {
              version: 2,
              effective_time: 1_775_031_200_000,
              seed: 'seed-v2',
              created_by: 8,
              created_at: 1_775_031_200_000,
              active: true,
              generator: { scenario: 'range', seed_mode: 'auto' }
            },
            {
              version: 1,
              effective_time: 1_775_027_600_000,
              seed: 'seed-v1',
              created_by: 7,
              created_at: 1_775_027_600_000,
              active: false,
              generator: { scenario: 'trend_up', seed_mode: 'fixed' }
            }
          ]
        };
      }
      return {};
    });

    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '版本历史' }));
    expect(await screen.findByText('不可变配置版本')).toBeInTheDocument();
    expect(screen.getByText('版本 2')).toBeInTheDocument();
    expect(screen.getByText('当前激活')).toBeInTheDocument();
    expect(screen.getByText('版本 1')).toBeInTheDocument();
    expect(screen.getByText('稳步上涨')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '复制为新版本' }));
    await user.type(await screen.findByLabelText('操作原因'), '恢复稳定版本');
    await user.click(screen.getByRole('button', { name: '确认' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/market-strategies/91/versions/1/restore', {
        method: 'POST',
        body: JSON.stringify({ reason: '恢复稳定版本' })
      });
    });
  });

  it('keeps create submission disabled until the strategy range is valid', async () => {
    const user = userEvent.setup();
    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '创建策略' }));

    const sheet = (await screen.findByText('创建策略', { selector: '.semi-sidesheet-title' })).closest('.semi-sidesheet-inner') as HTMLElement;
    await selectSemiOption(user, sheet, '交易对ID', 'NEW-USDT（ID: 22）');
    fireEvent.change(within(sheet).getByLabelText('起始价'), { target: { value: '1' } });
    fireEvent.change(within(sheet).getByLabelText('目标价'), { target: { value: '2' } });
    fireEvent.change(within(sheet).getByLabelText('开始时间'), { target: { value: '2026-08-12T10:00' } });
    fireEvent.change(within(sheet).getByLabelText('结束时间'), { target: { value: '2026-08-12T10:00' } });

    const submit = within(sheet).getByRole('button', { name: '提交创建策略' });
    expect(submit).toBeDisabled();

    fireEvent.change(within(sheet).getByLabelText('结束时间'), { target: { value: '2026-08-12T11:00' } });
    expect(submit).toBeEnabled();
  });

  it('keeps edit submission disabled for boundary, duplicate, or descending node times', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockResolvedValueOnce({
      id: 91,
      pair_id: 21,
      strategy_type: 'price_path',
      start_price: '1',
      target_price: '2',
      start_time: new Date('2026-08-12T10:00').getTime(),
      end_time: new Date('2026-08-12T11:00').getTime(),
      volatility: '0.01',
      volume_min: '10',
      volume_max: '20',
      status: 'paused',
      nodes: [
        {
          sequence_no: 0,
          target_time: new Date('2026-08-12T10:20').getTime(),
          target_type: 'absolute_price',
          target_value: '1.2',
          execution_mode: 'hard',
          tolerance: '0',
          volatility: '0.01',
          volume_min: null,
          volume_max: null
        },
        {
          sequence_no: 1,
          target_time: new Date('2026-08-12T10:40').getTime(),
          target_type: 'absolute_price',
          target_value: '1.4',
          execution_mode: 'hard',
          tolerance: '0',
          volatility: '0.01',
          volume_min: null,
          volume_max: null
        }
      ]
    });

    render(<MarketStrategyActions />);
    await user.click(await screen.findByRole('button', { name: '修改' }));
    const sheet = (await screen.findByText('修改行情策略', { selector: '.semi-sidesheet-title' })).closest('.semi-sidesheet-inner') as HTMLElement;
    const submit = within(sheet).getByRole('button', { name: '提交修改' });
    const nodeTimes = within(sheet).getAllByLabelText(/节点\d+目标时间/);

    expect(submit).toBeEnabled();

    fireEvent.change(nodeTimes[0], { target: { value: '2026-08-12T10:00' } });
    expect(submit).toBeDisabled();

    fireEvent.change(nodeTimes[0], { target: { value: '2026-08-12T10:30' } });
    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T10:30' } });
    expect(submit).toBeDisabled();

    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T10:20' } });
    expect(submit).toBeDisabled();

    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T10:45' } });
    expect(submit).toBeEnabled();

    fireEvent.change(nodeTimes[1], { target: { value: '2026-08-12T11:00' } });
    expect(submit).toBeDisabled();
  });
});
