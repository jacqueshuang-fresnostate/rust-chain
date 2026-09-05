import { Tabs, TabPane } from '@douyinfe/semi-ui';
import { useLocation, useNavigate } from 'react-router-dom';
import { ResourcePage, resourceConfigs, type ResourceConfig } from '../resources/resourceConfigs';
import { useCanAdminRequest } from '../access';

/** 仅新币工作台接收白名单查询筛选；旧深链与项目中心链接复用相同分页接口。 */
export function NewCoinResourcePage({ config }: { config: ResourceConfig }) {
  const { search } = useLocation();
  const params = new URLSearchParams(search);
  const initialFilters = Object.fromEntries((config.filters ?? []).filter(f=>params.has(f.key) && !['limit','offset'].includes(f.key)).map(f=>[f.key,params.get(f.key) ?? '']));
  return <ResourcePage key={`${config.endpoint}:${search}`} config={config} initialFilters={initialFilters} />;
}

/** 两个旧路由继续按各自资源读权限守卫；未授权标签不渲染，也不提前加载数据。 */
export function NewCoinLocksPage() {
  const { pathname, search }=useLocation();
  const navigate=useNavigate();
  const canLocks=useCanAdminRequest('/admin/api/v1/new-coins/lock-positions','GET');
  const canUnlocks=useCanAdminRequest('/admin/api/v1/new-coins/unlocks','GET');
  const active=pathname.endsWith('/unlocks')?'unlocks':'lock-positions';
  return <Tabs activeKey={active} keepDOM={false} tabPaneMotion={false} onChange={key=>navigate(`/admin/new-coins/${key}${search}`)}>
    {canLocks ? <TabPane itemKey="lock-positions" tab="锁仓仓位"><NewCoinResourcePage config={resourceConfigs.newCoinLockPositions}/></TabPane> : null}
    {canUnlocks ? <TabPane itemKey="unlocks" tab="解禁记录"><NewCoinResourcePage config={resourceConfigs.newCoinUnlocks}/></TabPane> : null}
  </Tabs>;
}
