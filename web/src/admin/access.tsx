import { Button, Spin, Typography } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { createContext, type ReactNode, useContext, useSyncExternalStore } from 'react';
import { Navigate } from 'react-router-dom';

import { apiRequest, ApiError, ContractError } from '../api/client';
import { authStore } from '../auth/authStore';

const { Text } = Typography;

export type AdminAccess = {
  admin_id: number;
  username: string;
  role_id: number;
  role_name: string;
  permissions: string[];
  is_super_admin: boolean;
};

export type AdminMutationAction = 'operate' | 'review' | 'settle' | 'write';
export type AdminPermissionAction = 'read' | AdminMutationAction;
export type AdminHttpMethod = 'DELETE' | 'GET' | 'HEAD' | 'OPTIONS' | 'PATCH' | 'POST' | 'PUT';

const AdminAccessContext = createContext<AdminAccess | null>(null);

const frontendPathResources: Array<[string, string]> = [
  ['/admin/support', 'support.conversations'],
  ['/admin/config-center', 'config_center'],
  ['/admin/prediction/sync-logs', 'prediction.sync'],
  ['/admin/prediction/sync', 'prediction.sync'],
  ['/admin/prediction/settings', 'prediction.settings'],
  ['/admin/prediction/assets', 'prediction.assets'],
  ['/admin/prediction/markets', 'prediction.markets'],
  ['/admin/prediction/orders', 'prediction.orders'],
  ['/admin/seconds-contract/products', 'seconds.products'],
  ['/admin/seconds-contract/orders', 'seconds.orders'],
  ['/admin/wallet/quick-recharge-orders', 'wallet.quick_recharge'],
  ['/admin/wallet/quick-recharge', 'wallet.quick_recharge'],
  ['/admin/wallet/deposit-network-configs', 'wallet.networks'],
  ['/admin/wallet/deposit-address-pool', 'wallet.address_pool'],
  ['/admin/wallet/withdrawals', 'wallet.withdrawals'],
  ['/admin/wallet/deposits', 'wallet.deposits'],
  ['/admin/wallet/accounts', 'wallet.accounts'],
  ['/admin/wallet/ledger', 'wallet.ledger'],
  ['/admin/loan/products', 'loan.products'],
  ['/admin/loan/orders', 'loan.orders'],
  ['/admin/margin/products', 'margin.products'],
  ['/admin/margin/positions', 'margin.positions'],
  ['/admin/margin/liquidations', 'margin.liquidations'],
  ['/admin/margin/interest', 'margin.interest'],
  ['/admin/earn/categories', 'earn.categories'],
  ['/admin/earn/products', 'earn.products'],
  ['/admin/earn/subscriptions', 'earn.subscriptions'],
  ['/admin/spot/orders', 'spot.orders'],
  ['/admin/spot/trades', 'spot.trades'],
  ['/admin/market/strategies', 'market.strategies'],
  ['/admin/market/feed-config', 'market.feed'],
  ['/admin/market/pairs', 'market.pairs'],
  ['/admin/new-coins/projects', 'new_coin.projects'],
  ['/admin/new-coins/actions', 'new_coin.projects'],
  ['/admin/new-coins/subscriptions', 'new_coin.subscriptions'],
  ['/admin/new-coins/distributions', 'new_coin.distributions'],
  ['/admin/new-coins/purchases', 'new_coin.purchases'],
  ['/admin/new-coins/lock-positions', 'new_coin.locks'],
  ['/admin/new-coins/unlocks', 'new_coin.unlocks'],
  ['/admin/convert/pairs', 'convert.pairs'],
  ['/admin/convert/orders', 'convert.orders'],
  ['/admin/users/kyc', 'users.kyc'],
  ['/admin/users', 'users'],
  ['/admin/agents', 'agents'],
  ['/admin/agent-commissions', 'agents.commissions'],
  ['/admin/agent-commission-rules', 'agents.commission_rules'],
  ['/admin/assets', 'wallet.assets'],
  ['/admin/news', 'content.news'],
  ['/admin/risk/events', 'risk.events'],
  ['/admin/risk', 'risk.rules'],
  ['/admin/system/countries', 'system.countries'],
  ['/admin/system/security-policy', 'system.security'],
  ['/admin/account/security', 'account.security'],
  ['/admin/system/two-factor', 'account.security'],
  ['/admin/system/brand', 'system.brand'],
  ['/admin/system/smtp', 'system.smtp'],
  ['/admin/system/uploads', 'system.uploads'],
  ['/admin/audit-logs', 'audit.logs'],
  ['/admin/dashboard', 'dashboard'],
];

