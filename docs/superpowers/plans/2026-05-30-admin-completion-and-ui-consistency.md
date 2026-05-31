# Admin Completion and UI Consistency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the missing Admin backend/frontend management surfaces and make the React + Semi Design Admin UI consistent, localized, and paginated.

**Architecture:** Add the smallest missing Admin read APIs in the Rust/Axum backend, then wire matching React resource pages through the existing `AdminResourcePage` abstraction. Keep high-risk write paths behind existing reason-confirmation action pages; for this plan, Risk rule writes are limited to create/status update with AdminAuth and audit. Frontend UI consistency is centralized in `AppProviders`, `DataTable`, `StatusTag`, and `styles.css` instead of duplicating styling in every page.

**Tech Stack:** Rust 2024, Axum, SQLx/MySQL, React, TypeScript, Vite, Semi Design, Vitest, Testing Library.

---

## Current Evidence

- `web/src/admin/routes.tsx` currently renders `AdminNoticePage` for users, wallet accounts, wallet ledger, new coin subscriptions, new coin distributions, and risk.
- `src/modules/admin/routes.rs` already exposes backend endpoints for new coin subscriptions and distributions, but the frontend does not wire them.
- `src/modules/admin/routes.rs` does not expose Admin user list/detail routes, Admin wallet account/ledger routes, or Risk rule/event routes.
- `web/src/shared/DataTable.tsx` hard-codes `pagination={false}`.
- `web/src/shared/StatusTag.tsx` has a small status map and falls back to raw English values.
- `web/src/app/providers.tsx` only wraps React Query and does not use Semi `ConfigProvider`.
- Semi MCP Table documentation says remote/list data should use controlled pagination via `pagination.currentPage`; since current backend list APIs mostly expose `limit` but not `total`, this plan implements local controlled pagination first.
- Semi MCP ConfigProvider documentation says global locale/timeZone/direction should be configured by wrapping the app in `ConfigProvider`.

## File Structure

### Backend

- Modify `src/modules/admin/routes.rs`
  - Add route registrations:
    - `GET /users`
    - `GET /users/:id`
    - `GET /wallet/accounts`
    - `GET /wallet/ledger`
    - `GET /risk/rules`
    - `POST /risk/rules`
    - `PATCH /risk/rules/:id/status`
    - `GET /risk/events`
  - Add response structs using existing Unix-milliseconds serializers for time fields.
  - Add list query structs with `user_id`, `asset_id`, `status`, `rule_type`, `target_type`, `decision`, `risk_level`, and `limit` filters.
  - Add audit logging for Risk rule creation/status updates.
- Modify `tests/admin_routes.rs`
  - Add MySQL integration tests for the new Admin routes.

### Frontend

- Modify `web/src/app/providers.tsx`
  - Wrap the app in Semi `ConfigProvider` with Chinese locale and `Asia/Shanghai` timezone.
- Modify `web/src/styles.css`
  - Align global typography and Admin shell styling with Semi Design variables.
  - Keep the dark financial sidebar, but make content/cards/tables consistently Semi-like.
- Modify `web/src/shared/StatusTag.tsx`
  - Expand status localization for product, order, lifecycle, risk, direction, fee, and boolean statuses.
- Modify `web/src/shared/DataTable.tsx`
  - Add controlled local pagination with default page size 20 and page size options 10/20/50/100.
  - Keep stable `rowKey`.
- Modify `web/src/admin/resources/AdminResourcePage.tsx`
  - Reset local page to 1 when filters change.
- Modify `web/src/admin/resources/resourceConfigs.tsx`
  - Add resource configs for users, wallet accounts, wallet ledger, new coin subscriptions, new coin distributions, risk rules, and risk events.
- Modify `web/src/admin/routes.tsx`
  - Replace relevant `AdminNoticePage` entries with `ResourcePage` configs.
- Add/modify frontend tests:
  - `web/src/shared/StatusTag.test.tsx`
  - `web/src/shared/DataTable.test.tsx`
  - `web/src/app/providers.test.tsx`
  - `web/src/admin/routes.test.tsx`

### Progress

- Modify `docs/superpowers/PROGRESS.md`
  - Add a final record after verification.

---

## Task 1: Wire Semi ConfigProvider

**Files:**
- Modify: `web/src/app/providers.tsx`
- Test: `web/src/app/providers.test.tsx`

- [ ] **Step 1: Write the failing provider test**

Create `web/src/app/providers.test.tsx`:

