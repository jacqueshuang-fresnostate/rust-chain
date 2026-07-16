import { ConfigProvider } from '@douyinfe/semi-ui';
import zhCN from '@douyinfe/semi-ui/lib/es/locale/source/zh_CN';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { type ReactNode, useEffect, useState } from 'react';

type AppProvidersProps = {
  children: ReactNode;
};

const SEMI_THEME_MODE = 'light';

export function AppProviders({ children }: AppProvidersProps) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            retry: 1,
            refetchOnWindowFocus: false
          },
          mutations: {
            retry: 1
          }
        }
      })
  );

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

  return (
    <ConfigProvider locale={zhCN} timeZone="Asia/Shanghai">
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </ConfigProvider>
  );
}
