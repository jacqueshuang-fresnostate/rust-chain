import { render, screen, waitFor, within } from '@testing-library/react';
import { Toast } from '@douyinfe/semi-ui';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError, apiRequest } from '../../api/client';
import { PredictionSettingsPage, PredictionSyncPage } from './PredictionConfigPage';

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const apiRequestMock = vi.mocked(apiRequest);

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function matchMediaMock(query: string): MediaQueryList {
  return {
    addEventListener: vi.fn(),
    addListener: vi.fn(),
    dispatchEvent: vi.fn(),
    matches: false,
    media: query,
    onchange: null,
    removeEventListener: vi.fn(),
    removeListener: vi.fn()
  };
}

const originalResizeObserver = globalThis.ResizeObserver;
const originalMatchMedia = window.matchMedia;

const settingsResponse = {
  sync_enabled: true,
  sync_interval_seconds: 120,
  sync_tags: ['crypto', 'politics'],
  allowed_asset_ids: [1],
  default_fee_rate: '0.02',
  default_settlement_mode: 'manual_confirm',
  default_invalid_refund_policy: 'refund_stake_and_fee',
  quote_ttl_seconds: 30,
  revision: 7,
  last_sync_status: 'success',
  last_sync_error: null,
  last_sync_started_at: 1_735_732_700_000,
  last_sync_finished_at: 1_735_732_760_000,
  last_successful_sync_at: 1_735_732_760_000,
  last_sync_imported_count: 3,
  last_sync_updated_count: 4
};

const assetConfigsResponse = {
  configs: [
    { asset_id: 1, asset_symbol: 'USDT', enabled: true, max_payout_amount: '1000', revision: 3, updated_at: 1_735_732_800_000 },
    { asset_id: 2, asset_symbol: 'BTC', enabled: false, max_payout_amount: '0', revision: 0, updated_at: 1_735_732_900_000 }
  ]
};

const syncLogsResponse = {
  logs: [
    {
      id: 10,
      trigger_type: 'manual',
      status: 'success',
      imported_count: 3,
      updated_count: 4,
      error_message: null,
      started_at: 1_735_732_700_000,
      finished_at: 1_735_732_760_000
    }
  ]
};

function renderPredictionPage(page: ReactElement, initialEntry = '/admin/prediction/settings') {
  return render(<MemoryRouter initialEntries={[initialEntry]}>{page}</MemoryRouter>);
}

