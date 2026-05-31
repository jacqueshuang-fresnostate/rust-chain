import { ConfigProvider } from '@douyinfe/semi-ui';
import zhCN from '@douyinfe/semi-ui/lib/es/locale/source/zh_CN';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { type ReactNode, useState } from 'react';

type AppProvidersProps = {
  children: ReactNode;
};

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

  return (
    <ConfigProvider locale={zhCN} timeZone="Asia/Shanghai">
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </ConfigProvider>
  );
}