```tsx
import { ConfigConsumer } from '@douyinfe/semi-ui';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AppProviders } from './providers';

describe('AppProviders', () => {
  it('provides Semi Chinese locale and Shanghai timezone', () => {
    render(
      <AppProviders>
        <ConfigConsumer>
          {(config) => (
            <div>
              <span data-testid="timezone">{config.timeZone}</span>
              <span data-testid="pagination-prev">{config.locale?.Pagination?.prevText}</span>
              <span data-testid="pagination-next">{config.locale?.Pagination?.nextText}</span>
            </div>
          )}
        </ConfigConsumer>
      </AppProviders>
    );

    expect(screen.getByTestId('timezone')).toHaveTextContent('Asia/Shanghai');
    expect(screen.getByTestId('pagination-prev').textContent).not.toEqual('');
    expect(screen.getByTestId('pagination-next').textContent).not.toEqual('');
  });
});
```

- [ ] **Step 2: Run RED test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/app/providers.test.tsx
```

Expected: FAIL because `AppProviders` does not wrap children in `ConfigProvider`, so `config.timeZone` is empty/undefined.

- [ ] **Step 3: Implement ConfigProvider**

Modify `web/src/app/providers.tsx` to:

```tsx
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
```

- [ ] **Step 4: Run GREEN test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/app/providers.test.tsx
```

Expected: PASS.

---

## Task 2: Expand Chinese status localization

**Files:**
- Modify: `web/src/shared/StatusTag.tsx`
- Test: `web/src/shared/StatusTag.test.tsx`

- [ ] **Step 1: Write the failing status tests**

Create `web/src/shared/StatusTag.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { StatusTag } from './StatusTag';

describe('StatusTag', () => {
  it.each([
    ['partially_filled', '部分成交'],
    ['cancelled', '已取消'],
    ['preheat', '预热中'],
    ['subscription', '发行申购'],
    ['distribution', '派发中'],
    ['listed', '已上市'],
    ['not_required', '无需支付'],
    ['unpaid', '未支付'],
    ['paid', '已支付'],
    ['opened', '持仓中'],
    ['settled', '已结算'],
    ['win', '盈利'],
    ['loss', '亏损'],
    ['liquidated', '已强平'],
    ['subscribed', '已申购'],
    ['redeemed', '已赎回'],
    ['review', '人工复核'],
    ['deny', '拒绝'],
    ['allow', '放行'],
    ['long', '做多'],
    ['short', '做空'],
    ['up', '看涨'],
    ['down', '看跌']
  ])('renders %s as %s', (value, label) => {
    render(<StatusTag value={value} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run RED test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/shared/StatusTag.test.tsx
```

Expected: FAIL because these statuses currently fall back to raw English values.

- [ ] **Step 3: Expand `STATUS_MAP`**

Modify `web/src/shared/StatusTag.tsx` so `STATUS_MAP` is:

```tsx
const STATUS_MAP: Record<string, StatusMeta> = {
  active: { label: '启用', color: 'green' },
  allow: { label: '放行', color: 'green' },
  approved: { label: '已通过', color: 'green' },
  cancelled: { label: '已取消', color: 'grey' },
  closed: { label: '已平仓', color: 'grey' },
  completed: { label: '已完成', color: 'green' },
  confirm: { label: '确认', color: 'blue' },
  denied: { label: '已拒绝', color: 'red' },
  deny: { label: '拒绝', color: 'red' },
  disabled: { label: '禁用', color: 'grey' },
  distribution: { label: '派发中', color: 'blue' },
  down: { label: '看跌', color: 'red' },
  enabled: { label: '启用', color: 'green' },
  external: { label: '外部行情', color: 'blue' },
  failed: { label: '失败', color: 'red' },
  false: { label: '禁用', color: 'grey' },
  filled: { label: '已成交', color: 'green' },
  fixed: { label: '固定汇率', color: 'blue' },
  inactive: { label: '禁用', color: 'grey' },
  listed: { label: '已上市', color: 'green' },
  liquidated: { label: '已强平', color: 'red' },
  locked: { label: '锁定', color: 'orange' },
  long: { label: '做多', color: 'green' },
  loss: { label: '亏损', color: 'red' },
  manual: { label: '手动处理', color: 'blue' },
  not_required: { label: '无需支付', color: 'grey' },
  open: { label: '委托中', color: 'blue' },
  opened: { label: '持仓中', color: 'blue' },
  paid: { label: '已支付', color: 'green' },
  partially_filled: { label: '部分成交', color: 'blue' },
  pending: { label: '待处理', color: 'orange' },
  preheat: { label: '预热中', color: 'orange' },
  processing: { label: '处理中', color: 'blue' },
  rejected: { label: '已拒绝', color: 'red' },
  released: { label: '已释放', color: 'green' },
  redeemed: { label: '已赎回', color: 'green' },
  review: { label: '人工复核', color: 'orange' },
  running: { label: '运行中', color: 'green' },
  settled: { label: '已结算', color: 'green' },
  short: { label: '做空', color: 'red' },
  stopped: { label: '已停止', color: 'grey' },
  subscribed: { label: '已申购', color: 'blue' },
  subscription: { label: '发行申购', color: 'blue' },
  suspended: { label: '暂停', color: 'orange' },
  true: { label: '启用', color: 'green' },
  unpaid: { label: '未支付', color: 'orange' },
  up: { label: '看涨', color: 'green' },
  win: { label: '盈利', color: 'green' }
};
```