const apiPathResources: Array<[string, string]> = [
  ['/prediction/sync', 'prediction.sync'],
  ['/prediction/settings', 'prediction.settings'],
  ['/prediction/asset-configs', 'prediction.assets'],
  ['/prediction/markets', 'prediction.markets'],
  ['/prediction/orders', 'prediction.orders'],
  ['/access/permissions', 'governance.roles'],
  ['/config-center', 'config_center'],
  ['/config-change-requests', 'governance.changes'],
  ['/seconds-contracts/products', 'seconds.products'],
  ['/seconds-contracts/orders', 'seconds.orders'],
  ['/wallet/withdrawals', 'wallet.withdrawals'],
  ['/wallet/deposits', 'wallet.deposits'],
  ['/wallet/accounts', 'wallet.accounts'],
  ['/wallet/ledger', 'wallet.ledger'],
  ['/wallet/deposit-network-configs', 'wallet.networks'],
  ['/wallet/deposit-address-pool', 'wallet.address_pool'],
  ['/wallet/quick-recharge', 'wallet.quick_recharge'],
  ['/deposit-network-configs', 'wallet.networks'],
  ['/deposit-address-pool', 'wallet.address_pool'],
  ['/quick-recharge', 'wallet.quick_recharge'],
  ['/loan/products', 'loan.products'],
  ['/loan/orders', 'loan.orders'],
  ['/margin/products', 'margin.products'],
  ['/margin/positions', 'margin.positions'],
  ['/margin/liquidations', 'margin.liquidations'],
  ['/margin/interest', 'margin.interest'],
  ['/earn/categories', 'earn.categories'],
  ['/earn/products', 'earn.products'],
  ['/earn/subscriptions', 'earn.subscriptions'],
  ['/spot/orders', 'spot.orders'],
  ['/spot/trades', 'spot.trades'],
  ['/spot/fills', 'spot.orders'],
  ['/support/conversations', 'support.conversations'],
  ['/market/strategies', 'market.strategies'],
  ['/market/feed', 'market.feed'],
  ['/market/pairs', 'market.pairs'],
  ['/market-strategies', 'market.strategies'],
  ['/market-feed', 'market.feed'],
  ['/market-pairs', 'market.pairs'],
  ['/trading-pairs', 'market.pairs'],
  ['/new-coins/projects', 'new_coin.projects'],
  ['/new-coins/subscriptions', 'new_coin.subscriptions'],
  ['/new-coins/distributions', 'new_coin.distributions'],
  ['/new-coins/purchases', 'new_coin.purchases'],
  ['/new-coins/lock-positions', 'new_coin.locks'],
  ['/new-coins/unlocks', 'new_coin.unlocks'],
  ['/new-coins', 'new_coin.projects'],
  ['/convert/pairs', 'convert.pairs'],
  ['/convert/orders', 'convert.orders'],
  ['/users/kyc', 'users.kyc'],
  ['/kyc', 'users.kyc'],
  ['/users', 'users'],
  ['/agents', 'agents'],
  ['/agent-commissions', 'agents.commissions'],
  ['/agent-commission-rules', 'agents.commission_rules'],
  ['/assets', 'wallet.assets'],
  ['/news', 'content.news'],
  ['/risk/events', 'risk.events'],
  ['/risk', 'risk.rules'],
  ['/countries', 'system.countries'],
  ['/security-policy', 'system.security'],
  ['/platform-brand', 'system.brand'],
  ['/platform/brand', 'system.brand'],
  ['/smtp', 'system.smtp'],
  ['/upload/config', 'system.uploads'],
  ['/uploads', 'system.uploads'],
  ['/audit-logs', 'audit.logs'],
  ['/dashboard', 'dashboard'],
  ['/events', 'system.events'],
];

