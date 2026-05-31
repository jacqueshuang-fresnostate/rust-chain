# Admin Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Admin-first React + Semi Design backend UI in `web/`, with Admin login, guarded `/admin/*` routes, Admin navigation, real API-first pages, and Agent UI deferred.

**Architecture:** Create a Vite React TypeScript app under `web/`. Keep the UI modular: `api/` owns HTTP and endpoint wrappers, `auth/` owns session state and guards, `layouts/` owns shell UI, `shared/` owns reusable table/status/time/action components, and `admin/` owns feature pages. Most Admin list pages use shared page primitives so the first implementation covers the full menu without duplicating table/error/loading code.

**Tech Stack:** Vite, React, TypeScript, Semi Design, React Router, TanStack React Query, Vitest, Testing Library, ESLint.

---

## File Structure

Create these files under `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web`.

```text
web/
  package.json
  index.html
  tsconfig.json
  tsconfig.node.json
  vite.config.ts
  vitest.setup.ts
  eslint.config.js
  src/
    main.tsx
    styles.css
    app/
      providers.tsx
      router.tsx
    auth/
      LoginPage.tsx
      RequireAdmin.tsx
      authStore.ts
      authStore.test.ts
      RequireAdmin.test.tsx
    api/
      client.ts
      client.test.ts
      types.ts
      adminAuth.ts
      adminResources.ts
    layouts/
      AdminLayout.tsx
      AdminLayout.test.tsx
      PageHeader.tsx
    shared/
      AmountText.tsx
      ConfirmAction.tsx
      DataTable.tsx
      FilterBar.tsx
      JsonDrawer.tsx
      StatusTag.tsx
      TimestampText.tsx
      format.test.tsx
    admin/
      dashboard/DashboardPage.tsx
      resources/resourceConfigs.tsx
      resources/AdminResourcePage.tsx
      resources/AdminResourcePage.test.tsx
      resources/AdminNoticePage.tsx
      actions/AgentManagementPage.tsx
      actions/NewCoinActions.tsx
      actions/MarketStrategyActions.tsx
      actions/ConvertRuleActions.tsx
      actions/ProductStatusActions.tsx
      routes.tsx
    pages/
      ForbiddenPage.tsx
      NotFoundPage.tsx
```

Do not create Agent pages in this plan. Keep only `scope`-aware auth state and disabled Agent selector text on the login page for future extension.

---

### Task 1: Scaffold the Vite React app

**Files:**
- Create: `web/package.json`
- Create: `web/index.html`
- Create: `web/tsconfig.json`
- Create: `web/tsconfig.node.json`
- Create: `web/vite.config.ts`
- Create: `web/vitest.setup.ts`
- Create: `web/eslint.config.js`
- Create: `web/src/main.tsx`
- Create: `web/src/styles.css`
- Create: `web/src/app/providers.tsx`

- [ ] **Step 1: Create package and tool config files**

Create `web/package.json`:

```json
{
  "name": "exchange-admin-web",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1 --port 5173",
    "build": "tsc -b && vite build",
    "typecheck": "tsc -b --pretty false",
    "lint": "eslint .",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "@douyinfe/semi-icons": "^2.75.0",
    "@douyinfe/semi-ui": "^2.75.0",
    "@tanstack/react-query": "^5.80.0",
    "react": "^19.1.0",
    "react-dom": "^19.1.0",
    "react-router-dom": "^7.6.0"
  },
  "devDependencies": {
    "@eslint/js": "^9.27.0",
    "@testing-library/jest-dom": "^6.6.3",
    "@testing-library/react": "^16.3.0",
    "@testing-library/user-event": "^14.6.1",
    "@types/node": "^22.15.0",
    "@types/react": "^19.1.0",
    "@types/react-dom": "^19.1.0",
    "@vitejs/plugin-react": "^4.5.0",
    "eslint": "^9.27.0",
    "globals": "^16.1.0",
    "jsdom": "^26.1.0",
    "typescript": "^5.8.3",
    "typescript-eslint": "^8.33.0",
    "vite": "^6.3.5",
    "vitest": "^3.1.4"
  }
}
```

Create `web/index.html`:

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>交易所管理后台</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

Create `web/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2022"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "types": ["vitest/globals", "@testing-library/jest-dom"]
  },
  "include": ["src", "vitest.setup.ts"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

Create `web/tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true
  },
  "include": ["vite.config.ts", "eslint.config.js"]
}
```

Create `web/vite.config.ts`:

```ts
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './vitest.setup.ts'
  }
});
```

Create `web/vitest.setup.ts`:

```ts
import '@testing-library/jest-dom/vitest';
```

Create `web/eslint.config.js`:

```js
import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      globals: {
        ...globals.browser,
        ...globals.es2022
      },
      parserOptions: {
        project: ['./tsconfig.json', './tsconfig.node.json'],
        tsconfigRootDir: import.meta.dirname
      }
    },
    rules: {
      '@typescript-eslint/no-explicit-any': 'error'
    }
  },
  {
    ignores: ['dist']
  }
);
```

- [ ] **Step 2: Create minimal React entry and providers**

Create `web/src/app/providers.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false
    }
  }
});

export function AppProviders({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}
```

Create `web/src/main.tsx`:

```tsx
import '@douyinfe/semi-ui/dist/css/semi.min.css';
import './styles.css';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider } from 'react-router-dom';
import { AppProviders } from './app/providers';
import { router } from './app/router';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <AppProviders>
      <RouterProvider router={router} />
    </AppProviders>
  </React.StrictMode>
);
```

Create `web/src/styles.css`:

```css
:root {
  color: #1f2933;
  background: #f5f7fb;
  font-family: Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
}

body {
  margin: 0;
  min-width: 1280px;
  background: #f5f7fb;
}

.exchange-page {
  padding: 20px;
}

.exchange-card-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
}
```

Create a temporary `web/src/app/router.tsx` so the scaffold compiles. It will be replaced in Task 4:

```tsx
import { createBrowserRouter, Navigate } from 'react-router-dom';

export const router = createBrowserRouter([
  { path: '/', element: <Navigate to="/login" replace /> },
  { path: '/login', element: <div>交易所管理后台</div> }
]);
```

- [ ] **Step 3: Install dependencies**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm install
```

Expected: dependencies install and `web/package-lock.json` is created.

- [ ] **Step 4: Verify scaffold builds**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run typecheck && npm run lint && npm run build
```

Expected: all commands exit 0.

- [ ] **Step 5: Record progress instead of committing**

This project is not a git repository. Do not run `git commit`. Append a `docs/superpowers/PROGRESS.md` entry after the frontend scaffold passes verification.

---

### Task 2: Shared formatting and display components

**Files:**
- Create: `web/src/shared/TimestampText.tsx`
- Create: `web/src/shared/AmountText.tsx`
- Create: `web/src/shared/StatusTag.tsx`
- Create: `web/src/shared/format.test.tsx`

- [ ] **Step 1: Write failing tests for timestamp, amount, and status rendering**

Create `web/src/shared/format.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { AmountText } from './AmountText';
import { StatusTag } from './StatusTag';
import { TimestampText } from './TimestampText';