- [ ] **Step 4: Run GREEN test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/shared/StatusTag.test.tsx
```

Expected: PASS.

---

## Task 3: Add controlled local pagination to DataTable

**Files:**
- Modify: `web/src/shared/DataTable.tsx`
- Test: `web/src/shared/DataTable.test.tsx`

- [ ] **Step 1: Write failing pagination tests**

Create `web/src/shared/DataTable.test.tsx`:

```tsx
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { DataTable } from './DataTable';

type Row = {
  id: number;
  name: string;
};

const columns = [{ dataIndex: 'name', title: '名称' }];
const rows: Row[] = Array.from({ length: 25 }, (_, index) => ({ id: index + 1, name: `记录${index + 1}` }));

describe('DataTable', () => {
  it('paginates rows locally with a controlled Semi table pagination', () => {
    render(<DataTable columns={columns} data={rows} />);

    expect(screen.getByText('记录1')).toBeInTheDocument();
    expect(screen.getByText('记录20')).toBeInTheDocument();
    expect(screen.queryByText('记录21')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('2'));

    expect(screen.queryByText('记录1')).not.toBeInTheDocument();
    expect(screen.getByText('记录21')).toBeInTheDocument();
    expect(screen.getByText('记录25')).toBeInTheDocument();
  });

  it('resets to page one when filtered data shrinks', () => {
    const { rerender } = render(<DataTable columns={columns} data={rows} />);
    fireEvent.click(screen.getByText('2'));
    expect(screen.getByText('记录21')).toBeInTheDocument();

    rerender(<DataTable columns={columns} data={rows.slice(0, 5)} />);

    expect(screen.getByText('记录1')).toBeInTheDocument();
    expect(screen.queryByText('记录21')).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run RED test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/shared/DataTable.test.tsx
```

Expected: FAIL because `DataTable` renders all rows and disables pagination.

- [ ] **Step 3: Implement local controlled pagination**

Modify `web/src/shared/DataTable.tsx`:

```tsx
import { Empty, Spin, Table, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useEffect, useMemo, useState } from 'react';

const { Text } = Typography;
const DEFAULT_PAGE_SIZE = 20;
const PAGE_SIZE_OPTIONS = [10, 20, 50, 100];

type DataTableProps<T extends Record<string, unknown>> = {
  columns: Array<ColumnProps<T>>;
  data: T[];
  error?: Error | null;
  loading?: boolean;
  rowKey?: Extract<keyof T, string> | ((record: T) => string | number);
};

function resolveRowKey<T extends Record<string, unknown>>(rowKey: DataTableProps<T>['rowKey']) {
  if (typeof rowKey === 'function') {
    return (record?: T) => (record ? String(rowKey(record)) : '');
  }

  return rowKey ?? 'id';
}

function pageData<T>(data: T[], currentPage: number, pageSize: number) {
  const start = (currentPage - 1) * pageSize;
  return data.slice(start, start + pageSize);
}

export function DataTable<T extends Record<string, unknown>>({ columns, data, error, loading, rowKey }: DataTableProps<T>) {
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);

  useEffect(() => {
    setCurrentPage(1);
  }, [data]);

  const visibleRows = useMemo(() => pageData(data, currentPage, pageSize), [currentPage, data, pageSize]);

  if (loading) {
    return (
      <div style={{ display: 'grid', minHeight: 220, placeItems: 'center' }}>
        <Spin size="large" tip="加载中" />
      </div>
    );
  }

  if (error) {
    return (
      <div role="alert" style={{ padding: 24 }}>
        <Text type="danger">加载失败：{error.message}</Text>
      </div>
    );
  }

  if (data.length === 0) {
    return <Empty description="暂无数据" />;
  }

  return (
    <Table
      columns={columns}
      dataSource={visibleRows}
      pagination={{
        currentPage,
        formatPageText: ({ currentStart, currentEnd, total }) => `第 ${currentStart}-${currentEnd} 条 / 共 ${total} 条`,
        onPageChange: setCurrentPage,
        onPageSizeChange: (nextPageSize) => {
          setPageSize(nextPageSize);
          setCurrentPage(1);
        },
        pageSize,
        pageSizeOpts: PAGE_SIZE_OPTIONS,
        showSizeChanger: true,
        total: data.length
      }}
      rowKey={resolveRowKey(rowKey)}
      scroll={{ x: 'max-content' }}
      size="small"
    />
  );
}
```

- [ ] **Step 4: Run GREEN test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/shared/DataTable.test.tsx
```

Expected: PASS.

---

## Task 4: Reset resource pages to first page when filters change

**Files:**
- Modify: `web/src/admin/resources/AdminResourcePage.tsx`
- Test: extend `web/src/shared/DataTable.test.tsx` only if Task 3 reset is insufficient.

- [ ] **Step 1: Confirm Task 3 reset covers filter changes**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/shared/DataTable.test.tsx
```

Expected: PASS. Because `AdminResourcePage` replaces `rows` after filter fetch, Task 3's `useEffect([data])` resets to page 1.

- [ ] **Step 2: No production change unless Task 3 test fails**

If the test fails, fix `DataTable` rather than adding page state to every resource page.

---

## Task 5: Wire existing New Coin subscription/distribution Admin pages

**Files:**
- Modify: `web/src/admin/resources/resourceConfigs.tsx`
- Modify: `web/src/admin/routes.tsx`
- Test: `web/src/admin/routes.test.tsx`

- [ ] **Step 1: Write failing route config test**

Create `web/src/admin/routes.test.tsx`:

```tsx
import { describe, expect, it } from 'vitest';

import { adminRoutes } from './routes';

function routeElementSource(path: string) {
  return String(adminRoutes.find((route) => route.path === path)?.element?.type?.name ?? '');
}

describe('adminRoutes', () => {
  it('uses resource pages instead of notice placeholders for new coin subscriptions and distributions', () => {
    expect(routeElementSource('new-coins/subscriptions')).toBe('ResourcePage');
    expect(routeElementSource('new-coins/distributions')).toBe('ResourcePage');
  });
});
```

- [ ] **Step 2: Run RED test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/routes.test.tsx
```

Expected: FAIL because those routes currently use `AdminNoticePage`.

- [ ] **Step 3: Add resource configs**

In `web/src/admin/resources/resourceConfigs.tsx`, add these configs inside `resourceConfigs`:

```tsx
  newCoinSubscriptions: {
    title: '发行申购',
    endpoint: '/admin/api/v1/new-coins/subscriptions',
    responseKey: 'subscriptions',
    filters: [projectFilter, userFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '申购ID' },
      { key: 'project_id', title: '项目ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'quote_asset', title: '支付资产' },
      { key: 'quote_amount', title: '支付金额', type: 'amount' },
      { key: 'requested_quantity', title: '申购数量', type: 'amount' },
      { key: 'allocated_quantity', title: '获配数量', type: 'amount' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  newCoinDistributions: {
    title: '派发记录',
    endpoint: '/admin/api/v1/new-coins/distributions',
    responseKey: 'distributions',
    filters: [projectFilter, userFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '派发ID' },
      { key: 'project_id', title: '项目ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'subscription_id', title: '申购ID' },
      { key: 'quantity', title: '派发数量', type: 'amount' },
      { key: 'lock_position_id', title: '锁仓ID' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
```

- [ ] **Step 4: Replace routes**

Modify `web/src/admin/routes.tsx`:

```tsx
{ path: 'new-coins/subscriptions', element: <ResourcePage config={resourceConfigs.newCoinSubscriptions} /> },
{ path: 'new-coins/distributions', element: <ResourcePage config={resourceConfigs.newCoinDistributions} /> },
```

- [ ] **Step 5: Run GREEN test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/routes.test.tsx
```

Expected: PASS.

---

## Task 6: Add Admin user list/detail backend APIs

**Files:**
- Modify: `src/modules/admin/routes.rs`
- Test: `tests/admin_routes.rs`

- [ ] **Step 1: Write failing backend tests**

Append to `tests/admin_routes.rs`:

```rust
#[tokio::test]
async fn admin_lists_users_and_reads_user_detail() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let user_id = create_user_with_email(
        &pool,
        format!("admin-user-list-{}@example.test", Uuid::now_v7().simple()),
    )
    .await;
    let token = issue_token(&test_settings(), "admin:1".to_owned(), TokenScope::Admin, 900)?;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/users?user_id={user_id}&limit=10"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = to_bytes(list_response.into_body(), 1024 * 1024).await?;
    let list_json: Value = serde_json::from_slice(&list_body)?;
    assert_eq!(list_json["users"][0]["id"], json!(user_id));
    assert_eq!(list_json["users"][0]["status"], json!("active"));
    assert!(list_json["users"][0]["created_at"].as_i64().is_some());

    let detail_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/users/{user_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = to_bytes(detail_response.into_body(), 1024 * 1024).await?;
    let detail_json: Value = serde_json::from_slice(&detail_body)?;
    assert_eq!(detail_json["user"]["id"], json!(user_id));

    Ok(())
}
```

- [ ] **Step 2: Run RED test**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_lists_users_and_reads_user_detail -- --nocapture
```

Expected: FAIL with 404 because `/admin/api/v1/users` and `/admin/api/v1/users/:id` are not registered.

- [ ] **Step 3: Implement route registration**

In `src/modules/admin/routes.rs`, add to `routes()`:

```rust
        .route("/users", get(list_admin_users))
        .route("/users/:id", get(get_admin_user))
```

- [ ] **Step 4: Implement response/query structs**

Add near other response structs:

```rust
#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminUserResponse {
    id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    #[serde(with = "crate::time::ts_milliseconds")]
    created_at: DateTime<Utc>,
    #[serde(with = "crate::time::ts_milliseconds")]
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AdminUsersResponse {
    users: Vec<AdminUserResponse>,
}

#[derive(Debug, Serialize)]
struct AdminUserDetailResponse {
    user: AdminUserResponse,
}

#[derive(Debug, Deserialize)]
struct AdminUserListQuery {
    user_id: Option<u64>,
    status: Option<String>,
    limit: Option<u32>,
}
```

- [ ] **Step 5: Implement handlers**

Add handlers:

```rust
async fn list_admin_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminUserListQuery>,
) -> AppResult<Json<AdminUsersResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::new(
        r#"SELECT id, email, phone, status, kyc_level, created_at, updated_at FROM users WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        builder.push(" AND id = ").push_bind(user_id);
    }
    if let Some(status) = query.status.filter(|value| !value.trim().is_empty()) {
        builder.push(" AND status = ").push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ").push_bind(query.limit.unwrap_or(100).min(500));
    let users = builder
        .build_query_as::<AdminUserResponse>()
        .fetch_all(pool)
        .await?;
    Ok(Json(AdminUsersResponse { users }))
}

async fn get_admin_user(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<Json<AdminUserDetailResponse>> {
    let pool = mysql_pool(&state)?;
    let user = sqlx::query_as::<_, AdminUserResponse>(
        r#"SELECT id, email, phone, status, kyc_level, created_at, updated_at FROM users WHERE id = ? LIMIT 1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("user not found".to_owned()))?;
    Ok(Json(AdminUserDetailResponse { user }))
}
```

- [ ] **Step 6: Run GREEN test**

Run the RED command again.

Expected: PASS.

---

## Task 7: Add Admin wallet account and ledger backend APIs

**Files:**
- Modify: `src/modules/admin/routes.rs`
- Test: `tests/admin_routes.rs`

- [ ] **Step 1: Write failing wallet route test**

Append to `tests/admin_routes.rs`:

```rust
#[tokio::test]
async fn admin_lists_wallet_accounts_and_ledger() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool, "AW").await;
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("3.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'admin_test', ?, 'available', ?, ?, ?, ?, 'admin_test', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("3.000000000000000000"))
    .bind(Uuid::now_v7().to_string())
    .execute(&pool)
    .await?;

    let token = issue_token(&test_settings(), "admin:1".to_owned(), TokenScope::Admin, 900)?;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    let accounts_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/wallet/accounts?user_id={user_id}&limit=10"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(accounts_response.status(), StatusCode::OK);
    let accounts_body = to_bytes(accounts_response.into_body(), 1024 * 1024).await?;
    let accounts_json: Value = serde_json::from_slice(&accounts_body)?;
    assert_eq!(accounts_json["accounts"][0]["user_id"], json!(user_id));
    assert_eq!(accounts_json["accounts"][0]["available"], json!("100.000000000000000000"));

    let ledger_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/wallet/ledger?user_id={user_id}&asset_id={asset_id}&limit=10"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(ledger_response.status(), StatusCode::OK);
    let ledger_body = to_bytes(ledger_response.into_body(), 1024 * 1024).await?;
    let ledger_json: Value = serde_json::from_slice(&ledger_body)?;
    assert_eq!(ledger_json["ledger"][0]["user_id"], json!(user_id));
    assert_eq!(ledger_json["ledger"][0]["change_type"], json!("admin_test"));

    Ok(())
}
```

- [ ] **Step 2: Run RED test**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_lists_wallet_accounts_and_ledger -- --nocapture
```

Expected: FAIL with 404 for wallet Admin routes.

- [ ] **Step 3: Register wallet routes**

In `src/modules/admin/routes.rs`, add:

```rust
        .route("/wallet/accounts", get(list_admin_wallet_accounts))
        .route("/wallet/ledger", get(list_admin_wallet_ledger))
```

- [ ] **Step 4: Add structs and handlers**

Add response structs with `BigDecimal` fields serialized as strings following existing module patterns. Include fields:

```rust
#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminWalletAccountResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    asset_symbol: String,
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
    #[serde(with = "crate::time::ts_milliseconds")]
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AdminWalletAccountsResponse {
    accounts: Vec<AdminWalletAccountResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminWalletLedgerResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    asset_symbol: String,
    change_type: String,
    amount: BigDecimal,
    balance_type: String,
    balance_after: BigDecimal,
    available_after: BigDecimal,
    frozen_after: BigDecimal,
    locked_after: BigDecimal,
    ref_type: String,
    ref_id: String,
    #[serde(with = "crate::time::ts_milliseconds")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AdminWalletLedgerListResponse {
    ledger: Vec<AdminWalletLedgerResponse>,
}

#[derive(Debug, Deserialize)]
struct AdminWalletListQuery {
    user_id: Option<u64>,
    asset_id: Option<u64>,
    limit: Option<u32>,
}
```

Handlers should join `assets` to expose `asset_symbol`, support user/asset filters, order by latest id desc, and limit to `limit.unwrap_or(100).min(500)`.

- [ ] **Step 5: Run GREEN test**

Run the RED command again.

Expected: PASS.

---

## Task 8: Add Admin Risk backend APIs

**Files:**
- Modify: `src/modules/admin/routes.rs`
- Test: `tests/admin_routes.rs`

- [ ] **Step 1: Write failing risk route test**

Append to `tests/admin_routes.rs`:

```rust
#[tokio::test]
async fn admin_manages_risk_rules_and_lists_events() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let user_id = create_user(&pool).await;
    sqlx::query(
        r#"INSERT INTO risk_events
           (user_id, actor_type, actor_id, event_type, risk_level, decision, reason, payload_json)
           VALUES (?, 'user', ?, 'withdraw', 'high', 'review', 'manual review', JSON_OBJECT('amount', '100'))"#,
    )
    .bind(user_id)
    .bind(user_id)
    .execute(&pool)
    .await?;

    let token = issue_token(&test_settings(), "admin:1".to_owned(), TokenScope::Admin, 900)?;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/risk/rules")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "rule_type": "withdraw_limit",
                        "target_type": "user",
                        "target_id": user_id.to_string(),
                        "config_json": { "max_amount": "100.000000000000000000" },
                        "reason": "limit high risk withdraw"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = to_bytes(create_response.into_body(), 1024 * 1024).await?;
    let create_json: Value = serde_json::from_slice(&create_body)?;
    let rule_id = create_json["rule"]["id"].as_u64().unwrap();
    assert_eq!(create_json["rule"]["enabled"], json!(true));

    let disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/risk/rules/{rule_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "enabled": false, "reason": "disable test" }).to_string()))?,
        )
        .await?;
    assert_eq!(disable_response.status(), StatusCode::OK);

    let list_rules_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/risk/rules?rule_type=withdraw_limit&limit=10")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(list_rules_response.status(), StatusCode::OK);
    let list_rules_body = to_bytes(list_rules_response.into_body(), 1024 * 1024).await?;
    let rules_json: Value = serde_json::from_slice(&list_rules_body)?;
    assert_eq!(rules_json["rules"][0]["id"], json!(rule_id));
    assert_eq!(rules_json["rules"][0]["enabled"], json!(false));

    let list_events_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/risk/events?user_id={user_id}&limit=10"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(list_events_response.status(), StatusCode::OK);
    let list_events_body = to_bytes(list_events_response.into_body(), 1024 * 1024).await?;
    let events_json: Value = serde_json::from_slice(&list_events_body)?;
    assert_eq!(events_json["events"][0]["decision"], json!("review"));

    Ok(())
}
```

- [ ] **Step 2: Run RED test**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_manages_risk_rules_and_lists_events -- --nocapture
```