export function hasAdminPermission(access: AdminAccess, permission: string): boolean {
  const permissions = new Set(access.permissions);
  if (access.is_super_admin || permissions.has('*') || permissions.has(permission)) {
    return true;
  }

  return [...permission.matchAll(/\./g)].some((match) => permissions.has(`${permission.slice(0, match.index)}.*`));
}

export function adminReadPermissionForPath(path: string): string {
  const resource = frontendPathResources.find(([prefix]) => path.startsWith(prefix))?.[1] ?? 'admin.unmapped';
  return `${resource}.read`;
}

export function adminPermissionForEndpoint(endpoint: string, action: AdminMutationAction): string {
  const path = endpoint.replace(/^\/admin\/api\/v1/, '');
  const resource = apiPathResources.find(([prefix]) => path.startsWith(prefix))?.[1] ?? 'admin.unmapped';
  return `${resource}.${action}`;
}

/** 与后端 required_admin_permission/operational_action 保持同序的单动作解析。 */
export function adminActionForRequest(endpoint: string, method: AdminHttpMethod): AdminPermissionAction | null {
  const path = endpoint.replace(/^\/admin\/api\/v1/, '').split('?')[0];
  if (path.startsWith('/auth/') || path === '/auth' || path === '/access/me') return null;
  if (method === 'GET' || method === 'HEAD' || method === 'OPTIONS') return 'read';
  if (
    path.includes('/approve') ||
    path.includes('/reject') ||
    path.includes('/review') ||
    path.includes('/confirm') ||
    path.includes('/fail')
  ) {
    return 'review';
  }
  if (path.endsWith('/settle')) return 'settle';
  if (
    path.includes('/reload') ||
    path.includes('/restore') ||
    path.includes('/recovery') ||
    path.endsWith('/sync') ||
    path.includes('/publish') ||
    path.includes('/requeue') ||
    path.endsWith('/apply')
  ) {
    return 'operate';
  }
  return 'write';
}

export function adminPermissionForRequest(endpoint: string, method: AdminHttpMethod): string | null {
  const action = adminActionForRequest(endpoint, method);
  if (!action) return null;
  if (action === 'read') {
    const path = endpoint.replace(/^\/admin\/api\/v1/, '').split('?')[0];
    const resource = apiPathResources.find(([prefix]) => path.startsWith(prefix))?.[1] ?? 'admin.unmapped';
    return `${resource}.read`;
  }
  return adminPermissionForEndpoint(endpoint, action);
}

function parseAdminAccess(value: unknown): AdminAccess {
  const path = '/admin/api/v1/access/me';
  if (!value || typeof value !== 'object') throw new ContractError('权限快照响应必须是对象', { path });
  const access = value as Record<string, unknown>;
  if (
    typeof access.admin_id !== 'number' ||
    !Number.isSafeInteger(access.admin_id) ||
    typeof access.username !== 'string' ||
    typeof access.role_id !== 'number' ||
    !Number.isSafeInteger(access.role_id) ||
    typeof access.role_name !== 'string' ||
    !Array.isArray(access.permissions) ||
    !access.permissions.every((permission) => typeof permission === 'string') ||
    typeof access.is_super_admin !== 'boolean'
  ) {
    throw new ContractError('权限快照响应字段无效', { path });
  }
  return access as unknown as AdminAccess;
}

