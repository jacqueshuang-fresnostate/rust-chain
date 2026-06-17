# Order Identifier Display Contract

## Scenario: User-visible order numbers

### 1. Scope / Trigger

- Trigger: any PC or admin UI that labels a value as an order number, subscription number, buy order number, or sell order number.
- Internal database primary keys may remain in API payloads and route paths for actions such as detail, cancel, approve, repay, or redeem.
- User-visible order labels must not render the raw internal `id` as the displayed order number.

### 2. Signatures

- Preferred backend response field: `order_no: string`.
- PC adapter field: `orderNo: string`.
- Admin table display helper: `formatBusinessOrderNo(prefix, record)`.
- Internal route fields remain unchanged: `/orders/:id`, `/subscriptions/:id`, and related action routes still use internal IDs unless a backend route explicitly supports `order_no`.

### 3. Contracts

- If an API response includes `order_no`, PC/admin UI must display that value.
- If `order_no` is absent, UI may generate a stable display number from business prefix, date-like timestamp, and a non-raw token derived from `id`.
- Generated order numbers are display-only. They must not be submitted to existing ID-based action endpoints.
- Current business prefixes:
  - `LN`: loan orders
  - `SP`: spot orders and spot trade buy/sell order references
  - `SC`: seconds contract orders
  - `CV`: convert orders
  - `EA`: earn subscriptions
  - `NC`: new-coin subscriptions
  - `NP`: new-coin post-listing purchases
  - `PM`: prediction market orders

### 4. Validation & Error Matrix

- Missing `order_no` and missing `id` -> display a prefix plus zero token fallback, never the literal string `undefined`.
- Existing `order_no` is blank -> ignore it and generate the fallback display number.
- Action endpoints still require internal `id`; do not pass generated display order numbers to ID path routes.

### 5. Good/Base/Bad Cases

- Good: PC finance order card renders `order.orderNo`, where `orderNo` prefers backend `order_no`.
- Base: admin convert order table renders `formatBusinessOrderNo('CV', record)` while row actions still call `/convert/orders/${record.id}`.
- Bad: `<span>订单号: {record.id}</span>`.

### 6. Tests Required

- PC adapter tests assert mapped order rows include `orderNo` and that `orderNo !== String(id)`.
- Admin resource config tests assert order-like resources use `order_no` display columns.
- Detail drawer tests assert order reference fields such as `buy_order_id` render generated order numbers rather than raw IDs.
- Type checks must pass for both `pc` and `web`.

### 7. Wrong vs Correct

#### Wrong

```tsx
{ key: 'id', title: '订单ID' }
```

```vue
{{ $t('ai_finance.order_sn') }}: {{ order.id }}
```

#### Correct

```tsx
orderNoColumn('CV')
```

```vue
{{ $t('ai_finance.order_sn') }}: {{ order.orderNo }}
```
