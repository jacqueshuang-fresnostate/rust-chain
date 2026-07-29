# Loan Product Contracts

## Scenario: Admin Product Filtering

### 1. Scope / Trigger

- Trigger: changing `GET /admin/api/v1/loan/products`, its query DTO, SQL predicates, pagination, or admin filters.
- Applies to the Axum query DTO, loan application validation, SQLx row/count queries, and the admin resource table.

### 2. Signatures

- API: `GET /admin/api/v1/loan/products?loan_type={value}&status={value}&limit={n}&offset={n}`.
- Optional filters: `loan_type`, `status`.
- Response: `{ products: LoanProductResponse[], total: number }`.

### 3. Contracts

- `loan_type` and `status` are optional exact-match filters.
- Leading and trailing whitespace is removed; an empty normalized value means no filter.
- Non-empty values reuse the same validators as product create/update.
- Row selection and `COUNT(*)` must use the same predicate builder so `total` describes the filtered result set.
- Combined filters use AND semantics.
- The public `/api/v1/loan/products` route remains independent and returns active products only.

### 4. Validation & Error Matrix

- Unsupported non-empty `loan_type` -> `400 VALIDATION_ERROR` before SQL execution.
- Unsupported non-empty `status` -> `400 VALIDATION_ERROR` before SQL execution.
- Blank filter -> ignored.
- Valid filter with no matches -> `200` with `products = []` and `total = 0`.

### 5. Good/Base/Bad Cases

- Good: `loan_type=credit&status=disabled` filters both rows and total with identical predicates.
- Base: missing or blank filters preserve the complete admin list.
- Bad: filtering only the row query while counting every product produces broken pagination.
- Bad: accepting arbitrary strings silently returns an empty list and hides operator mistakes.

### 6. Tests Required

- Route integration tests cover each filter, their AND combination, whitespace normalization, blank values, matching `total`, and public active-only behavior.
- A no-database validation test proves unsupported enums fail before SQL acquisition.
- Backend format and all-target compilation must pass.

### 7. Wrong vs Correct

#### Wrong

```rust
let rows = list_products(loan_type, status).await?;
let total = count_all_products().await?;
```

#### Correct

```rust
for builder in [&mut rows, &mut total] {
    push_loan_product_filters(builder, loan_type, status);
}
```