export async function getAdminAccess(signal?: AbortSignal): Promise<AdminAccess> {
  return parseAdminAccess(await apiRequest<unknown>('/admin/api/v1/access/me', { signal }));
}

export function AdminAccessGate({ children }: { children: ReactNode }) {
  const session = useSyncExternalStore(authStore.subscribe, () => authStore.getSession('admin'));
  const query = useQuery({
    queryKey: ['admin-access', session?.subject ?? 'anonymous', session?.generation ?? 'none'],
    queryFn: ({ signal }) => getAdminAccess(signal),
    enabled: Boolean(session),
    retry: false,
    staleTime: 30_000,
  });

  if (query.isPending) {
    return (
      <main className="admin-access-loading" aria-live="polite">
        <Spin size="large" />
        <Text>正在校验管理权限</Text>
      </main>
    );
  }

  if (query.error || !query.data) {
    if (query.error instanceof ApiError && query.error.status === 401) return <Navigate to="/login" replace />;
    if (query.error instanceof ApiError && query.error.status === 403) return <Navigate to="/403" replace />;
    return (
      <main className="admin-access-loading" aria-live="polite">
        <Text type="danger">权限校验服务暂时不可用，未将网络故障视为无权限。</Text>
        <Button onClick={() => void query.refetch()} theme="solid" type="primary">重试权限校验</Button>
      </main>
    );
  }

  return <AdminAccessContext.Provider value={query.data}>{children}</AdminAccessContext.Provider>;
}

/**
 * 注入已经由后端确认的权限快照。
 * 生产路由通过 AdminAccessGate 获取快照；显式 Provider 主要供组件测试与独立预览使用，
 * 避免各页面自行复制权限判断。
 */
export function AdminAccessProvider({ access, children }: { access: AdminAccess; children: ReactNode }) {
  return <AdminAccessContext.Provider value={access}>{children}</AdminAccessContext.Provider>;
}

export function AdminPermissionBoundary({ children, permission }: { children: ReactNode; permission: string }) {
  const access = useAdminAccess();
  return hasAdminPermission(access, permission) ? <>{children}</> : <Navigate to="/403" replace />;
}

/** 动作级权限门：没有权限时不渲染可交互控件，路由读权限仍由 AdminPermissionBoundary 处理。 */
export function AdminActionBoundary({ children, permission }: { children: ReactNode; permission: string }) {
  const access = useOptionalAdminAccess();
  // 独立组件测试可不注入路由壳；生产路由始终有 AdminAccessGate。
  return access === null || hasAdminPermission(access, permission) ? <>{children}</> : null;
}

/** 按真实 API 路径与 HTTP 方法解析单一动作权限，避免页面自行猜测。 */
export function AdminRequestActionBoundary({
  children,
  endpoint,
  method
}: {
  children: ReactNode;
  endpoint: string;
  method: AdminHttpMethod;
}) {
  const permission = adminPermissionForRequest(endpoint, method);
  return permission ? <AdminActionBoundary permission={permission}>{children}</AdminActionBoundary> : <>{children}</>;
}

export function useCanAdminRequest(endpoint: string, method: AdminHttpMethod): boolean {
  const access = useOptionalAdminAccess();
  const permission = adminPermissionForRequest(endpoint, method);
  return permission === null || access === null || hasAdminPermission(access, permission);
}

export function useAdminAccess(): AdminAccess {
  const access = useContext(AdminAccessContext);
  if (!access) {
    throw new Error('AdminAccessGate is required');
  }
  return access;
}

/**
 * 通用资源页存在不经过完整后台壳渲染的独立单元测试，因此提供可选读取入口。
 * 路由与布局必须使用 useAdminAccess；复用组件缺少上下文时仅保留旧展示行为，
 * 最终写权限仍由后端强制校验。
 */
export function useOptionalAdminAccess(): AdminAccess | null {
  return useContext(AdminAccessContext);
}