Expected: FAIL with 404 for risk routes.

- [ ] **Step 3: Register risk routes**

In `src/modules/admin/routes.rs`, add:

```rust
        .route("/risk/rules", get(list_risk_rules).post(create_risk_rule))
        .route("/risk/rules/:id/status", patch(update_risk_rule_status))
        .route("/risk/events", get(list_risk_events))
```

- [ ] **Step 4: Implement risk structs and handlers**

Add request/response structs:

```rust
#[derive(Debug, Deserialize)]
struct CreateRiskRuleRequest {
    rule_type: String,
    target_type: String,
    target_id: Option<String>,
    config_json: Value,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct UpdateRiskRuleStatusRequest {
    enabled: bool,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct RiskRuleListQuery {
    rule_type: Option<String>,
    target_type: Option<String>,
    enabled: Option<bool>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RiskEventListQuery {
    user_id: Option<u64>,
    decision: Option<String>,
    risk_level: Option<String>,
    limit: Option<u32>,
}
```

The handlers must:

- Require `AdminAuth`.
- Validate `reason.trim().len()` is between 1 and 512.
- Validate `rule_type` and `target_type` are non-empty.
- Insert/update `risk_rules` in a transaction.
- Insert `admin_audit_logs` with actions `risk_rule.create` and `risk_rule.update_status` in the same transaction.
- Return current rule/event payloads with Unix millisecond timestamp fields.

