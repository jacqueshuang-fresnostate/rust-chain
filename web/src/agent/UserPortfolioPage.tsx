import { Button, Card, Space, Tabs, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';

import {
  getAgentUserAssets,
  getAgentUserMarginPositions,
  getAgentUserSecondsContractOrders,
  type AgentMarginPositionStatus,
  type AgentSecondsContractOrderStatus,
  type AgentUserAsset,
  type AgentUserMarginPosition,
  type AgentUserSecondsContractOrder
} from '../api/agent';
import { PageHeader } from '../layouts/PageHeader';
import { AdminImageCell } from '../shared/AdminImageUpload';
import { AmountText } from '../shared/AmountText';
import { DataTable } from '../shared/DataTable';
import { AdminSelect } from '../shared/SemiFormControls';
import { StatusTag } from '../shared/StatusTag';
import { TimestampText } from '../shared/TimestampText';

const { Text } = Typography;

type PortfolioTab = 'assets' | 'margin' | 'seconds';
type PageState<T> = { data: T | null; error: Error | null; loading: boolean };

const portfolioTabs = [
  { itemKey: 'assets', tab: '资产' },
  { itemKey: 'margin', tab: '杠杆仓位' },
  { itemKey: 'seconds', tab: '秒合约订单' }
];

const marginStatusOptions = [
  { label: '全部状态', value: '' },
  { label: '持仓中', value: 'opened' },
  { label: '已平仓', value: 'closed' },
  { label: '已取消', value: 'canceled' },
  { label: '已强平', value: 'liquidated' }
];

const secondsStatusOptions = [
  { label: '全部状态（含进行中）', value: '' },
  { label: '进行中', value: 'opened' },
  { label: '已结算', value: 'settled' },
  { label: '人工复核', value: 'manual_review' }
];

function errorValue(error: unknown) {
  return error instanceof Error ? error : new Error('加载失败');
}

/**
 * 按完整查询键缓存成功页。未激活标签不发请求；切回同一页/筛选直接复用缓存。
 * 失败响应不入缓存，后续切换回该键时可重试，且不会清空其他标签的成功数据。
 */
function useCachedPage<T>(enabled: boolean, cacheKey: string, loader: () => Promise<T>): PageState<T> {
  const cache = useRef(new Map<string, T>());
  const [state, setState] = useState<PageState<T>>({ data: null, error: null, loading: false });

  useEffect(() => {
    if (!enabled) return;
    const cached = cache.current.get(cacheKey);
    if (cached !== undefined) {
      setState({ data: cached, error: null, loading: false });
      return;
    }

    let active = true;
    setState({ data: null, error: null, loading: true });
    loader()
      .then((data) => {
        if (!active) return;
        cache.current.set(cacheKey, data);
        setState({ data, error: null, loading: false });
      })
      .catch((error: unknown) => {
        if (active) setState({ data: null, error: errorValue(error), loading: false });
      });
    return () => {
      active = false;
    };
  }, [cacheKey, enabled, loader]);

  return state;
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
  { dataIndex: 'id', key: 'id', title: '仓位ID' },
  { dataIndex: 'symbol', key: 'symbol', title: '交易对' },
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
  { dataIndex: 'status', key: 'status', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '状态' },
  { dataIndex: 'opened_at', key: 'opened_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '开仓时间' },
  { dataIndex: 'closed_at', key: 'closed_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '平仓时间' }
];

const secondsColumns: Array<ColumnProps<AgentUserSecondsContractOrder>> = [
  { dataIndex: 'id', key: 'id', title: '订单ID' },
  { dataIndex: 'symbol', key: 'symbol', title: '交易对' },
  { dataIndex: 'direction', key: 'direction', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '方向' },
  { dataIndex: 'stake_amount', key: 'stake_amount', render: (value, record) => <AmountText asset={record.stake_asset_symbol} value={typeof value === 'string' ? value : null} />, title: '本金' },
  { dataIndex: 'duration_seconds', key: 'duration_seconds', render: (value) => `${String(value)} 秒`, title: '周期' },
  { dataIndex: 'payout_rate', key: 'payout_rate', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '赔率' },
  { dataIndex: 'entry_price', key: 'entry_price', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '开仓价' },
  { dataIndex: 'settlement_price', key: 'settlement_price', render: (value) => <AmountText appendAsset={false} value={typeof value === 'string' ? value : null} />, title: '结算价' },
  {
    dataIndex: 'status',
    key: 'status',
    render: (value) => {
      const status = typeof value === 'string' ? value : null;
      return <StatusTag label={status === 'opened' ? '进行中' : undefined} value={status} />;
    },
    title: '状态'
  },
  { dataIndex: 'result', key: 'result', render: (value) => <StatusTag value={typeof value === 'string' ? value : null} />, title: '输赢' },
  { dataIndex: 'expires_at', key: 'expires_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '到期时间' },
  { dataIndex: 'created_at', key: 'created_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '创建时间' },
  { dataIndex: 'settled_at', key: 'settled_at', render: (value) => <TimestampText value={typeof value === 'number' ? value : null} />, title: '结算时间' }
];

export function AgentUserPortfolioPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { userId: rawUserId = '' } = useParams();
  const userId = Number(rawUserId);
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

  const assets = useCachedPage(activeTab === 'assets' && Number.isSafeInteger(userId) && userId > 0, `assets:${userId}:${assetPageSize}:${assetPage}`, loadAssets);
  const margin = useCachedPage(activeTab === 'margin' && Number.isSafeInteger(userId) && userId > 0, `margin:${userId}:${marginStatus}:${marginPageSize}:${marginPage}`, loadMargin);
  const seconds = useCachedPage(activeTab === 'seconds' && Number.isSafeInteger(userId) && userId > 0, `seconds:${userId}:${secondsStatus}:${secondsPageSize}:${secondsPage}`, loadSeconds);

  if (!Number.isSafeInteger(userId) || userId <= 0) {
    return (
      <main className="exchange-page">
        <PageHeader title="用户资产与订单" />
        <Text type="danger">用户 ID 无效</Text>
      </main>
    );
  }

  return (
    <main className="exchange-page">
      <PageHeader actions={<Button onClick={() => navigate('/agent/users')}>返回团队用户</Button>} title="用户资产与订单" />
      <Card bordered={false} shadows="always" style={{ marginBottom: 16 }}>
        <Space>
          <Text strong>{locationState?.email || `用户 ${userId}`}</Text>
          <Text type="tertiary">用户ID：{userId}</Text>
        </Space>
      </Card>
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
          <Card bordered={false} style={{ marginBottom: 12 }}>
            <div style={{ maxWidth: 280 }}>
              <AdminSelect
                ariaLabel="杠杆仓位状态"
                onChange={(value) => { setMarginPage(1); setMarginStatus(value as AgentMarginPositionStatus | ''); }}
                optionList={marginStatusOptions}
                value={marginStatus}
              />
            </div>
          </Card>
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
          <Card bordered={false} style={{ marginBottom: 12 }}>
            <Space align="center">
              <div style={{ minWidth: 280 }}>
                <AdminSelect
                  ariaLabel="秒合约订单状态"
                  onChange={(value) => { setSecondsPage(1); setSecondsStatus(value as AgentSecondsContractOrderStatus | ''); }}
                  optionList={secondsStatusOptions}
                  value={secondsStatus}
                />
              </div>
              <Text type="tertiary">“全部状态”包含进行中订单。</Text>
            </Space>
          </Card>
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
    </main>
  );
}

function isLocationState(value: unknown): value is { email?: string | null } {
  return value !== null && typeof value === 'object' && !Array.isArray(value) && (!('email' in value) || typeof value.email === 'string' || value.email === null);
}