describe('PredictionConfigPage', () => {
  beforeEach(() => {
    if (!globalThis.ResizeObserver) {
      Object.defineProperty(globalThis, 'ResizeObserver', {
        configurable: true,
        value: ResizeObserverMock
      });
    }
    if (!window.matchMedia) {
      Object.defineProperty(window, 'matchMedia', {
        configurable: true,
        writable: true,
        value: matchMediaMock
      });
    }
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/prediction/settings' && !init?.method) {
        return Promise.resolve(settingsResponse);
      }
      if (path === '/admin/api/v1/prediction/asset-configs' && !init?.method) {
        return Promise.resolve(assetConfigsResponse);
      }
      if (path === '/admin/api/v1/prediction/sync/logs?limit=20' && !init?.method) {
        return Promise.resolve(syncLogsResponse);
      }
      if (path === '/admin/api/v1/prediction/settings' && init?.method === 'PATCH') {
        const body = JSON.parse(String(init.body));
        return Promise.resolve({ ...settingsResponse, ...body, revision: body.revision + 1 });
      }
      if (path === '/admin/api/v1/prediction/asset-configs' && init?.method === 'POST') {
        const body = JSON.parse(String(init.body));
        return Promise.resolve({
          asset_id: body.asset_id,
          asset_symbol: body.asset_id === 1 ? 'USDT' : 'BTC',
          enabled: body.enabled,
          max_payout_amount: body.max_payout_amount,
          revision: body.revision + 1,
          updated_at: 1_735_733_000_000
        });
      }
      if (path === '/admin/api/v1/prediction/sync' && init?.method === 'POST') {
        return Promise.resolve({});
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    if (!originalResizeObserver) {
      Reflect.deleteProperty(globalThis, 'ResizeObserver');
    }
    if (!originalMatchMedia) {
      Reflect.deleteProperty(window, 'matchMedia');
    }
  });

  it('renders prediction config as a Semi workbench with separated tabs and contained tables', async () => {
    const user = userEvent.setup();

    renderPredictionPage(<PredictionSettingsPage />);

    expect(await screen.findByText('竞猜配置')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '全局策略' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '下注资产' })).toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: '同步任务' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '前往同步运行' })).toBeInTheDocument();
    expect(screen.getByText('同步来源')).toBeInTheDocument();
    expect(screen.getByText('交易与结算')).toBeInTheDocument();
    expect(screen.getByLabelText('同步间隔秒数')).toHaveValue(120);
    expect(screen.getByLabelText('报价有效秒数')).toHaveValue(30);
    expect(screen.getByLabelText('默认手续费率')).toHaveValue(0.02);
    expect(screen.getAllByText('USDT').length).toBeGreaterThan(0);

    await user.click(screen.getByRole('tab', { name: '下注资产' }));

    expect(screen.getByText('已启用 1')).toBeInTheDocument();
    const assetTable = screen.getByRole('grid', { name: '竞猜下注资产配置表' });
    const tableWrapper = assetTable.closest('.semi-table-wrapper');
    expect(tableWrapper).toHaveStyle({ maxWidth: '100%', width: '100%' });
    expect(tableWrapper).toHaveClass('admin-resizable-table');
    expect(within(assetTable).getByRole('separator', { name: '调整资产列宽' })).toBeInTheDocument();
    expect(within(assetTable).getByRole('separator', { name: '调整默认最大赔付列宽' })).toBeInTheDocument();
    expect(within(assetTable).getByRole('separator', { name: '调整操作列宽' })).toHaveAttribute('aria-valuemin', '120');
    expect(assetTable.querySelector('.react-resizable-handle')).not.toBeInTheDocument();
    expect(within(assetTable).getByRole('columnheader', { name: '资产' })).toBeInTheDocument();
    expect(within(assetTable).getByRole('columnheader', { name: '状态' })).toBeInTheDocument();
    expect(within(assetTable).getByRole('columnheader', { name: '默认最大赔付' })).toBeInTheDocument();
    expect(within(assetTable).getByRole('columnheader', { name: '操作' })).toHaveClass('admin-table-action-column');
    expect(within(assetTable).getAllByRole('button', { name: '保存' })[0].closest('td')).toHaveClass('admin-table-action-column');
    expect(screen.getByRole('switch', { name: 'BTC 允许下注' })).not.toBeChecked();
    expect(screen.getByLabelText('USDT 默认最大赔付')).toHaveValue(1000);

    expect(apiRequestMock.mock.calls.some(([path]) => path === '/admin/api/v1/prediction/sync/logs?limit=20')).toBe(false);
  });

  it('opens the canonical asset editor when the tab query is present', async () => {
    renderPredictionPage(<PredictionSettingsPage />, '/admin/prediction/settings?tab=assets');

    expect(await screen.findByRole('grid', { name: '竞猜下注资产配置表' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '下注资产' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByText('同步来源')).not.toBeInTheDocument();
  });

  it('keeps synchronization execution and logs in the dedicated runtime workspace', async () => {
    const user = userEvent.setup();

    renderPredictionPage(<PredictionSyncPage />);

    expect(await screen.findByText('竞猜同步运行')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '返回竞猜配置' })).toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: '全局策略' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('同步间隔秒数')).not.toBeInTheDocument();
    expect(apiRequestMock.mock.calls.some(([path]) => path === '/admin/api/v1/prediction/asset-configs')).toBe(false);

    const syncTable = screen.getByRole('grid', { name: '竞猜同步日志表' });
    expect(syncTable.closest('.semi-table-wrapper')).toHaveStyle({ maxWidth: '100%', width: '100%' });
    expect(within(syncTable).getByRole('separator', { name: '调整触发方式列宽' })).toBeInTheDocument();
    expect(within(syncTable).getByRole('separator', { name: '调整错误信息列宽' })).toBeInTheDocument();
    expect(within(syncTable).getByText('手动触发')).toBeInTheDocument();
    expect(within(syncTable).getByText('成功')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '立即同步' }));
    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/prediction/sync', { method: 'POST' });
    });
    await waitFor(() => {
      expect(apiRequestMock.mock.calls.filter(([path]) => path === '/admin/api/v1/prediction/sync/logs?limit=20')).toHaveLength(2);
    });
  });

  it('submits trimmed reasons and loaded revisions for global and asset configuration writes', async () => {
    const user = userEvent.setup();

    renderPredictionPage(<PredictionSettingsPage />);

    await user.clear(await screen.findByLabelText('同步间隔秒数'));
    await user.type(screen.getByLabelText('同步间隔秒数'), '300');
    await user.clear(screen.getByLabelText('Polymarket 标签或分类'));
    await user.type(screen.getByLabelText('Polymarket 标签或分类'), 'sports\ncrypto');
    await user.click(screen.getByRole('button', { name: '保存全局策略' }));
    await user.type(await screen.findByLabelText('操作原因'), '  调整同步策略  ');
    await user.click(await screen.findByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/prediction/settings', expect.objectContaining({ method: 'PATCH' }));
    });
    const settingsRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/prediction/settings' && init?.method === 'PATCH')?.[1];
    expect(JSON.parse(String(settingsRequest?.body))).toEqual({
      sync_enabled: true,
      sync_interval_seconds: 300,
      sync_tags: ['sports', 'crypto'],
      allowed_asset_ids: [1],
      default_fee_rate: '0.02',
      default_settlement_mode: 'manual_confirm',
      default_invalid_refund_policy: 'refund_stake_and_fee',
      quote_ttl_seconds: 30,
      revision: 7,
      reason: '调整同步策略'
    });
    await waitFor(() => expect(screen.queryByLabelText('操作原因')).not.toBeInTheDocument());

    await user.click(screen.getByRole('tab', { name: '下注资产' }));
    const assetTable = screen.getByRole('grid', { name: '竞猜下注资产配置表' });
    const btcRow = within(assetTable).getByText('BTC').closest('tr');
    expect(btcRow).toBeInTheDocument();
    await user.click(within(btcRow as HTMLTableRowElement).getByRole('switch', { name: 'BTC 允许下注' }));
    await user.clear(within(btcRow as HTMLTableRowElement).getByLabelText('BTC 默认最大赔付'));
    await user.type(within(btcRow as HTMLTableRowElement).getByLabelText('BTC 默认最大赔付'), '2500');
    await user.click(within(btcRow as HTMLTableRowElement).getByRole('button', { name: '保存' }));
    await user.type(await screen.findByLabelText('操作原因'), '  开放 BTC 下注  ');
    await user.click(await screen.findByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/prediction/asset-configs', expect.objectContaining({ method: 'POST' }));
    });
    const assetRequest = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/prediction/asset-configs' && init?.method === 'POST')?.[1];
    expect(JSON.parse(String(assetRequest?.body))).toEqual({
      asset_id: 2,
      enabled: true,
      max_payout_amount: '2500',
      revision: 0,
      reason: '开放 BTC 下注'
    });
  });

  it('preserves each dirty draft after 409 and reloads only after an explicit discard action', async () => {
    const user = userEvent.setup();
    const toastError = vi.spyOn(Toast, 'error');

    renderPredictionPage(<PredictionSettingsPage />);

    await screen.findByLabelText('同步间隔秒数');
    apiRequestMock.mockRejectedValueOnce(new ApiError(409, 'CONFLICT', 'stale revision'));
    await user.clear(screen.getByLabelText('同步间隔秒数'));
    await user.type(screen.getByLabelText('同步间隔秒数'), '600');
    await user.click(screen.getByRole('button', { name: '保存全局策略' }));
    await user.type(await screen.findByLabelText('操作原因'), '并发冲突验证');
    await user.click(await screen.findByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith('全局策略已被其他管理员更新；当前草稿已保留，请重新加载最新配置后再修改。');
    });
    expect(screen.getByLabelText('同步间隔秒数')).toHaveValue(600);
    expect(apiRequestMock.mock.calls.filter(([path, init]) => path === '/admin/api/v1/prediction/settings' && !init?.method)).toHaveLength(1);

    await user.click(screen.getByRole('button', { name: '放弃草稿并重新加载' }));
    await waitFor(() => expect(screen.getByLabelText('同步间隔秒数')).toHaveValue(120));
    expect(apiRequestMock.mock.calls.filter(([path, init]) => path === '/admin/api/v1/prediction/settings' && !init?.method)).toHaveLength(2);

    await user.click(screen.getByRole('tab', { name: '下注资产' }));
    const assetTable = screen.getByRole('grid', { name: '竞猜下注资产配置表' });
    const btcRow = within(assetTable).getByText('BTC').closest('tr') as HTMLTableRowElement;
    await user.click(within(btcRow).getByRole('switch', { name: 'BTC 允许下注' }));
    apiRequestMock.mockRejectedValueOnce(new ApiError(409, 'CONFLICT', 'stale revision'));
    await user.click(within(btcRow).getByRole('button', { name: '保存' }));
    await user.type(await screen.findByLabelText('操作原因'), '资产并发冲突验证');
    await user.click(await screen.findByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith('BTC 配置已被其他管理员更新；当前草稿已保留，请重新加载最新配置后再修改。');
    });
    expect(within(btcRow).getByRole('switch', { name: 'BTC 允许下注' })).toBeChecked();
    expect(apiRequestMock.mock.calls.filter(([path, init]) => path === '/admin/api/v1/prediction/asset-configs' && !init?.method)).toHaveLength(2);

    await user.click(screen.getByRole('button', { name: '放弃草稿并重新加载' }));
    await waitFor(() => expect(screen.getByRole('switch', { name: 'BTC 允许下注' })).not.toBeChecked());
    expect(apiRequestMock.mock.calls.filter(([path, init]) => path === '/admin/api/v1/prediction/asset-configs' && !init?.method)).toHaveLength(3);
    toastError.mockRestore();
  });
});