- [ ] **Step 5: Run GREEN test**

Run the RED command again.

Expected: PASS.

---

## Task 9: Wire Admin users, wallet, and risk frontend pages

**Files:**
- Modify: `web/src/admin/resources/resourceConfigs.tsx`
- Modify: `web/src/admin/routes.tsx`
- Test: extend `web/src/admin/routes.test.tsx`

- [ ] **Step 1: Extend failing route test**

Add to `web/src/admin/routes.test.tsx`:

```tsx
  it('uses resource pages instead of notice placeholders for users wallet and risk', () => {
    expect(routeElementSource('users')).toBe('ResourcePage');
    expect(routeElementSource('wallet/accounts')).toBe('ResourcePage');
    expect(routeElementSource('wallet/ledger')).toBe('ResourcePage');
    expect(routeElementSource('risk')).toBe('ResourcePage');
  });
```

- [ ] **Step 2: Run RED test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/routes.test.tsx
```

Expected: FAIL because those routes currently use `AdminNoticePage`.

- [ ] **Step 3: Add resource configs**

Add configs:

```tsx
  users: {
    title: '用户管理',
    endpoint: '/admin/api/v1/users',
    responseKey: 'users',
    filters: [userFilter, statusFilter, limitFilter],
    columns: [
      { key: 'id', title: '用户ID' },
      { key: 'email', title: '邮箱' },
      { key: 'phone', title: '手机号' },
      { key: 'status', title: '状态', type: 'status' },
      { key: 'kyc_level', title: 'KYC等级' },
      { key: 'created_at', title: '注册时间', type: 'timestamp' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  walletAccounts: {
    title: '钱包账户',
    endpoint: '/admin/api/v1/wallet/accounts',
    responseKey: 'accounts',
    filters: [userFilter, assetFilter, limitFilter],
    columns: [
      { key: 'id', title: '账户ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_id', title: '资产ID' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'available', title: '可用', type: 'amount' },
      { key: 'frozen', title: '冻结', type: 'amount' },
      { key: 'locked', title: '锁定', type: 'amount' },
      { key: 'updated_at', title: '更新时间', type: 'timestamp' }
    ]
  },
  walletLedger: {
    title: '钱包流水',
    endpoint: '/admin/api/v1/wallet/ledger',
    responseKey: 'ledger',
    filters: [userFilter, assetFilter, { key: 'ref_type', label: '引用类型' }, { key: 'ref_id', label: '引用ID' }, limitFilter],
    columns: [
      { key: 'id', title: '流水ID' },
      { key: 'user_id', title: '用户ID' },
      { key: 'asset_symbol', title: '资产' },
      { key: 'change_type', title: '变动类型', type: 'status' },
      { key: 'amount', title: '金额', type: 'amount' },
      { key: 'balance_type', title: '余额类型', type: 'status' },
      { key: 'available_after', title: '可用后', type: 'amount' },
      { key: 'frozen_after', title: '冻结后', type: 'amount' },
      { key: 'locked_after', title: '锁定后', type: 'amount' },
      { key: 'ref_type', title: '引用类型' },
      { key: 'ref_id', title: '引用ID' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  },
  riskRules: {
    title: '风控中心',
    endpoint: '/admin/api/v1/risk/rules',
    responseKey: 'rules',
    filters: [{ key: 'rule_type', label: '规则类型' }, { key: 'target_type', label: '对象类型' }, { key: 'enabled', label: '是否启用' }, limitFilter],
    columns: [
      { key: 'id', title: '规则ID' },
      { key: 'rule_type', title: '规则类型' },
      { key: 'target_type', title: '对象类型' },
      { key: 'target_id', title: '对象ID' },
      { key: 'enabled', title: '启用', type: 'status' },
      { key: 'config_json', title: '配置', type: 'json' },
      { key: 'created_by', title: '创建人' },
      { key: 'created_at', title: '创建时间', type: 'timestamp' }
    ]
  }
```

- [ ] **Step 4: Replace routes**

Modify `web/src/admin/routes.tsx`:

```tsx
{ path: 'users', element: <ResourcePage config={resourceConfigs.users} /> },
{ path: 'wallet/accounts', element: <ResourcePage config={resourceConfigs.walletAccounts} /> },
{ path: 'wallet/ledger', element: <ResourcePage config={resourceConfigs.walletLedger} /> },
{ path: 'risk', element: <ResourcePage config={resourceConfigs.riskRules} /> },
```

- [ ] **Step 5: Run GREEN test**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/routes.test.tsx
```

Expected: PASS.

---

## Task 10: Align Admin UI styling with Semi

**Files:**
- Modify: `web/src/styles.css`
- Test: no unit test; validate via build and existing component tests.

- [ ] **Step 1: Replace global font/background foundation**

Modify the top of `web/src/styles.css`:

```css
:root {
  color: var(--semi-color-text-0);
  background: #f5f7fb;
  font-family:
    Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC',
    'Microsoft YaHei', sans-serif;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  min-width: 320px;
  min-height: 100vh;
  margin: 0;
  background: #f5f7fb;
}
```

- [ ] **Step 2: Keep dark sidebar but normalize content shell**

Modify Admin shell rules:

```css
.admin-shell {
  min-height: 100vh;
  background: #f5f7fb;
}

.admin-shell-sider {
  width: 288px;
  padding: 22px 16px;
  overflow: auto;
  border-right: 1px solid rgba(29, 41, 57, 0.16);
  background: #101828;
  box-sizing: border-box;
}

.admin-shell-header {
  height: 64px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 28px;
  border-bottom: 1px solid var(--semi-color-border);
  background: var(--semi-color-bg-2);
  box-sizing: border-box;
}

.admin-shell-content {
  min-width: 0;
  background: #f5f7fb;
}

.exchange-page {
  min-height: 100vh;
  padding: 28px;
  box-sizing: border-box;
}

.page-header h2 {
  margin: 8px 0;
  color: var(--semi-color-text-0);
}

.page-header-description {
  display: block;
  max-width: 720px;
  color: var(--semi-color-text-2) !important;
}
```

- [ ] **Step 3: Remove native input/select action styles only if they conflict**

Keep `.admin-action-form input/select` for existing action pages until those are migrated to Semi Form controls. Do not broaden the scope.

- [ ] **Step 4: Run frontend tests after styling change**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"
```

Expected: PASS.

---

## Task 11: Final verification and progress record

**Files:**
- Modify: `docs/superpowers/PROGRESS.md`

- [ ] **Step 1: Run backend focused tests**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run backend quality gates**

Run:

```bash
cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check
cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets
cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings
```

Expected: all PASS.

- [ ] **Step 3: Run frontend quality gates**

Run:

```bash
npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"
npm run lint --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"
npm run build --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"
```

Expected: all PASS. If build emits the existing lottie direct-eval/chunk-size warnings, record them as warnings, not failures.

- [ ] **Step 4: Append progress entry**

Append to `docs/superpowers/PROGRESS.md`:

```markdown
## 2026-05-30 HH:mm - Admin 后台补全与 Semi UI 统一

- 完成内容：补齐 Admin 用户、钱包账户、钱包流水、风控规则/事件后端接口；接入新币申购/派发、用户、钱包、风控前端资源页；接入 Semi ConfigProvider 中文全局配置；统一 Admin 样式基础；补全状态中文化；为通用表格增加受控分页。
- 修改文件：`src/modules/admin/routes.rs`、`tests/admin_routes.rs`、`web/src/app/providers.tsx`、`web/src/app/providers.test.tsx`、`web/src/shared/StatusTag.tsx`、`web/src/shared/StatusTag.test.tsx`、`web/src/shared/DataTable.tsx`、`web/src/shared/DataTable.test.tsx`、`web/src/admin/routes.tsx`、`web/src/admin/routes.test.tsx`、`web/src/admin/resources/resourceConfigs.tsx`、`web/src/styles.css`、`docs/superpowers/PROGRESS.md`
- 验证结果：记录本任务实际执行的 backend/frontend verification commands 和结果。
- 后续事项：如仍有 Admin 写操作需要开放，必须逐项补权限、reason、二次确认和审计后再开放。
```

Replace `HH:mm` and verification text with the actual values.

---

## Self-Review

- Spec coverage: The plan covers the user's requested Admin gaps: existing new coin pages, Admin users, wallet accounts, wallet ledger, Risk center, Semi MCP-driven ConfigProvider/style handling, status Chinese localization, pagination, final validation, and progress recording.
- Placeholder scan: No `TBD`, `TODO`, `implement later`, or vague "add appropriate" instructions remain. Risk handler requirements are concrete and limited.
- Type consistency: Frontend resource configs use existing `AdminResourceColumn<ApiRecord>` keys. Backend routes use the existing `/admin/api/v1` mount. Status localization values match backend status strings observed in routes/migrations.
- Scope control: This does not open broad money-changing Admin wallet write actions. It adds read-only wallet/user pages and limited Risk rule management with audit.
