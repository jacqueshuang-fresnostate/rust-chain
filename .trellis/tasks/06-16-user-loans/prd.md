# 用户贷款申请与后台配置

## Goal

Add a dedicated loan feature so admins can configure both credit-loan and collateral-loan offerings and review user loan applications, while PC users can apply for platform loans and view their loan orders. This module must be separate from margin-position borrowing, which is internal to leveraged trading.

## What I Already Know

* The user wants the admin backend to configure user loan functionality.
* Users should be able to apply to the platform for loans.
* PC already has `/loan` and `/user/loan-orders` routes, but both pages currently show unavailable placeholder states.
* `pc/src/api/loan.ts` is a stub that returns "backend unavailable" instead of calling real APIs.
* Existing `earn` routes provide the closest backend pattern for product configuration, user order creation, admin listing, and wallet ledger writes.
* Existing admin ResourcePage configuration is the closest admin frontend pattern for list pages, filters, row actions, and SideSheet details.
* Wallet ledger integration must use explicit `change_type`, `balance_type`, and `ref_type` values so both PC and admin transaction records can display loan flows consistently.
* User selected option 3: loan products must support both credit loans without collateral and collateralized loans with pledged assets.
* User selected manual collateralized MVP: users submit collateral asset and amount, the system freezes collateral, admin reviews, approval disburses funds, repayment releases collateral.
* User selected one-time repayment: users repay principal plus interest in a single payment; partial repayment and installment repayment are out of scope for MVP.
* User selected product-level early repayment interest mode: each loan product can choose full-term interest or actual-days interest.
* User selected product-level eligibility: each loan product configures its own minimum KYC level.

## Assumptions (Temporary)

* Loan products will be configured by type, loan asset, term, interest rate, early repayment interest mode, minimum KYC level, min/max amount, status, and display metadata.
* User loan applications will require admin approval before funds are credited.
* Approved/disbursed loans will credit the user's available wallet balance.
* Repayment will debit the user's available wallet balance and write wallet ledger entries.
* Admin review should show user email and asset symbol, not raw user/asset IDs as primary visible fields.
* PC loan pages should reuse existing i18n patterns and call real `/api/v1/loan/*` APIs.

## Open Questions

* None currently. Requirements are ready for final confirmation.

## Requirements (Evolving)

* Add backend loan persistence with Chinese column comments in migration files.
* Loan products must have a `loan_type` equivalent with at least `credit` and `collateralized`.
* Credit-loan products allow application without pledged assets.
* Collateralized-loan products require collateral asset and collateral amount information during application.
* Collateralized-loan applications freeze the user's collateral asset at application time.
* Cancelling a pending collateralized application releases frozen collateral.
* Rejecting a collateralized application releases frozen collateral.
* Repaying a disbursed collateralized loan releases frozen collateral.
* MVP collateral handling is manual-review based and does not perform automated LTV checks, margin calls, or liquidation.
* MVP repayment supports one-time repayment only: principal plus calculated interest is paid in a single wallet debit.
* Partial repayment and installment schedules are not supported in this task.
* Loan products must include an interest calculation mode for early repayment:
  * `full_term`: early repayment still charges the full configured term interest.
  * `actual_days`: early repayment charges by actual elapsed days, with at least one day charged after disbursement.
* Loan product names must support multiple countries and languages, using each selected country's default locale in admin configuration.
* Backend loan product APIs must persist and return `name_json` with `version`, `default_locale`, and `items(locale,country,title)`, while keeping `name` as the default/fallback name.
* PC loan product and loan order pages must display the localized loan product name for the current UI locale, then fall back to the configured default locale and finally `name`.
* Loan products must include `min_kyc_level`; only users whose current KYC level is greater than or equal to the product requirement can apply.
* PC must display/disable product application when the current user does not meet the configured KYC level.
* Backend application APIs must enforce `min_kyc_level` regardless of PC-side UI state.
* Loan orders must snapshot `interest_calculation_mode`, `interest_rate`, `term_days`, and fee/amount terms at application or approval time so later product edits do not change existing repayment amounts.
* Add user APIs for listing active loan products, creating applications, listing orders, viewing details, cancelling pending applications, and repayment when applicable.
* Add admin APIs for creating/updating loan products, enabling/disabling products, listing applications/orders, and approving/rejecting applications.
* Add wallet ledger entries for loan disbursement and repayment.
* Add admin pages for loan product configuration and loan application/order review using existing Semi UI admin ResourcePage patterns.
* Add PC Loan and LoanOrders pages that connect to the real backend API and are fully i18n-ready.
* Keep margin trading borrowed amount behavior unchanged.

## Acceptance Criteria (Evolving)

* [ ] Admin can create, edit, enable, and disable loan products.
* [ ] Admin can configure loan product names for multiple countries/languages.
* [ ] Admin can choose whether a loan product is credit-loan or collateralized-loan.
* [ ] Admin can configure each product's minimum KYC level.
* [ ] PC users can see active loan products and submit a loan application within configured limits.
* [ ] PC blocks or disables applications when the user does not meet the product's minimum KYC level.
* [ ] Backend rejects loan applications from users whose KYC level is below the product requirement.
* [ ] PC users can submit credit-loan applications without collateral.
* [ ] PC users can submit collateralized-loan applications with collateral asset and amount.
* [ ] Submitting a collateralized-loan application freezes collateral once and writes a wallet ledger entry.
* [ ] Admin can approve or reject a pending application.
* [ ] Rejecting or cancelling a pending collateralized-loan application releases collateral once.
* [ ] Approving an application credits the user's wallet once and writes a loan disbursement ledger entry.
* [ ] Users can see loan orders in `/user/loan-orders`.
* [ ] PC loan product and loan order displays use the localized product name when `name_json` contains the current locale.
* [ ] One-time repayment debits principal plus interest once, writes a loan repayment ledger entry, and releases collateral for collateralized orders.
* [ ] Early repayment interest follows the product's snapshotted interest calculation mode.
* [ ] Editing a loan product does not alter repayment terms for existing loan orders.
* [ ] Partial repayment and installment endpoints/actions are not exposed in PC or admin UI.
* [ ] Admin loan tables show user email and asset symbol instead of making raw IDs the main visible fields.
* [ ] Existing margin borrowing, earn products, and wallet flows keep working.
* [ ] Backend, admin frontend, and PC frontend validations pass for touched areas.

