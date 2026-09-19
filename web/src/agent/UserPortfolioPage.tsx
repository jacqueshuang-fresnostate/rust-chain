import { IconRefresh } from '@douyinfe/semi-icons';
import { Button, Space, Tabs, Tooltip, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import { parseSafeInteger } from '../shared/integer';

import {
  getAgentUserAssets,
  getAgentUserMarginPositions,
  getAgentUserMarginOrders,
  getAgentUserSpotOrders,
  getAgentUserSecondsContractOrders,
  type AgentMarginPositionStatus,
  type AgentMarginOrderStatus,
  type AgentSpotOrderStatus,
  type AgentUserSpotOrder,
  type AgentSecondsContractOrderStatus,
  type AgentUserAsset,
  type AgentUserMarginPosition,
  type AgentUserSecondsContractOrder
} from '../api/agent';
import { authStore } from '../auth/authStore';
import { PageHeader } from '../layouts/PageHeader';
import { AdminImageCell } from '../shared/AdminImageUpload';
import { AmountText } from '../shared/AmountText';
import { DataTable } from '../shared/DataTable';
import { AdminSelect } from '../shared/SemiFormControls';
import { StatusTag } from '../shared/StatusTag';
import { TimestampText } from '../shared/TimestampText';
import './UserPortfolioPage.css';

const { Text } = Typography;

type PortfolioTab = 'assets' | 'margin' | 'seconds' | 'margin-orders' | 'spot';
type PageState<T> = { data: T | null; error: Error | null; loading: boolean };

const portfolioTabs = [
  { itemKey: 'assets', tab: '钱包账户' },
  { itemKey: 'seconds', tab: '秒合约订单' },
  { itemKey: 'margin-orders', tab: '杠杆订单' },
  { itemKey: 'margin', tab: '杠杆持仓' },
  { itemKey: 'spot', tab: '现货订单' }
];

const marginStatusOptions = [
  { label: '全部状态', value: '' },
  { label: '持仓中', value: 'opened' },
  { label: '已平仓', value: 'closed' },
  { label: '已强平', value: 'liquidated' }
];

const marginOrderStatusOptions = [
  { label: '全部状态', value: '' },
  { label: '待成交', value: 'pending' },
  { label: '持仓中', value: 'opened' },
  { label: '已平仓', value: 'closed' },
  { label: '已取消', value: 'canceled' },
  { label: '已强平', value: 'liquidated' }
];

const spotStatusOptions = [
  { label: '全部状态', value: '' },
  { label: '待处理', value: 'pending' },
  { label: '委托中', value: 'open' },
  { label: '部分成交', value: 'partially_filled' },
  { label: '已成交', value: 'filled' },
  { label: '已取消', value: 'cancelled' },
  { label: '已拒绝', value: 'rejected' }
];

const secondsStatusOptions = [
  { label: '全部状态（含进行中）', value: '' },
  { label: '进行中', value: 'opened' },
  { label: '已结算', value: 'settled' },
  { label: '人工复核', value: 'manual_review' },
  { label: '已退还本金', value: 'refunded' }
];

function errorValue(error: unknown) {
  return error instanceof Error ? error : new Error('加载失败');
}

/**
 * 按完整查询键缓存成功页。未激活标签不发请求；切回同一页/筛选直接复用缓存。
 * 失败响应不入缓存，后续切换回该键时可重试，且不会清空其他标签的成功数据。
 */
function useCachedPage<T>(enabled: boolean, cacheKey: string, loader: () => Promise<T>) {
  const cache = useRef(new Map<string, T>());
  const [revision, setRevision] = useState(0);
  const requestKey = `${cacheKey}:${revision}`;
  const [state, setState] = useState<PageState<T> & { key: string }>({ key: '', data: null, error: null, loading: false });
  const refresh = () => {
    cache.current.clear();
    setRevision((value) => value + 1);
  };

  useEffect(() => {
    if (!enabled) return;
    const cached = cache.current.get(cacheKey);
    if (cached !== undefined) {
      setState({ key: requestKey, data: cached, error: null, loading: false });
      return;
    }

    let active = true;
    setState({ key: requestKey, data: null, error: null, loading: true });
    loader()
      .then((data) => {
        if (!active) return;
        cache.current.set(cacheKey, data);
        setState({ key: requestKey, data, error: null, loading: false });
      })
      .catch((error: unknown) => {
        if (active) setState({ key: requestKey, data: null, error: errorValue(error), loading: false });
      });
    return () => {
      active = false;
    };
  }, [cacheKey, enabled, loader, requestKey]);

  return { ...(state.key === requestKey ? state : { data: null, error: null, loading: enabled }), refresh };
}

const assetColumns: Array<ColumnProps<AgentUserAsset>> = [
  { dataIndex: 'account_type', key: 'account_type', render: (value) => (value === 'margin' ? '杠杆账户' : '现货账户'), title: '账户类型' },
  { dataIndex: 'logo_url', key: 'logo_url', render: (value, record) => <AdminImageCell alt={`${record.asset_symbol} Logo`} value={value} />, title: 'Logo' },
  { dataIndex: 'asset_symbol', key: 'asset_symbol', title: '资产' },
  { dataIndex: 'available', key: 'available', render: (value, record) => <AmountText asset={record.asset_symbol} precision={record.precision_scale} value={typeof value === 'string' ? value : null} />, title: '可用' },
  { dataIndex: 'frozen', key: 'frozen', render: (value, record) => <AmountText asset={record.asset_symbol} precision={record.precision_scale} value={typeof value === 'string' ? value : null} />, title: '冻结' },
  { dataIndex: 'locked', key: 'locked', render: (value, record) => <AmountText asset={record.asset_symbol} precision={record.precision_scale} value={typeof value === 'string' ? value : null} />, title: '锁定' },
  { dataIndex: 'updated_at', key: 'updated_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '更新时间' }
];

const marginColumns: Array<ColumnProps<AgentUserMarginPosition>> = [
  { dataIndex: 'id', key: 'id', title: '仓位ID', width: 100 },
  { dataIndex: 'symbol', key: 'symbol', title: '交易对', width: 140 },
  { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态', width: 120 },
  { dataIndex: 'direction', key: 'direction', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '方向' },
  { dataIndex: 'margin_mode', key: 'margin_mode', render: (value) => (value === 'cross' ? '全仓' : '逐仓'), title: '保证金模式' },
  { dataIndex: 'wallet_scope', key: 'wallet_scope', render: (value) => (value === 'margin' ? '杠杆账户' : '现货账户'), title: '资金账户' },
  { dataIndex: 'leverage', key: 'leverage', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '杠杆' },
  { dataIndex: 'margin_amount', key: 'margin_amount', render: (value, record) => <AmountText asset={record.margin_asset_symbol} value={typeof value === 'string' ? value : null} />, title: '保证金' },
  { dataIndex: 'notional_amount', key: 'notional_amount', render: (value, record) => <AmountText asset={record.margin_asset_symbol} value={typeof value === 'string' ? value : null} />, title: '名义金额' },
  { dataIndex: 'borrowed_amount', key: 'borrowed_amount', render: (value, record) => <AmountText asset={record.margin_asset_symbol} value={typeof value === 'string' ? value : null} />, title: '借款本金' },
  { dataIndex: 'interest_amount', key: 'interest_amount', render: (value, record) => <AmountText asset={record.margin_asset_symbol} value={typeof value === 'string' ? value : null} />, title: '利息' },
  { dataIndex: 'entry_price', key: 'entry_price', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '开仓价' },
  { dataIndex: 'limit_price', key: 'limit_price', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '限价' },
  { dataIndex: 'exit_price', key: 'exit_price', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '平仓价' },
  { dataIndex: 'realized_pnl', key: 'realized_pnl', render: (value, record) => <AmountText asset={record.margin_asset_symbol} value={typeof value === 'string' ? value : null} />, title: '已实现PnL' },
  { dataIndex: 'opened_at', key: 'opened_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '开仓时间' },
  { dataIndex: 'closed_at', key: 'closed_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '平仓时间' }
];

const marginOrderColumns: Array<ColumnProps<AgentUserMarginPosition>> = [
  { dataIndex: 'id', key: 'id', title: '订单ID', width: 100 },
  { dataIndex: 'symbol', key: 'symbol', title: '交易对', width: 140 },
  {
    key: 'status', title: '状态', width: 120,
    render: (_, record) => <StatusTag label={record.status === 'opened' && record.entry_price === null ? '待成交' : undefined} value={record.status === 'opened' && record.entry_price === null ? 'pending' : record.status} />
  },
  { dataIndex: 'order_type', key: 'order_type', title: '委托类型', width: 120, render: (value) => value === 'limit' ? '限价' : '市价' },
  ...marginColumns.filter((column) => ['direction', 'margin_mode', 'wallet_scope', 'leverage', 'margin_amount', 'limit_price', 'entry_price'].includes(String(column.key))),
  { dataIndex: 'created_at', key: 'created_at', title: '委托时间', render: (value) => <TimestampText value={value} /> },
  { dataIndex: 'closed_at', key: 'closed_at', title: '结束时间', render: (value) => <TimestampText value={value} /> }
];

const spotColumns: Array<ColumnProps<AgentUserSpotOrder>> = [
  { dataIndex: 'id', key: 'id', title: '订单ID', width: 100 },
  { dataIndex: 'symbol', key: 'symbol', title: '交易对', width: 140 },
  { dataIndex: 'status', key: 'status', title: '状态', width: 120, render: (value) => <StatusTag label={spotStatusOptions.find((option) => option.value === value)?.label} value={value} /> },
  { dataIndex: 'side', key: 'side', title: '方向', width: 100, render: (value) => <StatusTag label={value === 'buy' ? '买入' : '卖出'} value={value} /> },
  { dataIndex: 'order_type', key: 'order_type', title: '委托类型', render: (value) => ({ market: '市价', limit: '限价', stop_limit: '止盈止损限价' })[value as AgentUserSpotOrder['order_type']] },
  { dataIndex: 'price', key: 'price', title: '委托价', render: (value) => <AmountText appendAsset={false} value={value} /> },
  { dataIndex: 'trigger_price', key: 'trigger_price', title: '触发价', render: (value) => <AmountText appendAsset={false} value={value} /> },
  { dataIndex: 'quantity', key: 'quantity', title: '委托数量', render: (value) => <AmountText appendAsset={false} value={value} /> },
  { dataIndex: 'filled_quantity', key: 'filled_quantity', title: '已成交数量', render: (value) => <AmountText appendAsset={false} value={value} /> },
  { dataIndex: 'created_at', key: 'created_at', title: '委托时间', render: (value) => <TimestampText value={value} /> },
  { dataIndex: 'updated_at', key: 'updated_at', title: '更新时间', render: (value) => <TimestampText value={value} /> }
];

function SecondsOrderCountdown({ expiresAt, orderId }: { expiresAt: number; orderId: number }) {
  const [remaining, setRemaining] = useState(() => Math.max(0, Math.ceil((expiresAt - Date.now()) / 1000)));

  useEffect(() => {
    let interval: number | undefined;
    // 始终按截止时间重算，后台节流或休眠后不累积逐秒递减误差。
    const sync = () => {
      const next = Math.max(0, Math.ceil((expiresAt - Date.now()) / 1000));
      setRemaining(next);
      if (next === 0 && interval !== undefined) {
        window.clearInterval(interval);
        interval = undefined;
      }
    };
    const onVisible = () => {
      if (document.visibilityState === 'visible') sync();
    };
    sync();
    if (expiresAt > Date.now()) interval = window.setInterval(sync, 1000);
    window.addEventListener('focus', sync);
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener('focus', sync);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, [expiresAt]);

  const hours = Math.floor(remaining / 3600);
  const minutes = String(Math.floor((remaining % 3600) / 60)).padStart(2, '0');
  const seconds = String(remaining % 60).padStart(2, '0');
  const countdown = `${hours > 0 ? `${String(hours).padStart(2, '0')}:` : ''}${minutes}:${seconds}`;
  return (
    <span aria-label={`订单 ${orderId} 倒计时`} className="agent-order-countdown" role="timer">
      {remaining > 0 ? countdown : '待结算'}
    </span>
  );
}

const secondsColumns: Array<ColumnProps<AgentUserSecondsContractOrder>> = [
  { dataIndex: 'id', key: 'id', title: '订单ID', width: 100 },
  { dataIndex: 'symbol', key: 'symbol', title: '交易对', width: 140 },
  {
    dataIndex: 'status', key: 'status', title: '状态', width: 120,
    render: (value) => {
      const status = typeof value === 'string' ? value : null;
      return <StatusTag label={status === 'opened' ? '进行中' : undefined} value={status} />;
    }
  },
  {
    key: 'countdown', title: '倒计时', width: 140,
    render: (_, record) => record.status === 'opened'
      ? <SecondsOrderCountdown key={`${record.id}:${record.expires_at}`} expiresAt={record.expires_at} orderId={record.id} />
      : <span>-</span>
  },
  { dataIndex: 'direction', key: 'direction', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '方向' },
  { dataIndex: 'stake_amount', key: 'stake_amount', render: (value, record) => <AmountText asset={record.stake_asset_symbol} value={typeof value === 'string' ? value : null} />, title: '本金' },
  { dataIndex: 'duration_seconds', key: 'duration_seconds', render: (value) => `${String(value)} 秒`, title: '周期' },
  { dataIndex: 'payout_rate', key: 'payout_rate', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '赔率' },
  { dataIndex: 'entry_price', key: 'entry_price', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '开仓价' },
  { dataIndex: 'settlement_price', key: 'settlement_price', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '结算价' },
  { dataIndex: 'result', key: 'result', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '输赢' },
  { dataIndex: 'expires_at', key: 'expires_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '到期时间' },
  { dataIndex: 'created_at', key: 'created_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '创建时间' },
  { dataIndex: 'settled_at', key: 'settled_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '结算时间' }
];

export function AgentUserPortfolioPage() {
  const { userId } = useParams();
  const generation = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('agent')?.generation ?? '');
  return <UserPortfolio key={`${generation}:${userId}`} />;
}

function UserPortfolio() {
  const navigate = useNavigate();
  const location = useLocation();
  const { userId: rawUserId = '' } = useParams();
  const userId = parseSafeInteger(rawUserId, 1) ?? Number.NaN;
  const locationState = isLocationState(location.state) ? location.state : null;
  const [activeTab, setActiveTab] = useState<PortfolioTab>('assets');
  const [assetPage, setAssetPage] = useState(1);
  const [assetPageSize, setAssetPageSize] = useState(20);
  const [marginPage, setMarginPage] = useState(1);
  const [marginPageSize, setMarginPageSize] = useState(20);
  const [marginStatus, setMarginStatus] = useState<AgentMarginPositionStatus | ''>('');
  const [secondsPage, setSecondsPage] = useState(1);
  const [secondsPageSize, setSecondsPageSize] = useState(20);
  const [secondsStatus, setSecondsStatus] = useState<AgentSecondsContractOrderStatus | ''>('');
  const [marginOrderPage, setMarginOrderPage] = useState(1);
  const [marginOrderPageSize, setMarginOrderPageSize] = useState(20);
  const [marginOrderStatus, setMarginOrderStatus] = useState<AgentMarginOrderStatus | ''>('');
  const [spotPage, setSpotPage] = useState(1);
  const [spotPageSize, setSpotPageSize] = useState(20);
  const [spotStatus, setSpotStatus] = useState<AgentSpotOrderStatus | ''>('');

  const loadAssets = useCallback(
    () => getAgentUserAssets(userId, { limit: assetPageSize, offset: (assetPage - 1) * assetPageSize }),
    [assetPage, assetPageSize, userId]
  );
  const loadMargin = useCallback(
    () => getAgentUserMarginPositions(userId, { limit: marginPageSize, offset: (marginPage - 1) * marginPageSize, status: marginStatus || undefined }),
    [marginPage, marginPageSize, marginStatus, userId]
  );
  const loadSeconds = useCallback(
    () => getAgentUserSecondsContractOrders(userId, { limit: secondsPageSize, offset: (secondsPage - 1) * secondsPageSize, status: secondsStatus || undefined }),
    [secondsPage, secondsPageSize, secondsStatus, userId]
  );
  const loadMarginOrders = useCallback(
    () => getAgentUserMarginOrders(userId, { limit: marginOrderPageSize, offset: (marginOrderPage - 1) * marginOrderPageSize, status: marginOrderStatus || undefined }),
    [marginOrderPage, marginOrderPageSize, marginOrderStatus, userId]
  );
  const loadSpot = useCallback(
    () => getAgentUserSpotOrders(userId, { limit: spotPageSize, offset: (spotPage - 1) * spotPageSize, status: spotStatus || undefined }),
    [spotPage, spotPageSize, spotStatus, userId]
  );

  const assets = useCachedPage(activeTab === 'assets' && Number.isSafeInteger(userId) && userId > 0, `assets:${userId}:${assetPageSize}:${assetPage}`, loadAssets);
  const margin = useCachedPage(activeTab === 'margin' && Number.isSafeInteger(userId) && userId > 0, `margin:${userId}:${marginStatus}:${marginPageSize}:${marginPage}`, loadMargin);
  const seconds = useCachedPage(activeTab === 'seconds' && Number.isSafeInteger(userId) && userId > 0, `seconds:${userId}:${secondsStatus}:${secondsPageSize}:${secondsPage}`, loadSeconds);
  const marginOrders = useCachedPage(activeTab === 'margin-orders' && Number.isSafeInteger(userId) && userId > 0, `margin-orders:${userId}:${marginOrderStatus}:${marginOrderPageSize}:${marginOrderPage}`, loadMarginOrders);
  const spot = useCachedPage(activeTab === 'spot' && Number.isSafeInteger(userId) && userId > 0, `spot:${userId}:${spotStatus}:${spotPageSize}:${spotPage}`, loadSpot);
  const current = { assets, margin, seconds, 'margin-orders': marginOrders, spot }[activeTab];

  if (!Number.isSafeInteger(userId) || userId <= 0) {
    return (
      <main className="exchange-page agent-user-portfolio">
        <PageHeader title="用户资产与订单" />
        <Text type="danger">用户 ID 无效</Text>
      </main>
    );
  }

  return (
    <main className="exchange-page agent-user-portfolio">
      <PageHeader actions={<Space>
        <Tooltip content="刷新当前列表"><Button aria-label="刷新当前列表" disabled={current.loading} icon={<IconRefresh />} onClick={current.refresh} /></Tooltip>
        <Button onClick={() => navigate('/agent/users')}>返回团队用户</Button>
      </Space>} title="用户资产与订单" />
      <div style={{ marginBottom: 16 }}>
        <Space wrap>
          <Text strong>{locationState?.email || `用户 ${userId}`}</Text>
          <Text type="tertiary">用户ID：{userId}</Text>
        </Space>
      </div>
      <Tabs activeKey={activeTab} className="admin-action-tabs" onChange={(key) => setActiveTab(key as PortfolioTab)} tabList={portfolioTabs} type="button" />

      {activeTab === 'assets' ? (
        <div aria-labelledby="semiTabassets" id="semiTabPanelassets" role="tabpanel" tabIndex={0}>
          <DataTable
            columns={assetColumns}
            data={assets.data?.assets ?? []}
            error={assets.error}
            loading={assets.loading}
            pagination={{
              currentPage: assetPage,
              onPageChange: setAssetPage,
              onPageSizeChange: (next) => { setAssetPage(1); setAssetPageSize(next); },
              pageSize: assetPageSize,
              total: assets.data?.total ?? 0
            }}
            rowKey={(record) => `${record.account_type}-${record.account_id}`}
          />
        </div>
      ) : null}

      {activeTab === 'margin' ? (
        <div aria-labelledby="semiTabmargin" id="semiTabPanelmargin" role="tabpanel" tabIndex={0}>
          <div style={{ marginBottom: 12 }}>
            <div style={{ maxWidth: 280 }}>
              <AdminSelect
                ariaLabel="杠杆持仓状态"
                onChange={(value) => { setMarginPage(1); setMarginStatus(value as AgentMarginPositionStatus | ''); }}
                optionList={marginStatusOptions}
                value={marginStatus}
              />
            </div>
          </div>
          <DataTable
            columns={marginColumns}
            data={margin.data?.positions ?? []}
            error={margin.error}
            loading={margin.loading}
            pagination={{
              currentPage: marginPage,
              onPageChange: setMarginPage,
              onPageSizeChange: (next) => { setMarginPage(1); setMarginPageSize(next); },
              pageSize: marginPageSize,
              total: margin.data?.total ?? 0
            }}
            rowKey="id"
          />
        </div>
      ) : null}

      {activeTab === 'seconds' ? (
        <div aria-labelledby="semiTabseconds" id="semiTabPanelseconds" role="tabpanel" tabIndex={0}>
          <div style={{ marginBottom: 12 }}>
            <Space align="center">
              <div style={{ width: 280, maxWidth: '100%' }}>
                <AdminSelect
                  ariaLabel="秒合约订单状态"
                  onChange={(value) => { setSecondsPage(1); setSecondsStatus(value as AgentSecondsContractOrderStatus | ''); }}
                  optionList={secondsStatusOptions}
                  value={secondsStatus}
                />
              </div>
            </Space>
          </div>
          <DataTable
            columns={secondsColumns}
            data={seconds.data?.orders ?? []}
            error={seconds.error}
            loading={seconds.loading}
            pagination={{
              currentPage: secondsPage,
              onPageChange: setSecondsPage,
              onPageSizeChange: (next) => { setSecondsPage(1); setSecondsPageSize(next); },
              pageSize: secondsPageSize,
              total: seconds.data?.total ?? 0
            }}
            rowKey="id"
          />
        </div>
      ) : null}

      {activeTab === 'margin-orders' ? (
        <div aria-labelledby="semiTabmargin-orders" id="semiTabPanelmargin-orders" role="tabpanel" tabIndex={0}>
          <div style={{ maxWidth: 280, marginBottom: 12 }}>
            <AdminSelect ariaLabel="杠杆订单状态" value={marginOrderStatus} optionList={marginOrderStatusOptions}
              onChange={(value) => { setMarginOrderPage(1); setMarginOrderStatus(value as AgentMarginOrderStatus | ''); }} />
          </div>
          <DataTable columns={marginOrderColumns} data={marginOrders.data?.orders ?? []} error={marginOrders.error} loading={marginOrders.loading}
            pagination={{ currentPage: marginOrderPage, pageSize: marginOrderPageSize, total: marginOrders.data?.total ?? 0,
              onPageChange: setMarginOrderPage, onPageSizeChange: (next) => { setMarginOrderPage(1); setMarginOrderPageSize(next); } }} rowKey="id" />
        </div>
      ) : null}

      {activeTab === 'spot' ? (
        <div aria-labelledby="semiTabspot" id="semiTabPanelspot" role="tabpanel" tabIndex={0}>
          <div style={{ maxWidth: 280, marginBottom: 12 }}>
            <AdminSelect ariaLabel="现货订单状态" value={spotStatus} optionList={spotStatusOptions}
              onChange={(value) => { setSpotPage(1); setSpotStatus(value as AgentSpotOrderStatus | ''); }} />
          </div>
          <DataTable columns={spotColumns} data={spot.data?.orders ?? []} error={spot.error} loading={spot.loading}
            pagination={{ currentPage: spotPage, pageSize: spotPageSize, total: spot.data?.total ?? 0,
              onPageChange: setSpotPage, onPageSizeChange: (next) => { setSpotPage(1); setSpotPageSize(next); } }} rowKey="id" />
        </div>
      ) : null}
    </main>
  );
}

function isLocationState(value: unknown): value is { email?: string | null } {
  return value !== null && typeof value === 'object' && !Array.isArray(value) && (!('email' in value) || typeof value.email === 'string' || value.email === null);
}
