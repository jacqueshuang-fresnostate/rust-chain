import { Navigate, useLocation } from 'react-router-dom';
import { projectPath } from '../new-coins/projectModel';

/** 保留旧书签入口；项目按 ID 加载，不依赖首批引用列表。 */
export function NewCoinActions() {
  const { search } = useLocation();
  const id = new URLSearchParams(search).get('project_id') ?? '';
  return <Navigate replace to={/^[1-9]\d*$/.test(id) ? projectPath(id) : '/admin/new-coins/projects'} />;
}