describe('shared display components', () => {
  it('formats Unix milliseconds as Chinese local date text', () => {
    render(<TimestampText value={1783740153000} />);
    expect(screen.getByText(/2026/)).toBeInTheDocument();
    expect(screen.getByText(/07/)).toBeInTheDocument();
  });

  it('keeps decimal string values without floating point math', () => {
    render(<AmountText value="123456789.123456789123456789" asset="USDT" />);
    expect(screen.getByText('123456789.123456789123456789 USDT')).toBeInTheDocument();
  });

  it('maps risk-like statuses to danger tags', () => {
    render(<StatusTag value="failed" />);
    expect(screen.getByText('失败')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail before implementation**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/shared/format.test.tsx
```

Expected: FAIL because `AmountText`, `StatusTag`, and `TimestampText` do not exist.

- [ ] **Step 3: Implement minimal display components**

Create `web/src/shared/TimestampText.tsx`:

```tsx
export function TimestampText({ value }: { value?: number | null }) {
  if (value === null || value === undefined) {
    return <span>-</span>;
  }
  const text = new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false
  }).format(new Date(value));
  return <span>{text}</span>;
}
```

Create `web/src/shared/AmountText.tsx`:

```tsx
export function AmountText({ value, asset }: { value?: string | null; asset?: string | null }) {
  const amount = value && value.trim() ? value : '0';
  const suffix = asset && asset.trim() ? ` ${asset}` : '';
  return <span>{amount}{suffix}</span>;
}
```

Create `web/src/shared/StatusTag.tsx`:

```tsx
import { Tag } from '@douyinfe/semi-ui';

type SemiTagColor = 'green' | 'blue' | 'red' | 'grey' | 'orange' | 'violet';

const statusMap: Record<string, { label: string; color: SemiTagColor }> = {
  active: { label: '启用', color: 'green' },
  enabled: { label: '启用', color: 'green' },
  completed: { label: '完成', color: 'green' },
  consumed: { label: '已消费', color: 'green' },
  listed: { label: '已上市', color: 'green' },
  pending: { label: '待处理', color: 'blue' },
  processing: { label: '处理中', color: 'blue' },
  subscription: { label: '发行中', color: 'blue' },
  failed: { label: '失败', color: 'red' },
  rejected: { label: '已拒绝', color: 'red' },
  liquidated: { label: '已强平', color: 'red' },
  disabled: { label: '禁用', color: 'grey' },
  paused: { label: '暂停', color: 'grey' },
  locked: { label: '锁定', color: 'orange' },
  frozen: { label: '冻结', color: 'orange' }
};

export function StatusTag({ value }: { value?: string | null }) {
  const key = (value ?? '').toLowerCase();
  const status = statusMap[key] ?? { label: value || '-', color: 'violet' as const };
  return <Tag color={status.color}>{status.label}</Tag>;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/shared/format.test.tsx
```

Expected: PASS.

---

### Task 3: Auth store and API client

**Files:**
- Create: `web/src/auth/authStore.ts`
- Create: `web/src/auth/authStore.test.ts`
- Create: `web/src/api/types.ts`
- Create: `web/src/api/client.ts`
- Create: `web/src/api/client.test.ts`
- Create: `web/src/api/adminAuth.ts`

- [ ] **Step 1: Write failing auth store tests**

Create `web/src/auth/authStore.test.ts`:

```ts
import { beforeEach, describe, expect, it } from 'vitest';
import { authStore } from './authStore';

describe('authStore', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('saves and restores an admin session', () => {
    authStore.setSession({ accessToken: 'access', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
    expect(authStore.getSession()).toEqual({ accessToken: 'access', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
  });

  it('clears the stored session', () => {
    authStore.setSession({ accessToken: 'access', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
    authStore.clearSession();
    expect(authStore.getSession()).toBeNull();
  });
});
```

- [ ] **Step 2: Run auth tests and verify failure**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/auth/authStore.test.ts
```

Expected: FAIL because `authStore` does not exist.

- [ ] **Step 3: Implement auth store**

Create `web/src/auth/authStore.ts`:

```ts
export type AuthScope = 'admin' | 'agent' | 'user';

export interface AuthSession {
  accessToken: string;
  refreshToken: string;
  scope: AuthScope;
  subject: string;
}

const storageKey = 'exchange_admin_session';

function parseSession(raw: string | null): AuthSession | null {
  if (!raw) return null;
  try {
    const value = JSON.parse(raw) as AuthSession;
    if (!value.accessToken || !value.refreshToken || !value.scope || !value.subject) return null;
    return value;
  } catch {
    return null;
  }
}

export const authStore = {
  getSession(): AuthSession | null {
    return parseSession(localStorage.getItem(storageKey));
  },
  setSession(session: AuthSession): void {
    localStorage.setItem(storageKey, JSON.stringify(session));
  },
  clearSession(): void {
    localStorage.removeItem(storageKey);
  }
};
```

- [ ] **Step 4: Write failing API client tests**

Create `web/src/api/client.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { authStore } from '../auth/authStore';
import { ApiError, apiRequest } from './client';

describe('apiRequest', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it('adds bearer token and returns JSON', async () => {
    authStore.setSession({ accessToken: 'token', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);

    const result = await apiRequest<{ ok: boolean }>('/admin/api/v1/test');

    expect(result.ok).toBe(true);
    expect(fetchMock).toHaveBeenCalledWith('/admin/api/v1/test', expect.objectContaining({
      headers: expect.objectContaining({ Authorization: 'Bearer token' })
    }));
  });

  it('clears session on 401', async () => {
    authStore.setSession({ accessToken: 'token', refreshToken: 'refresh', scope: 'admin', subject: 'admin:1' });
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({ code: 'UNAUTHORIZED', message: 'unauthorized' }), { status: 401 })));

    await expect(apiRequest('/admin/api/v1/test')).rejects.toBeInstanceOf(ApiError);
    expect(authStore.getSession()).toBeNull();
  });
});
```

- [ ] **Step 5: Run API tests and verify failure**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/api/client.test.ts
```

Expected: FAIL because `client.ts` does not exist.

- [ ] **Step 6: Implement API types, client, and Admin auth endpoint**

Create `web/src/api/types.ts`:

```ts
export interface ApiErrorPayload {
  code?: string;
  message?: string;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  scope: 'admin' | 'agent' | 'user';
  subject: string;
}

export interface PageResponse<T> {
  items?: T[];
  logs?: T[];
  orders?: T[];
  trades?: T[];
  projects?: T[];
  markets?: T[];
  pairs?: T[];
  strategies?: T[];
  users?: T[];
  commissions?: T[];
  subscriptions?: T[];
  distributions?: T[];
  purchases?: T[];
  lock_positions?: T[];
  unlocks?: T[];
  positions?: T[];
  liquidations?: T[];
  summaries?: T[];
  products?: T[];
}

export type ApiRecord = Record<string, string | number | boolean | null | object | undefined>;
```

Create `web/src/api/client.ts`:

```ts
import { authStore } from '../auth/authStore';
import type { ApiErrorPayload } from './types';

export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string) {
    super(message);
    this.name = 'ApiError';
  }
}

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? '';

export async function apiRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
  const session = authStore.getSession();
  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json');
  if (session?.accessToken) {
    headers.set('Authorization', `Bearer ${session.accessToken}`);
  }

  const response = await fetch(`${baseUrl}${path}`, { ...init, headers });
  if (!response.ok) {
    const payload = await safeErrorPayload(response);
    if (response.status === 401) {
      authStore.clearSession();
    }
    throw new ApiError(response.status, payload.code ?? `HTTP_${response.status}`, payload.message ?? response.statusText);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return response.json() as Promise<T>;
}

async function safeErrorPayload(response: Response): Promise<ApiErrorPayload> {
  try {
    return (await response.json()) as ApiErrorPayload;
  } catch {
    return { code: `HTTP_${response.status}`, message: response.statusText };
  }
}
```

Create `web/src/api/adminAuth.ts`:

```ts
import { apiRequest } from './client';
import type { LoginRequest, LoginResponse } from './types';

export function adminLogin(payload: LoginRequest) {
  return apiRequest<LoginResponse>('/admin/api/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify(payload)
  });
}
```

- [ ] **Step 7: Verify auth and client tests pass**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/auth/authStore.test.ts src/api/client.test.ts
```

Expected: PASS.

---

### Task 4: Login page, route guard, and router

**Files:**
- Create: `web/src/auth/LoginPage.tsx`
- Create: `web/src/auth/RequireAdmin.tsx`
- Create: `web/src/auth/RequireAdmin.test.tsx`
- Create: `web/src/pages/ForbiddenPage.tsx`
- Create: `web/src/pages/NotFoundPage.tsx`
- Modify: `web/src/app/router.tsx`

- [ ] **Step 1: Write failing route guard tests**

Create `web/src/auth/RequireAdmin.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { authStore } from './authStore';
import { RequireAdmin } from './RequireAdmin';

describe('RequireAdmin', () => {
  beforeEach(() => localStorage.clear());

  it('renders admin content for admin scope', () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'admin', subject: 'admin:1' });
    const router = createMemoryRouter([{ path: '/', element: <RequireAdmin><div>Admin content</div></RequireAdmin> }]);
    render(<RouterProvider router={router} />);
    expect(screen.getByText('Admin content')).toBeInTheDocument();
  });

  it('redirects unauthenticated users to login', async () => {
    const router = createMemoryRouter([
      { path: '/', element: <RequireAdmin><div>Admin content</div></RequireAdmin> },
      { path: '/login', element: <div>登录</div> }
    ]);
    render(<RouterProvider router={router} />);
    expect(await screen.findByText('登录')).toBeInTheDocument();
  });

  it('redirects non-admin sessions to forbidden page', async () => {
    authStore.setSession({ accessToken: 'a', refreshToken: 'r', scope: 'agent', subject: 'agent:1' });
    const router = createMemoryRouter([
      { path: '/', element: <RequireAdmin><div>Admin content</div></RequireAdmin> },
      { path: '/403', element: <div>无权限</div> }
    ]);
    render(<RouterProvider router={router} />);
    expect(await screen.findByText('无权限')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run guard tests and verify failure**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/auth/RequireAdmin.test.tsx
```

Expected: FAIL because `RequireAdmin` does not exist.

- [ ] **Step 3: Implement route guard and static pages**

Create `web/src/auth/RequireAdmin.tsx`:

```tsx
import type { ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { authStore } from './authStore';

export function RequireAdmin({ children }: { children: ReactNode }) {
  const location = useLocation();
  const session = authStore.getSession();
  if (!session) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }
  if (session.scope !== 'admin') {
    return <Navigate to="/403" replace />;
  }
  return <>{children}</>;
}
```

Create `web/src/pages/ForbiddenPage.tsx`:

```tsx
import { Button, Empty } from '@douyinfe/semi-ui';
import { Link } from 'react-router-dom';

export function ForbiddenPage() {
  return <Empty title="无权限" description="当前账号不能访问此页面" footer={<Link to="/login"><Button>返回登录</Button></Link>} />;
}
```

Create `web/src/pages/NotFoundPage.tsx`:

```tsx
import { Empty } from '@douyinfe/semi-ui';

export function NotFoundPage() {
  return <Empty title="页面不存在" description="请检查访问地址" />;
}
```

- [ ] **Step 4: Create login page**

Create `web/src/auth/LoginPage.tsx`:

```tsx
import { Button, Card, Form, RadioGroup, Radio, Toast, Typography } from '@douyinfe/semi-ui';
import { useMutation } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { adminLogin } from '../api/adminAuth';
import { ApiError } from '../api/client';
import { authStore } from './authStore';

const { Title, Text } = Typography;

export function LoginPage() {
  const navigate = useNavigate();
  const mutation = useMutation({
    mutationFn: adminLogin,
    onSuccess: (response) => {
      if (response.scope !== 'admin') {
        Toast.error('当前账号不是管理员');
        return;
      }
      authStore.setSession({
        accessToken: response.access_token,
        refreshToken: response.refresh_token,
        scope: response.scope,
        subject: response.subject
      });
      navigate('/admin/dashboard', { replace: true });
    },
    onError: (error) => {
      const message = error instanceof ApiError ? error.message : '登录失败';
      Toast.error(message);
    }
  });

  return <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', background: '#eef2f7' }}>
    <Card style={{ width: 420 }}>
      <Title heading={3}>交易所管理后台</Title>
      <Text type="tertiary">首期开放管理员后台，代理后台暂未开放</Text>
      <Form style={{ marginTop: 24 }} onSubmit={(values) => mutation.mutate({ email: values.email, password: values.password })}>
        <Form.Slot label="登录身份">
          <RadioGroup value="admin" type="button">
            <Radio value="admin">管理员</Radio>
            <Radio value="agent" disabled>代理暂未开放</Radio>
          </RadioGroup>
        </Form.Slot>
        <Form.Input field="email" label="邮箱" rules={[{ required: true, message: '请输入邮箱' }]} />
        <Form.Input field="password" label="密码" mode="password" rules={[{ required: true, message: '请输入密码' }]} />
        <Button htmlType="submit" theme="solid" type="primary" block loading={mutation.isPending}>登录</Button>
      </Form>
    </Card>
  </div>;
}
```

- [ ] **Step 5: Replace router with Admin routes shell placeholder**

Modify `web/src/app/router.tsx`:

```tsx
import { createBrowserRouter, Navigate } from 'react-router-dom';
import { LoginPage } from '../auth/LoginPage';
import { RequireAdmin } from '../auth/RequireAdmin';
import { ForbiddenPage } from '../pages/ForbiddenPage';
import { NotFoundPage } from '../pages/NotFoundPage';

export const router = createBrowserRouter([
  { path: '/', element: <Navigate to="/login" replace /> },
  { path: '/login', element: <LoginPage /> },
  { path: '/403', element: <ForbiddenPage /> },
  { path: '/admin/*', element: <RequireAdmin><div>Admin layout pending</div></RequireAdmin> },
  { path: '*', element: <NotFoundPage /> }
]);
```

- [ ] **Step 6: Verify route guard tests pass**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/auth/RequireAdmin.test.tsx
```

Expected: PASS.

---

### Task 5: Admin layout and menu

**Files:**
- Create: `web/src/layouts/PageHeader.tsx`
- Create: `web/src/layouts/AdminLayout.tsx`
- Create: `web/src/layouts/AdminLayout.test.tsx`
- Create: `web/src/admin/routes.tsx`
- Modify: `web/src/app/router.tsx`

- [ ] **Step 1: Write failing Admin layout test**

Create `web/src/layouts/AdminLayout.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { AdminLayout } from './AdminLayout';

describe('AdminLayout', () => {
  it('renders the Admin navigation menu', async () => {
    const router = createMemoryRouter([{ path: '/admin', element: <AdminLayout /> }], { initialEntries: ['/admin'] });
    render(<RouterProvider router={router} />);
    expect(await screen.findByText('Admin 后台')).toBeInTheDocument();
    expect(screen.getByText('新币管理')).toBeInTheDocument();
    expect(screen.getByText('审计日志')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run layout test and verify failure**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/layouts/AdminLayout.test.tsx
```

Expected: FAIL because `AdminLayout` does not exist.

- [ ] **Step 3: Implement PageHeader and AdminLayout**

Create `web/src/layouts/PageHeader.tsx`:

```tsx
import { Typography } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';

const { Title, Text } = Typography;

export function PageHeader({ title, description, extra }: { title: string; description?: string; extra?: ReactNode }) {
  return <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 16 }}>
    <div>
      <Title heading={4} style={{ margin: 0 }}>{title}</Title>
      {description ? <Text type="tertiary">{description}</Text> : null}
    </div>
    {extra}
  </div>;
}
```

Create `web/src/layouts/AdminLayout.tsx`:

```tsx
import { IconExit, IconHome, IconSetting, IconUserGroup } from '@douyinfe/semi-icons';
import { Button, Layout, Nav, Typography } from '@douyinfe/semi-ui';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { authStore } from '../auth/authStore';

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

const menuItems = [
  { itemKey: '/admin/dashboard', text: '仪表盘', icon: <IconHome /> },
  { itemKey: 'users', text: '用户与代理', icon: <IconUserGroup />, items: [
    { itemKey: '/admin/users', text: '用户管理' },
    { itemKey: '/admin/agents', text: '代理管理' },
    { itemKey: '/admin/agent-commissions', text: '代理返佣' }
  ] },
  { itemKey: 'wallet', text: '资产与交易', items: [
    { itemKey: '/admin/wallet/accounts', text: '钱包账户' },
    { itemKey: '/admin/wallet/ledger', text: '资产流水' },
    { itemKey: '/admin/spot/orders', text: '现货订单' },
    { itemKey: '/admin/spot/trades', text: '现货成交' }
  ] },
  { itemKey: 'newCoins', text: '新币管理', items: [
    { itemKey: '/admin/new-coins/projects', text: '新币项目' },
    { itemKey: '/admin/new-coins/subscriptions', text: '申购记录' },
    { itemKey: '/admin/new-coins/distributions', text: '派发记录' },
    { itemKey: '/admin/new-coins/purchases', text: '上市后认购' },
    { itemKey: '/admin/new-coins/lock-positions', text: '锁定仓位' },
    { itemKey: '/admin/new-coins/unlocks', text: '解禁记录' }
  ] },
  { itemKey: 'market', text: '行情管理', items: [
    { itemKey: '/admin/market/pairs', text: '交易对' },
    { itemKey: '/admin/market/strategies', text: '行情策略' }
  ] },
  { itemKey: 'convert', text: '闪兑管理', items: [
    { itemKey: '/admin/convert/pairs', text: '闪兑币对' },
    { itemKey: '/admin/convert/rules', text: '新币汇率规则' },
    { itemKey: '/admin/convert/orders', text: '闪兑订单' }
  ] },
  { itemKey: 'seconds', text: '秒合约', items: [
    { itemKey: '/admin/seconds-contract/products', text: '产品配置' },
    { itemKey: '/admin/seconds-contract/orders', text: '订单记录' }
  ] },
  { itemKey: 'margin', text: '杠杆', items: [
    { itemKey: '/admin/margin/products', text: '产品配置' },
    { itemKey: '/admin/margin/positions', text: '仓位记录' },
    { itemKey: '/admin/margin/liquidations', text: '强平记录' },
    { itemKey: '/admin/margin/interest', text: '利息记录' }
  ] },
  { itemKey: 'earn', text: '理财 Earn', items: [
    { itemKey: '/admin/earn/products', text: '产品配置' },
    { itemKey: '/admin/earn/subscriptions', text: '申购记录' }
  ] },
  { itemKey: 'risk', text: '风控与审计', icon: <IconSetting />, items: [
    { itemKey: '/admin/risk', text: '风控概览' },
    { itemKey: '/admin/audit-logs', text: '审计日志' }
  ] }
];

export function AdminLayout() {
  const location = useLocation();
  const navigate = useNavigate();
  const session = authStore.getSession();
  return <Layout style={{ minHeight: '100vh' }}>
    <Sider style={{ background: '#111827' }}>
      <div style={{ color: '#fff', padding: 20, fontWeight: 700 }}>Admin 后台</div>
      <Nav items={menuItems} selectedKeys={[location.pathname]} onClick={({ itemKey }) => navigate(String(itemKey))} style={{ maxWidth: 260 }} />
    </Sider>
    <Layout>
      <Header style={{ background: '#fff', padding: '0 20px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Text>当前账号：{session?.subject ?? '-'}</Text>
        <Button icon={<IconExit />} onClick={() => { authStore.clearSession(); navigate('/login', { replace: true }); }}>退出登录</Button>
      </Header>
      <Content className="exchange-page"><Outlet /></Content>
    </Layout>
  </Layout>;
}
```

- [ ] **Step 4: Create Admin route registry and wire router**

Create `web/src/admin/routes.tsx`:

```tsx
import { Navigate, RouteObject } from 'react-router-dom';
import { DashboardPage } from './dashboard/DashboardPage';

export const adminRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  { path: 'dashboard', element: <DashboardPage /> }
];
```

Create `web/src/admin/dashboard/DashboardPage.tsx`:

```tsx
import { Card, Empty } from '@douyinfe/semi-ui';
import { PageHeader } from '../../layouts/PageHeader';

export function DashboardPage() {
  return <div>
    <PageHeader title="仪表盘" description="交易所运营入口与关键状态" />
    <div className="exchange-card-grid">
      {['今日注册用户', '今日交易订单数', '今日闪兑订单数', '待解禁数量'].map((title) => <Card key={title} title={title}><Empty description="暂无聚合数据" /></Card>)}
    </div>
  </div>;
}
```

Modify `web/src/app/router.tsx`:

```tsx
import { createBrowserRouter, Navigate } from 'react-router-dom';
import { adminRoutes } from '../admin/routes';
import { LoginPage } from '../auth/LoginPage';
import { RequireAdmin } from '../auth/RequireAdmin';
import { AdminLayout } from '../layouts/AdminLayout';
import { ForbiddenPage } from '../pages/ForbiddenPage';
import { NotFoundPage } from '../pages/NotFoundPage';

export const router = createBrowserRouter([
  { path: '/', element: <Navigate to="/login" replace /> },
  { path: '/login', element: <LoginPage /> },
  { path: '/403', element: <ForbiddenPage /> },
  { path: '/admin', element: <RequireAdmin><AdminLayout /></RequireAdmin>, children: adminRoutes },
  { path: '*', element: <NotFoundPage /> }
]);
```

- [ ] **Step 5: Verify layout test passes**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/layouts/AdminLayout.test.tsx
```

Expected: PASS.

---

### Task 6: Admin resource API and reusable list page

**Files:**
- Create: `web/src/api/adminResources.ts`
- Create: `web/src/shared/DataTable.tsx`
- Create: `web/src/shared/FilterBar.tsx`
- Create: `web/src/shared/JsonDrawer.tsx`
- Create: `web/src/shared/ConfirmAction.tsx`
- Create: `web/src/admin/resources/AdminResourcePage.tsx`
- Create: `web/src/admin/resources/AdminResourcePage.test.tsx`
- Create: `web/src/admin/resources/AdminNoticePage.tsx`

- [ ] **Step 1: Write failing resource page test**

Create `web/src/admin/resources/AdminResourcePage.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { AdminResourcePage } from './AdminResourcePage';

vi.mock('../../api/adminResources', () => ({
  listAdminResource: vi.fn().mockResolvedValue({ rows: [{ id: 1, status: 'active', created_at: 1783740153000 }], raw: {} })
}));

describe('AdminResourcePage', () => {
  it('renders rows from a configured admin endpoint', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><AdminResourcePage title="审计日志" endpoint="/admin/api/v1/audit-logs" responseKey="logs" columns={[{ key: 'id', title: 'ID' }, { key: 'status', title: '状态' }]} /></QueryClientProvider>);
    expect(await screen.findByText('审计日志')).toBeInTheDocument();
    expect(await screen.findByText('active')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run resource page test and verify failure**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/admin/resources/AdminResourcePage.test.tsx
```

Expected: FAIL because `AdminResourcePage` does not exist.

- [ ] **Step 3: Implement admin resource API wrapper**

Create `web/src/api/adminResources.ts`:

```ts
import { apiRequest } from './client';
import type { ApiRecord } from './types';

export interface AdminResourceResult {
  rows: ApiRecord[];
  raw: Record<string, unknown>;
}

export async function listAdminResource(endpoint: string, responseKey: string, filters: Record<string, string | number | undefined> = {}): Promise<AdminResourceResult> {
  const params = new URLSearchParams();
  Object.entries(filters).forEach(([key, value]) => {
    if (value !== undefined && value !== '') params.set(key, String(value));
  });
  const query = params.toString();
  const raw = await apiRequest<Record<string, unknown>>(`${endpoint}${query ? `?${query}` : ''}`);
  const value = raw[responseKey];
  return { rows: Array.isArray(value) ? value as ApiRecord[] : [], raw };
}
```

- [ ] **Step 4: Implement shared list primitives**

Create `web/src/shared/DataTable.tsx`:

```tsx
import { Banner, Button, Empty, Spin, Table } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import type { ApiRecord } from '../api/types';

export function DataTable({ loading, error, data, columns, onRetry }: { loading: boolean; error?: Error | null; data: ApiRecord[]; columns: ColumnProps<ApiRecord>[]; onRetry: () => void }) {
  if (loading) return <Spin />;
  if (error) return <Banner type="danger" title="加载失败" description={error.message} closeIcon={null} fullMode={false} extra={<Button onClick={onRetry}>重试</Button>} />;
  if (data.length === 0) return <Empty description="暂无数据" />;
  return <Table dataSource={data} columns={columns} pagination={false} rowKey={(record) => String(record.id ?? JSON.stringify(record))} />;
}
```

Create `web/src/shared/FilterBar.tsx`:

```tsx
import { Button, Input, Space } from '@douyinfe/semi-ui';
import type { FormEvent } from 'react';

export interface FilterField {
  key: string;
  label: string;
}

export function FilterBar({ fields, values, onChange, onSubmit }: { fields: FilterField[]; values: Record<string, string>; onChange: (key: string, value: string) => void; onSubmit: () => void }) {
  function submit(event: FormEvent) {
    event.preventDefault();
    onSubmit();
  }
  return <form onSubmit={submit} style={{ marginBottom: 16 }}>
    <Space wrap>
      {fields.map((field) => <Input key={field.key} prefix={field.label} value={values[field.key] ?? ''} onChange={(value) => onChange(field.key, value)} />)}
      <Button htmlType="submit" theme="solid" type="primary">筛选</Button>
    </Space>
  </form>;
}
```

Create `web/src/shared/JsonDrawer.tsx`:

```tsx
import { Button, Drawer, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

export function JsonDrawer({ title, value }: { title: string; value: unknown }) {
  const [visible, setVisible] = useState(false);
  return <>
    <Button size="small" onClick={() => setVisible(true)}>查看</Button>
    <Drawer title={title} visible={visible} onCancel={() => setVisible(false)} width={520}>
      <Typography.Paragraph copyable style={{ whiteSpace: 'pre-wrap' }}>{JSON.stringify(value ?? {}, null, 2)}</Typography.Paragraph>
    </Drawer>
  </>;
}
```

Create `web/src/shared/ConfirmAction.tsx`:

```tsx
import { Button, Input, Modal, Toast, Typography } from '@douyinfe/semi-ui';
import { useState } from 'react';

export function ConfirmAction({ label, summary, danger, onConfirm }: { label: string; summary: string; danger?: boolean; onConfirm: (reason: string) => Promise<void> }) {
  const [visible, setVisible] = useState(false);
  const [reason, setReason] = useState('');
  const [loading, setLoading] = useState(false);
  async function confirm() {
    if (!reason.trim()) {
      Toast.warning('请输入操作原因');
      return;
    }
    setLoading(true);
    try {
      await onConfirm(reason.trim());
      setVisible(false);
      setReason('');
      Toast.success('操作已提交');
    } finally {
      setLoading(false);
    }
  }
  return <>
    <Button type={danger ? 'danger' : 'primary'} onClick={() => setVisible(true)}>{label}</Button>
    <Modal title="二次确认" visible={visible} confirmLoading={loading} onOk={confirm} onCancel={() => setVisible(false)} okText="确认执行" cancelText="取消">
      <Typography.Paragraph>{summary}</Typography.Paragraph>
      <Input.TextArea placeholder="请输入操作原因" value={reason} onChange={setReason} autosize />
    </Modal>
  </>;
}
```

Create `web/src/admin/resources/AdminNoticePage.tsx`:

```tsx
import { Banner, Card, Typography } from '@douyinfe/semi-ui';
import type { ReactNode } from 'react';
import { PageHeader } from '../../layouts/PageHeader';

const { Paragraph } = Typography;

export function AdminNoticePage({ title, description, warning, children }: { title: string; description: string; warning?: string; children?: ReactNode }) {
  return <div>
    <PageHeader title={title} description={description} />
    {warning ? <Banner type="warning" title="操作提示" description={warning} closeIcon={null} fullMode={false} /> : null}
    <Card style={{ marginTop: 16 }}>
      {children ?? <Paragraph>当前后端未提供该页面的 Admin 查询接口，首期仅保留菜单入口，不调用用户端或代理端接口冒充真实数据。</Paragraph>}
    </Card>
  </div>;
}
```

- [ ] **Step 5: Implement AdminResourcePage**

Create `web/src/admin/resources/AdminResourcePage.tsx`:

```tsx
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { listAdminResource } from '../../api/adminResources';
import type { ApiRecord } from '../../api/types';
import { PageHeader } from '../../layouts/PageHeader';
import { DataTable } from '../../shared/DataTable';
import { FilterBar, type FilterField } from '../../shared/FilterBar';

export function AdminResourcePage({ title, description, endpoint, responseKey, filters = [], columns }: { title: string; description?: string; endpoint: string; responseKey: string; filters?: FilterField[]; columns: ColumnProps<ApiRecord>[] }) {
  const [filterValues, setFilterValues] = useState<Record<string, string>>({});
  const query = useQuery({
    queryKey: ['admin-resource', endpoint, responseKey, filterValues],
    queryFn: () => listAdminResource(endpoint, responseKey, filterValues)
  });
  return <div>
    <PageHeader title={title} description={description} />
    {filters.length ? <FilterBar fields={filters} values={filterValues} onChange={(key, value) => setFilterValues((current) => ({ ...current, [key]: value }))} onSubmit={() => query.refetch()} /> : null}
    <DataTable loading={query.isLoading} error={query.error} data={query.data?.rows ?? []} columns={columns} onRetry={() => query.refetch()} />
  </div>;
}
```

- [ ] **Step 6: Verify resource page test passes**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run test -- src/admin/resources/AdminResourcePage.test.tsx
```

Expected: PASS.

---

### Task 7: Configure Admin read pages and routes

**Files:**
- Create: `web/src/admin/resources/resourceConfigs.tsx`
- Modify: `web/src/admin/routes.tsx`

- [ ] **Step 1: Create resource configs for all read pages**

Create `web/src/admin/resources/resourceConfigs.tsx`:

```tsx
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import type { ApiRecord } from '../../api/types';
import { AmountText } from '../../shared/AmountText';
import { JsonDrawer } from '../../shared/JsonDrawer';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';

function text(key: string, title: string): ColumnProps<ApiRecord> {
  return { title, dataIndex: key, render: (value) => String(value ?? '-') };
}

function status(key = 'status'): ColumnProps<ApiRecord> {
  return { title: '状态', dataIndex: key, render: (value) => <StatusTag value={typeof value === 'string' ? value : null} /> };
}

function time(key: string, title = '时间'): ColumnProps<ApiRecord> {
  return { title, dataIndex: key, render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> };
}

function amount(amountKey: string, assetKey: string, title = '金额'): ColumnProps<ApiRecord> {
  return { title, render: (_, record) => <AmountText value={String(record[amountKey] ?? '')} asset={String(record[assetKey] ?? '')} /> };
}

export const resourceConfigs = {
  commissions: { title: '代理返佣', endpoint: '/admin/api/v1/agent-commissions', responseKey: 'commissions', filters: [{ key: 'agent_id', label: '代理ID' }, { key: 'user_id', label: '用户ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('agent_id', '代理ID'), text('user_id', '用户ID'), text('source_type', '来源'), amount('commission_amount', '', '返佣金额'), status(), time('created_at', '创建时间')] },
  markets: { title: '交易对', endpoint: '/api/v1/markets', responseKey: 'markets', columns: [text('id', 'ID'), text('symbol', '交易对'), text('base_asset', '基础资产'), text('quote_asset', '计价资产'), text('market_type', '行情类型'), status()] },
  spotOrders: { title: '现货订单', endpoint: '/admin/api/v1/spot/orders', responseKey: 'orders', filters: [{ key: 'pair_id', label: '交易对' }, { key: 'user_id', label: '用户ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('user_id', '用户ID'), text('pair_id', '交易对'), text('side', '方向'), text('order_type', '类型'), amount('quantity', '', '数量'), amount('filled_quantity', '', '已成交'), status()] },
  spotTrades: { title: '现货成交', endpoint: '/admin/api/v1/spot/trades', responseKey: 'trades', filters: [{ key: 'pair_id', label: '交易对' }, { key: 'user_id', label: '用户ID' }], columns: [text('id', 'ID'), text('pair_id', '交易对'), text('buy_order_id', '买单'), text('sell_order_id', '卖单'), amount('quantity', '', '数量'), amount('price', '', '价格'), amount('fee', '', '手续费'), time('created_at', '成交时间')] },
  newCoinProjects: { title: '新币项目', endpoint: '/admin/api/v1/new-coins', responseKey: 'projects', columns: [text('id', 'ID'), text('symbol', '币种'), text('asset_id', '资产ID'), text('lifecycle_status', '生命周期'), amount('total_supply', '', '总量'), amount('issue_price', '', '发行价'), text('unlock_type', '解禁规则'), status(), time('listed_at', '上市时间')] },
  newCoinPurchases: { title: '上市后认购', endpoint: '/admin/api/v1/new-coins/purchases', responseKey: 'purchases', filters: [{ key: 'project_id', label: '项目ID' }, { key: 'user_id', label: '用户ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('project_id', '项目ID'), text('user_id', '用户ID'), amount('quantity', '', '认购数量'), amount('quote_amount', '', '支付金额'), status(), time('created_at', '创建时间')] },
  newCoinLockPositions: { title: '锁定仓位', endpoint: '/admin/api/v1/new-coins/lock-positions', responseKey: 'lock_positions', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'asset_id', label: '资产ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('user_id', '用户ID'), text('asset_id', '资产ID'), text('unlock_type', '解禁类型'), amount('remaining_amount', '', '剩余锁定'), status(), time('unlock_at', '解禁时间')] },
  newCoinUnlocks: { title: '解禁记录', endpoint: '/admin/api/v1/new-coins/unlocks', responseKey: 'unlocks', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'asset_id', label: '资产ID' }, { key: 'fee_paid_status', label: '矿工费' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('user_id', '用户ID'), text('asset_id', '资产ID'), amount('unlock_quantity', '', '解禁数量'), text('fee_paid_status', '矿工费状态'), status(), time('created_at', '创建时间')] },
  convertPairs: { title: '闪兑币对', endpoint: '/admin/api/v1/convert/pairs', responseKey: 'pairs', columns: [text('id', 'ID'), text('from_asset_id', 'From 资产'), text('to_asset_id', 'To 资产'), text('pricing_mode', '报价模式'), amount('spread_rate', '', '价差'), amount('min_amount', '', '最小额'), status('enabled')] },
  convertOrders: { title: '闪兑订单', endpoint: '/admin/api/v1/convert/orders', responseKey: 'orders', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('quote_id', '报价ID'), text('user_id', '用户ID'), amount('from_amount', '', '支付'), amount('to_amount', '', '获得'), amount('rate', '', '汇率'), status(), time('created_at', '创建时间')] },
  marketStrategies: { title: '行情策略', endpoint: '/admin/api/v1/market-strategies', responseKey: 'strategies', filters: [{ key: 'pair_id', label: '交易对ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('symbol', '交易对'), text('market_type', '行情类型'), text('strategy_type', '策略'), amount('start_price', '', '起始价'), amount('target_price', '', '目标价'), status(), time('created_at', '创建时间')] },
  secondsProducts: { title: '秒合约产品配置', endpoint: '/admin/api/v1/seconds-contracts/products', responseKey: 'products', columns: [text('id', 'ID'), text('symbol', '交易对'), text('duration_seconds', '周期秒数'), amount('payout_rate', '', '赔付率'), amount('min_stake', '', '最小本金'), status()] },
  secondsOrders: { title: '秒合约订单', endpoint: '/admin/api/v1/seconds-contracts/orders', responseKey: 'orders', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('user_id', '用户ID'), text('product_id', '产品ID'), text('direction', '方向'), amount('stake_amount', '', '本金'), status(), time('expires_at', '到期时间')] },
  marginProducts: { title: '杠杆产品配置', endpoint: '/admin/api/v1/margin/products', responseKey: 'products', columns: [text('id', 'ID'), text('symbol', '交易对'), text('margin_asset_symbol', '保证金资产'), amount('max_leverage', '', '最大杠杆'), amount('min_margin', '', '最小保证金'), status()] },
  marginPositions: { title: '杠杆仓位', endpoint: '/admin/api/v1/margin/positions', responseKey: 'positions', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'pair_id', label: '交易对ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('user_id', '用户ID'), text('pair_id', '交易对ID'), text('direction', '方向'), amount('margin_amount', '', '保证金'), amount('interest_amount', '', '利息'), status()] },
  marginLiquidations: { title: '强平记录', endpoint: '/admin/api/v1/margin/liquidations', responseKey: 'liquidations', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'pair_id', label: '交易对ID' }, { key: 'position_id', label: '仓位ID' }], columns: [text('id', 'ID'), text('position_id', '仓位ID'), text('user_id', '用户ID'), amount('equity', '', '权益'), amount('maintenance_margin', '', '维持保证金'), amount('realized_pnl', '', '已实现盈亏'), time('liquidated_at', '强平时间')] },
  marginInterest: { title: '利息记录', endpoint: '/admin/api/v1/margin/interest/summary', responseKey: 'summaries', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'pair_id', label: '交易对ID' }, { key: 'status', label: '状态' }], columns: [text('margin_asset', '资产ID'), status(), text('position_count', '仓位数'), amount('borrowed_amount', '', '借款金额'), amount('interest_amount', '', '利息')] },
  earnProducts: { title: 'Earn 产品配置', endpoint: '/admin/api/v1/earn/products', responseKey: 'products', columns: [text('id', 'ID'), text('asset_symbol', '资产'), text('name', '名称'), text('term_days', '期限'), amount('apr_rate', '', 'APR'), amount('min_subscribe', '', '最小申购'), status()] },
  earnSubscriptions: { title: 'Earn 申购记录', endpoint: '/admin/api/v1/earn/subscriptions', responseKey: 'subscriptions', filters: [{ key: 'user_id', label: '用户ID' }, { key: 'status', label: '状态' }], columns: [text('id', 'ID'), text('user_id', '用户ID'), text('product_id', '产品ID'), amount('amount', '', '本金'), amount('apr_rate', '', 'APR'), status(), time('matures_at', '到期时间')] },
  auditLogs: { title: '审计日志', endpoint: '/admin/api/v1/audit-logs', responseKey: 'logs', filters: [{ key: 'admin_id', label: '管理员ID' }, { key: 'action', label: '操作' }, { key: 'target_type', label: '目标类型' }, { key: 'target_id', label: '目标ID' }], columns: [text('id', 'ID'), text('admin_id', '管理员'), text('action', '操作'), text('target_type', '目标类型'), text('target_id', '目标ID'), text('reason', '原因'), text('ip', 'IP'), time('created_at', '创建时间'), { title: '变更前', render: (_, record) => <JsonDrawer title="变更前" value={record.before_json} /> }, { title: '变更后', render: (_, record) => <JsonDrawer title="变更后" value={record.after_json} /> }] }
};
```

- [ ] **Step 2: Wire resource pages into Admin routes**

Modify `web/src/admin/routes.tsx`:

```tsx
import { Navigate, RouteObject } from 'react-router-dom';
import { DashboardPage } from './dashboard/DashboardPage';
import { AdminNoticePage } from './resources/AdminNoticePage';
import { AdminResourcePage } from './resources/AdminResourcePage';
import { resourceConfigs } from './resources/resourceConfigs';

function page(key: keyof typeof resourceConfigs) {
  const config = resourceConfigs[key];
  return <AdminResourcePage {...config} />;
}

const noAdminApi = (title: string, description: string) => <AdminNoticePage title={title} description={description} warning="后端当前未提供独立 Admin 查询接口；本页面首期不调用用户端或代理端接口，避免越权和假数据。" />;

export const adminRoutes: RouteObject[] = [
  { index: true, element: <Navigate to="dashboard" replace /> },
  { path: 'dashboard', element: <DashboardPage /> },
  { path: 'users', element: noAdminApi('用户管理', '查看用户列表需要后端补充 Admin 用户查询接口') },
  { path: 'agents', element: <AdminNoticePage title="代理管理" description="创建代理、启用或禁用代理、查看指定代理团队用户" warning="后端提供 POST /admin/api/v1/agents、PATCH /admin/api/v1/agents/:id/status、GET /admin/api/v1/agents/:id/users；首期通过操作页按代理 ID 查询团队用户，不调用不存在的代理列表接口。" /> },
  { path: 'agent-commissions', element: page('commissions') },
  { path: 'wallet/accounts', element: noAdminApi('钱包账户', '钱包账户当前只有用户端查询接口') },
  { path: 'wallet/ledger', element: noAdminApi('资产流水', '资产流水当前只有用户端查询接口') },
  { path: 'spot/orders', element: page('spotOrders') },
  { path: 'spot/trades', element: page('spotTrades') },
  { path: 'new-coins/projects', element: page('newCoinProjects') },
  { path: 'new-coins/subscriptions', element: noAdminApi('申购记录', '请输入项目 ID 后调用 /admin/api/v1/new-coins/:id/subscriptions，避免硬编码项目') },
  { path: 'new-coins/distributions', element: noAdminApi('派发记录', '请输入项目 ID 后调用 /admin/api/v1/new-coins/:id/distributions，避免硬编码项目') },
  { path: 'new-coins/purchases', element: page('newCoinPurchases') },
  { path: 'new-coins/lock-positions', element: page('newCoinLockPositions') },
  { path: 'new-coins/unlocks', element: page('newCoinUnlocks') },
  { path: 'market/pairs', element: page('markets') },
  { path: 'market/strategies', element: page('marketStrategies') },
  { path: 'convert/pairs', element: page('convertPairs') },
  { path: 'convert/rules', element: noAdminApi('新币汇率规则', '后端当前提供 POST /admin/api/v1/convert/new-coin-rules 写入接口，未提供 GET 列表接口') },
  { path: 'convert/orders', element: page('convertOrders') },
  { path: 'seconds-contract/products', element: page('secondsProducts') },
  { path: 'seconds-contract/orders', element: page('secondsOrders') },
  { path: 'margin/products', element: page('marginProducts') },
  { path: 'margin/positions', element: page('marginPositions') },
  { path: 'margin/liquidations', element: page('marginLiquidations') },
  { path: 'margin/interest', element: page('marginInterest') },
  { path: 'earn/products', element: page('earnProducts') },
  { path: 'earn/subscriptions', element: page('earnSubscriptions') },
  { path: 'risk', element: noAdminApi('风控概览', '风控概览首期展示入口，等待后端聚合接口') },
  { path: 'audit-logs', element: page('auditLogs') }
];
```

- [ ] **Step 3: Run typecheck and fix any endpoint config type errors**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run typecheck
```

Expected: PASS. If TypeScript reports config spread errors, make `resourceConfigs` values consistent with `AdminResourcePage` props rather than weakening types.

---

### Task 8: Admin write/action pages for agent, new coin, market, convert, and product operations

**Files:**
- Create: `web/src/admin/actions/AgentManagementPage.tsx`
- Create: `web/src/admin/actions/NewCoinActions.tsx`
- Create: `web/src/admin/actions/MarketStrategyActions.tsx`
- Create: `web/src/admin/actions/ConvertRuleActions.tsx`
- Create: `web/src/admin/actions/ProductStatusActions.tsx`
- Modify: `web/src/admin/routes.tsx`

- [ ] **Step 1: Create agent management action page**

Create `web/src/admin/actions/AgentManagementPage.tsx`:

```tsx
import { Banner, Card, Form } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

export function AgentManagementPage() {
  const [agentId, setAgentId] = useState('');
  const [userId, setUserId] = useState('');
  const [agentCode, setAgentCode] = useState('');
  const [adminUsername, setAdminUsername] = useState('');
  const [adminPasswordHash, setAdminPasswordHash] = useState('');
  const [status, setStatus] = useState('disabled');
  return <div>
    <PageHeader title="代理管理" description="创建代理、启用或禁用代理、按代理 ID 查看团队用户" />
    <Banner type="warning" title="高风险操作" description="创建代理和状态变更会影响用户归属与返佣，必须填写原因并二次确认。" closeIcon={null} fullMode={false} />
    <Card title="创建代理" style={{ marginTop: 16 }}>
      <Form layout="horizontal">
        <Form.Input field="user_id" label="用户ID" value={userId} onChange={setUserId} />
        <Form.Input field="agent_code" label="代理码" value={agentCode} onChange={setAgentCode} />
        <Form.Input field="admin_username" label="后台账号" value={adminUsername} onChange={setAdminUsername} />
        <Form.Input field="admin_password_hash" label="密码哈希" value={adminPasswordHash} onChange={setAdminPasswordHash} />
      </Form>
      <ConfirmAction label="创建代理" summary="提交 POST /admin/api/v1/agents" onConfirm={(reason) => apiRequest('/admin/api/v1/agents', { method: 'POST', body: JSON.stringify({ user_id: Number(userId), agent_code: agentCode, admin_username: adminUsername, admin_password_hash: adminPasswordHash, reason }) })} />
    </Card>
    <Card title="代理状态" style={{ marginTop: 16 }}>
      <Form layout="horizontal">
        <Form.Input field="agent_id" label="代理ID" value={agentId} onChange={setAgentId} />
        <Form.Select field="status" label="目标状态" value={status} onChange={(value) => setStatus(String(value))} optionList={[{ label: '禁用', value: 'disabled' }, { label: '启用', value: 'active' }]} />
      </Form>
      <ConfirmAction danger label="更新代理状态" summary="提交 PATCH /admin/api/v1/agents/:id/status" onConfirm={(reason) => apiRequest(`/admin/api/v1/agents/${agentId}/status`, { method: 'PATCH', body: JSON.stringify({ status, reason }) })} />
    </Card>
  </div>;
}
```

- [ ] **Step 2: Create new coin action page with real request fields**

Create `web/src/admin/actions/NewCoinActions.tsx`:

```tsx
import { Banner, Card, Form } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

export function NewCoinActionsPage() {
  const [projectId, setProjectId] = useState('');
  const [lifecycleStatus, setLifecycleStatus] = useState('subscription');
  const [listedAt, setListedAt] = useState('');
  return <div>
    <PageHeader title="新币项目操作" description="调整生命周期、派发、配置解禁与矿工费" />
    <Banner type="warning" title="高风险配置" description="生命周期、派发和解禁配置会影响用户资产，所有操作必须填写原因并二次确认。" closeIcon={null} fullMode={false} />
    <Card title="生命周期变更" style={{ marginTop: 16 }}>
      <Form layout="horizontal">
        <Form.Input field="project_id" label="项目ID" value={projectId} onChange={setProjectId} />
        <Form.Select field="lifecycle_status" label="生命周期" value={lifecycleStatus} onChange={(value) => setLifecycleStatus(String(value))} optionList={[{ label: '预热', value: 'preheat' }, { label: '发行', value: 'subscription' }, { label: '派发', value: 'distribution' }, { label: '上市', value: 'listed' }]} />
        <Form.Input field="listed_at" label="上市时间戳毫秒" value={listedAt} onChange={setListedAt} />
      </Form>
      <ConfirmAction danger label="更新生命周期" summary="提交 PATCH /admin/api/v1/new-coins/:id/lifecycle" onConfirm={(reason) => apiRequest(`/admin/api/v1/new-coins/${projectId}/lifecycle`, { method: 'PATCH', body: JSON.stringify({ lifecycle_status: lifecycleStatus, listed_at: listedAt ? Number(listedAt) : undefined, reason }) })} />
    </Card>
  </div>;
}
```

- [ ] **Step 3: Create market strategy action page with real endpoints**

Create `web/src/admin/actions/MarketStrategyActions.tsx`:

```tsx
import { Banner, Card, Form } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

export function MarketStrategyActionsPage() {
  const [strategyId, setStrategyId] = useState('');
  const [status, setStatus] = useState('paused');
  return <div>
    <PageHeader title="行情策略操作" description="管理 internal / strategy 市场的策略运行状态" />
    <Banner type="warning" title="外部行情不可人工控制" description="只有 internal / strategy 类型交易对允许后台策略控制，external 行情必须来自外部数据源。" closeIcon={null} fullMode={false} />
    <Card title="状态变更" style={{ marginTop: 16 }}>
      <Form layout="horizontal">
        <Form.Input field="strategy_id" label="策略ID" value={strategyId} onChange={setStrategyId} />
        <Form.Select field="status" label="目标状态" value={status} onChange={(value) => setStatus(String(value))} optionList={[{ label: '暂停', value: 'paused' }, { label: '运行', value: 'active' }, { label: '停止', value: 'stopped' }]} />
      </Form>
      <ConfirmAction danger label="更新策略状态" summary="提交 PATCH /admin/api/v1/market-strategies/:id/status" onConfirm={(reason) => apiRequest(`/admin/api/v1/market-strategies/${strategyId}/status`, { method: 'PATCH', body: JSON.stringify({ status, reason }) })} />
    </Card>
  </div>;
}
```

- [ ] **Step 4: Create convert rule and product status action pages**

Create `web/src/admin/actions/ConvertRuleActions.tsx`:

```tsx
import { Banner, Card, Form } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

export function ConvertRuleActionsPage() {
  const [convertPairId, setConvertPairId] = useState('');
  const [rateSource, setRateSource] = useState('fixed');
  const [fixedRate, setFixedRate] = useState('');
  const [status, setStatus] = useState('active');
  return <div>
    <PageHeader title="新币汇率规则" description="创建或更新新币闪兑汇率规则" />
    <Banner type="warning" title="报价风险" description="后端当前提供写入接口 POST /admin/api/v1/convert/new-coin-rules，未提供列表接口；本页只提交明确表单，不展示假列表。" closeIcon={null} fullMode={false} />
    <Card title="汇率规则" style={{ marginTop: 16 }}>
      <Form layout="horizontal">
        <Form.Input field="convert_pair_id" label="闪兑币对ID" value={convertPairId} onChange={setConvertPairId} />
        <Form.Select field="rate_source" label="汇率来源" value={rateSource} onChange={(value) => setRateSource(String(value))} optionList={[{ label: '固定汇率', value: 'fixed' }, { label: '浮动规则', value: 'floating' }]} />
        <Form.Input field="fixed_rate" label="固定汇率" value={fixedRate} onChange={setFixedRate} />
        <Form.Select field="status" label="状态" value={status} onChange={(value) => setStatus(String(value))} optionList={[{ label: '启用', value: 'active' }, { label: '禁用', value: 'disabled' }]} />
      </Form>
      <ConfirmAction danger label="保存汇率规则" summary="提交 POST /admin/api/v1/convert/new-coin-rules" onConfirm={(reason) => apiRequest('/admin/api/v1/convert/new-coin-rules', { method: 'POST', body: JSON.stringify({ convert_pair_id: Number(convertPairId), rate_source: rateSource, fixed_rate: fixedRate || undefined, status, reason }) })} />
    </Card>
  </div>;
}
```

Create `web/src/admin/actions/ProductStatusActions.tsx`:

```tsx
import { Card, Form } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

export function ProductStatusPage({ title, description, endpointPrefix }: { title: string; description: string; endpointPrefix: string }) {
  const [productId, setProductId] = useState('');
  const [status, setStatus] = useState('disabled');
  return <div>
    <PageHeader title={title} description={description} />
    <Card title="产品状态" style={{ marginTop: 16 }}>
      <Form layout="horizontal">
        <Form.Input field="product_id" label="产品ID" value={productId} onChange={setProductId} />
        <Form.Select field="status" label="目标状态" value={status} onChange={(value) => setStatus(String(value))} optionList={[{ label: '禁用', value: 'disabled' }, { label: '启用', value: 'active' }]} />
      </Form>
      <ConfirmAction danger label="更新产品状态" summary={`提交 PATCH ${endpointPrefix}/:id/status`} onConfirm={(reason) => apiRequest(`${endpointPrefix}/${productId}/status`, { method: 'PATCH', body: JSON.stringify({ status, reason }) })} />
    </Card>
  </div>;
}
```

- [ ] **Step 5: Wire action pages into Admin routes**

Modify `web/src/admin/routes.tsx` to add imports:

```tsx
import { AgentManagementPage } from './actions/AgentManagementPage';
import { ConvertRuleActionsPage } from './actions/ConvertRuleActions';
import { MarketStrategyActionsPage } from './actions/MarketStrategyActions';
import { NewCoinActionsPage } from './actions/NewCoinActions';
import { ProductStatusPage } from './actions/ProductStatusActions';
```

Replace these route entries from Task 7:

```tsx
{ path: 'agents', element: <AgentManagementPage /> },
{ path: 'new-coins/projects', element: <><AdminResourcePage {...resourceConfigs.newCoinProjects} /><NewCoinActionsPage /></> },
{ path: 'market/strategies', element: <><AdminResourcePage {...resourceConfigs.marketStrategies} /><MarketStrategyActionsPage /></> },
{ path: 'convert/rules', element: <ConvertRuleActionsPage /> },
{ path: 'seconds-contract/products', element: <><AdminResourcePage {...resourceConfigs.secondsProducts} /><ProductStatusPage title="秒合约产品状态" description="启用或禁用秒合约产品" endpointPrefix="/admin/api/v1/seconds-contracts/products" /></> },
{ path: 'margin/products', element: <><AdminResourcePage {...resourceConfigs.marginProducts} /><ProductStatusPage title="杠杆产品状态" description="启用或禁用杠杆产品" endpointPrefix="/admin/api/v1/margin/products" /></> },
{ path: 'earn/products', element: <><AdminResourcePage {...resourceConfigs.earnProducts} /><ProductStatusPage title="Earn 产品状态" description="启用或禁用理财产品" endpointPrefix="/admin/api/v1/earn/products" /></> }
```

- [ ] **Step 6: Run typecheck**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run typecheck
```

Expected: PASS.

---

### Task 9: Frontend verification and production build

**Files:**
- Modify if needed: files created in previous tasks
- Modify: `docs/superpowers/PROGRESS.md`

- [ ] **Step 1: Run full frontend verification**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run typecheck && npm run lint && npm run test && npm run build
```

Expected: all commands exit 0. Test output should show all Vitest suites passing.

- [ ] **Step 2: Run local app smoke**

Run:

```bash
cd "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run dev
```

Open `http://127.0.0.1:5173/login`.

Expected:

- Login page displays Chinese title `交易所管理后台`.
- Admin identity is enabled.
- Agent identity is visible but disabled with `代理暂未开放`.
- Direct `/admin/dashboard` access without token redirects to `/login`.

- [ ] **Step 3: Record progress**

Append `docs/superpowers/PROGRESS.md` with:

```markdown
## 2026-05-30 HH:mm - Admin 后台前端首期实现

- 完成内容：创建 `/web` Vite React TypeScript + Semi Design Admin 后台；实现 Admin 登录、权限守卫、AdminLayout、菜单、共享表格/状态/时间/金额组件、审计日志等主要列表页入口；Agent 后台保持未开放。
- 修改文件：
  - `web/*`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm run typecheck && npm run lint && npm run test && npm run build`，通过；已运行本地 smoke，确认登录页、Admin 身份、Agent 禁用态、未登录 Admin 路由跳转符合预期。
- 剩余事项：Admin 首期页面已覆盖；更细的批量操作和端到端业务验收不属于本次首期范围。
```

Use the actual current timestamp and exact verification result.

---

## Self-Review Checklist

- Spec coverage: This plan implements `/web`, Vite, React, TypeScript, Semi Design, Admin login, `/admin/*`, Admin menu, Admin API-first list pages, shared state components, timestamp/Decimal display, Agent deferred boundary, and frontend verification commands.
- Endpoint coverage: All configured data pages use existing backend routes. Pages without Admin read APIs use `AdminNoticePage` and do not call user, agent, hardcoded project, or nonexistent endpoints.
- Placeholder scan: The plan contains no unresolved placeholder markers or sample write requests.
- Type consistency: `AuthSession`, `AuthScope`, `ApiError`, `ApiRecord`, `AdminResourcePage`, `AdminNoticePage`, `resourceConfigs`, action page names, and route paths are defined before use.
- Scope control: Agent backend UI, Agent routes, and `/agent/api/v1/*` calls are intentionally excluded from implementation.