## Definition of Done

* Tests added or updated for the backend loan routes and wallet ledger behavior.
* Admin frontend tests updated for new resource config/menu behavior.
* PC frontend adapter/page tests updated where existing test patterns apply.
* `cargo fmt`, focused backend tests, and relevant frontend type/tests pass.
* `docs/superpowers/PROGRESS.md` is updated with the task slice result.

## Out of Scope (Explicit Until Confirmed)

* Automatic risk scoring or automated credit limits.
* Automated collateral LTV monitoring, margin calls, or liquidation.
* Partial repayment.
* Installment schedules and per-period bills.
* Overdue penalty worker.
* External lending provider integration.
* Reworking margin-position borrowing.

## Technical Approach (Draft)

Data flow:

```text
Admin loan product config
  -> active product list API
  -> PC loan application
  -> freeze collateral when loan_type = collateralized
  -> loan order pending
  -> admin approve/reject
  -> wallet credit on approval
  -> one-time principal plus interest repayment
  -> release collateral when repaid/rejected/cancelled
  -> wallet debit on repayment
  -> admin/PC transaction displays
```

Loan product naming:

```text
Admin country selection
  -> country default locale
  -> loan_products.name_json
  -> backend response includes name_json and fallback name
  -> PC picks current locale, default_locale, first title, then name
```

Draft tables:

* `loan_products`: admin-configured loan product/rule.
* `loan_orders`: user application and lifecycle state snapshot.
* MVP collateral fields can live on `loan_orders` because collateral is a single manually reviewed pledge per order.
* `loan_orders` snapshots product terms, including `interest_calculation_mode` and `min_kyc_level`, to keep review and repayment deterministic.

Draft loan types:

* `credit`: no collateral required.
* `collateralized`: collateral asset and amount required.

Draft interest calculation modes:

* `full_term`: interest = principal * rate for the entire configured term.
* `actual_days`: interest = principal * rate * charged_days / term_days, where charged days are derived from disbursement-to-repayment elapsed time and clamped to at least one day.

Draft statuses:

* Product: `active`, `disabled`.
* Order: `pending`, `approved`, `rejected`, `disbursed`, `repaid`, `cancelled`.

Draft wallet ledger values:

* Disbursement: `change_type = loan_disbursement`, `ref_type = loan_order`.
* Repayment: `change_type = loan_repayment`, `ref_type = loan_order`.
* Collateral freeze/release: `loan_collateral_freeze`, `loan_collateral_release`, `ref_type = loan_order`.

## Decision (ADR-lite)

**Context**: The loan product can be modeled as either a simple credit application, a collateralized product, or a shared product model supporting both.

**Decision**: Support both `credit` and `collateralized` loan product types.

**Consequences**: The backend schema, admin forms, PC application form, and wallet ledger labels must carry loan type explicitly. Collateralized products require additional collateral fields and wallet freezing/release behavior, while credit products skip collateral fields.

**Collateralized MVP Decision**: Use manual-review collateral handling. The system freezes the submitted collateral asset/amount at application time, releases it on pending cancellation or admin rejection, and releases it after repayment. Automated LTV monitoring and liquidation are out of scope for this task.

**Repayment MVP Decision**: Use one-time repayment only. The user repays principal plus calculated interest in a single wallet debit; partial repayment and installment repayment are excluded from this task.

**Interest Calculation Decision**: Configure early repayment interest at the product level. Products can use `full_term` or `actual_days`, and each order snapshots the selected mode and numeric terms for stable repayment.

**Eligibility Decision**: Configure minimum KYC level at the loan product level. PC should prevent users below the requirement from applying, and the backend must enforce the same rule.

## Expansion Sweep

Future evolution:

* The module may later need collateral, overdue penalties, risk levels, and partial repayment.
* Product config should leave room for future fee/penalty fields without coupling to margin trading.

Related scenarios:

* Admin list/detail display should follow current compact ResourcePage/Semi patterns.
* Wallet ledger and PC transaction i18n should include loan transaction types.

Failure and edge cases:

* Approval must be idempotent so repeated admin clicks cannot double-credit the wallet.
* Repayment must be idempotent and reject insufficient available balance.
* Disabled products should not accept new applications, but existing orders must remain visible.

## Technical Notes

* Existing PC placeholders:
  * `pc/src/views/Loan.vue`
  * `pc/src/views/User/LoanOrders.vue`
  * `pc/src/api/loan.ts`
* Existing backend pattern:
  * `src/modules/earn/routes.rs`
  * `migrations/0023_earn_products.sql`
  * `migrations/0065_earn_product_fee_config.sql`
* Existing admin pattern:
  * `web/src/admin/resources/resourceConfigs.tsx`
  * `web/src/admin/routes.tsx`
  * `web/src/layouts/AdminLayout.tsx`
* Existing wallet ledger helpers/patterns:
  * `src/modules/admin/routes.rs`
  * `src/modules/wallet/mod.rs`
  * `src/modules/wallet/routes.rs`
