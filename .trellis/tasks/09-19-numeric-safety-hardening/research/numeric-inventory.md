# Numeric Surface Inventory

The unit of review is a numeric boundary, not every occurrence of a numeral.
The linked slice reports list the inspected production paths, permitted
approximations, changed files and actual verification evidence.

| Numeric role | Canonical representation and protection | Evidence |
| --- | --- | --- |
| Financial source inputs | Exact decimal strings; bounded BigDecimal parsing; reject effective excess precision, huge exponents and oversized values | N01, N02, N03 |
| Stored money | Actual schema envelope, normally 38,18; Quick Recharge 36,18; asset precision may be stricter | N04, N05, N06 |
| Rates/leverage | Exact text; actual narrow schema, generally 18,8, plus existing business range | N03, N05, N06 |
| Financial request collections | Each element uses the bounded scalar parser; absence/null/empty-list semantics preserved | N01; 160-field AST guard |
| Generated settlement values | Product-specific quantization once; one value reused by order, wallet, ledger and journal | N04, N05, N06 |
| Historical balances and reserves | Exact original values; final capacity check; no independent bucket rescaling or historical dust erasure | N04, N05, N06 |
| Interest carry | Exact forward-only 26-fraction remainder and atomic hour checkpoint; no historical reconstruction | N05; migration 0141 |
| Aggregate money | Exact decimal arithmetic and asset-scoped aggregation; aggregate read capacity may exceed a single-row envelope | N02, N03, N06 |
| Financial formatting | Exact confirmation source; nonzero tiny values never presented as free/zero; ordinary summaries may round explicitly | N02, N03 |
| IDs | Existing safe numeric or exact string contract; reject unsafe identities before mutation | N02, N03, N04 |
| Counts/pagination | Integer lexical/range validation, safe conversion and existing page caps | N02, N03, N06 |
| Timestamps/TTL | Safe integer wire timestamps; checked multiplication/addition; generated deadlines fit actual storage | N01, N02, N03, N05, N06 |
| UI/charts/geometry | Finite bounded Number is permitted for coordinates, ratios, widths and non-authoritative projections | N02, N03 |
| Dynamic policy JSON | Field-specific exact decimal parsing; aggregate thresholds are not blindly forced into wallet storage precision | N03, N06 |

## Coverage and Limits

- Typed DTO inventory uses a Rust AST guard rather than a count of grep hits.
  It is complemented by behavioral tests for actual adapters/forms and
  isolated database tests for the financial lifecycle.
- New invalid inputs fail before funds are changed. A derived balance overflow
  aborts the transaction; existing successful replay is not newly rounded.
- An unsafe numeric value already parsed by JavaScript cannot be repaired with
  String(number). Authoritative monetary fields require string input;
  generic finite-number transport checks alone are insufficient.
- Read-only totals without an asset dimension are not presented as meaningful
  money. This does not introduce a new currency-conversion reporting policy.
- No claim is made that every possible future JSON shape, native timer option,
  historical production value, external payment callback, or financial
  terminal-overflow permutation has been executed. The slice reports identify
  tested paths separately from inspected/common-guard paths.
- Large-ID API migration, post-2038 schema extension, native iOS/Android runtime
  testing and production MySQL 8.4 rehearsal remain separate work.
- The existing spot account-bootstrap lock inversion found during parallel
  testing is documented in the release checklist, not masked as a precision fix.
