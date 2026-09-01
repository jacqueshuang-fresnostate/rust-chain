# Admin Resource Response Contract

## 1. Scope / Trigger

Apply this contract when adding or changing an `AdminResourcePage` column, an
Admin list response DTO, or `listAdminResource` row validation. It separates
fields carried by the backend response from values computed only for display,
so strict DTO validation does not reject valid rows before derived rendering
can run.

## 2. Signatures

```ts
type AdminResourceColumnSource = 'api' | 'derived';

type AdminResourceColumn<T extends ApiRecord> = {
  key: Extract<keyof T, string>;
  source?: AdminResourceColumnSource;
  type?: 'amount' | 'json' | 'status' | 'text' | 'timestamp';
  render?: (record: T) => ReactNode;
  // Other presentation metadata is omitted here.
};

type AdminResourceRowContract = {
  requiredFields?: readonly string[];
  decimalFields?: readonly string[];
};

buildAdminResourceRowContract<T extends ApiRecord>(
  columns: Array<AdminResourceColumn<T>>,
): AdminResourceRowContract;
```

`source` defaults to `api`. A derived column must opt in explicitly with
`source: 'derived'`.

## 3. Contracts

- API columns are backend DTO fields. Their keys enter `requiredFields`, and
  amount-type API columns also enter `decimalFields`.
- Field presence and value validity are separate checks. A required key may be
  explicitly `null` when its backend contract is nullable; Decimal validation
  applies to every non-null value.
- Derived columns are calculated by `render(record)` from one or more actual
  response fields. Their synthetic keys enter neither row-contract list.
- `listAdminResource` remains fail-closed for missing API fields and malformed
  Decimal text; callers must not weaken it to accommodate a presentation-only
  key.
- `orderNoColumn()` is derived because some resources persist `order_no` while
  others generate the displayed business number from timestamp and ID. The
  formatter still prefers a nonblank backend `order_no` or `orderNo` when it is
  present.
- Related order columns whose keys identify real response fields, including
  `buy_order_id`, `sell_order_id`, and `subscription_id`, remain API columns
  even when their render function formats those IDs as business numbers.
- Endpoint-specific required-field allowlists are forbidden. Column source is
  a structural property and must be declared by the reusable column factory.

## 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| API column key is missing from a row | Throw a `ContractError` naming the endpoint, row, and field |
| API amount column contains a number or invalid Decimal text | Throw a `ContractError`; never accept rounded JavaScript numbers |
| Derived column key is absent | Accept the row and let its renderer compute the display value |
| Backend supplies a nonblank `order_no` | Display the backend value |
| Backend omits `order_no` for a derived business-number column | Display the deterministic prefix/time/ID fallback |
| A new column omits `source` | Treat it as an API field and validate it strictly |

## 5. Good / Base / Bad Cases

- **Good**: a seconds-contract row has `id`, `created_at`, and amount fields but
  no `order_no`; it passes the wire contract and renders an `SC...` number.
- **Base**: a prediction row includes a real `order_no`; the same formatter
  displays that value without synthesizing another number.
- **Bad**: every table key is copied into `requiredFields`; a valid earn
  subscription fails before its `EA...` display number can be rendered.
- **Bad**: `order_no` is removed globally from validation by endpoint name;
  future API-backed columns silently bypass the response contract.

## 6. Tests Required

- Unit-test `buildAdminResourceRowContract`: default/API fields remain required,
  API amount fields remain required Decimal fields, and derived fields are
  excluded from both lists.
- Unit-test `listAdminResource` separately to prove missing required fields and
  non-string/invalid Decimal values still fail closed.
- Assert every resource using `orderNoColumn()` receives
  `source: 'derived'`, including seconds-contract orders and earn
  subscriptions.
- Assert the business-number formatter prefers an existing backend order number
  and generates a stable fallback when it is absent.
- Run the Admin quality gate from `.trellis/spec/admin/index.md`.

## 7. Wrong vs Correct

### Wrong

```ts
const rowContract = {
  requiredFields: columns.map((column) => column.key),
};

const orderNumber = {
  key: 'order_no',
  render: (row) => formatBusinessOrderNo('EA', row),
};
```

The synthetic display key is now incorrectly required from every backend row.

### Correct

```ts
const orderNumber = {
  key: 'order_no',
  source: 'derived',
  render: (row) => formatBusinessOrderNo('EA', row),
};

const rowContract = buildAdminResourceRowContract(columns);
```

The presentation fallback can run while genuine response fields retain strict
validation.
