import { ConfigProvider } from '@douyinfe/semi-ui';
import zhCN from '@douyinfe/semi-ui/lib/es/locale/source/zh_CN';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { type ReactNode, useEffect, useState } from 'react';

import { authStore } from '../auth/authStore';
import { resetMarketTickerConnection } from '../api/marketTickerSocket';

type AppProvidersProps = {
  children: ReactNode;
};

const SEMI_THEME_MODE = 'light';

export function createAppQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: 1,
        refetchOnWindowFocus: false
      },
      mutations: {
        // 写请求默认不重放；只有持有稳定幂等键的业务才可在局部显式开启。
        retry: false
      }
    }
  });
}

export function AppProviders({ children }: AppProvidersProps) {
  const [queryClient] = useState(createAppQueryClient);

  useEffect(() => {
    const previousThemeMode = document.body.getAttribute('theme-mode');
    document.body.setAttribute('theme-mode', SEMI_THEME_MODE);

    return () => {
      if (previousThemeMode) {
        document.body.setAttribute('theme-mode', previousThemeMode);
      } else {
        document.body.removeAttribute('theme-mode');
      }
    };
  }, []);

  useEffect(() => {
    const identitySnapshot = () =>
      (['admin', 'agent', 'user'] as const)
        .map((scope) => {
          const session = authStore.getSession(scope);
          return session ? `${scope}:${session.subject}:${session.generation}` : `${scope}:none`;
        })
        .join('|');
    let previous = identitySnapshot();
    return authStore.subscribe(() => {
      const next = identitySnapshot();
      if (next === previous) return;
      previous = next;
      // 先取消旧代请求，再清掉所有身份相关缓存，避免管理员 A 的 fresh 数据被 B 复用。
      void queryClient.cancelQueries();
      queryClient.clear();
      resetMarketTickerConnection();
    });
  }, [queryClient]);

  return (
    <ConfigProvider locale={zhCN} timeZone="Asia/Shanghai">
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </ConfigProvider>
  );
}
