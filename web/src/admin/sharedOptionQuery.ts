import { useQuery } from '@tanstack/react-query';
import { useRef, useSyncExternalStore } from 'react';

import { authStore } from '../auth/authStore';

export type SharedOptionQueryState<T> = {
  data: T;
  error: Error | null;
  loading: boolean;
};

export const ADMIN_OPTION_QUERY_KEY = 'admin-reference-options';

function identityKey(): string {
  const session = authStore.getSession('admin');
  return session ? `${session.subject}:${session.generation}` : 'anonymous:none';
}

/**
 * 管理端目录数据的唯一共享查询入口。TanStack Query 负责同 key 去重、最后一个
 * observer 离开时取消 signal，且 subject/generation 参与 key，不会跨会话复用。
 */
export function useSharedAdminOptionQuery<T>({
  cacheKey,
  empty,
  enabled,
  load,
  staleTime = 5 * 60_000
}: {
  cacheKey: string;
  empty: T;
  enabled: boolean;
  load: (signal: AbortSignal) => Promise<T>;
  staleTime?: number;
}): SharedOptionQueryState<T> {
  const identity = useSyncExternalStore(authStore.subscribe, identityKey, identityKey);
  const loadRef = useRef(load);
  loadRef.current = load;
  const query = useQuery({
    queryKey: [ADMIN_OPTION_QUERY_KEY, identity, cacheKey],
    queryFn: ({ signal }) => loadRef.current(signal),
    enabled,
    staleTime
  });

  return {
    data: query.data ?? empty,
    error: query.error instanceof Error ? query.error : query.error ? new Error('选项数据加载失败') : null,
    loading: enabled && query.isPending
  };
}
