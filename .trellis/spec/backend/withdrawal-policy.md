# Configurable Withdrawal Policy

## Scope And Defaults

Migration `0134_withdrawal_policy.sql` adds per-asset policy, authenticated
address registrations, immutable request policy receipts, and independent
review votes. No policy or active financial threshold is seeded.
Missing policy is disabled at revision zero; the legacy one-review contract
remains. This is not a custody, sanctions-screening or full address-revocation
system.

## Configuration

- Admin saves require `system.security.write`, expected revision and reason.
  Configuration and Admin audit commit together.
- Explicit amount basis is principal or total reserved (including fee).
  Every matching user/KYC allowance applies independently; user-specific rules
  cannot override a stricter matching public rule.
- Windows are rolling seconds, not calendar days. Select either all outstanding
  requests or requests created within the window for pending occupancy.
- Precision must match the asset; no float arithmetic, inferred exchange rate,
  truncation or implicit monetary defaults.
- Review tiers begin at zero, have increasing inclusive lower bounds and
  nondecreasing counts of distinct reviewers. Never infer zero-review approval.

## Admission And Review

- Creation and policy updates serialize on the asset lock. Aggregate reads
  follow that lock. Policy receipt, quote consumption, wallet freeze, ledger
  and request insertion share the transaction.
- Exact successful request replay precedes new policy/rate-limit consumption.
- Pending, approved, broadcasting, unknown-outcome and manual-review requests
  occupy allowance according to the configured mode. Confirmed requests count
  within the creation-time window. Rejection or safe failure releases occupancy
  via existing status changes, not another refund command.
- The request freezes its required reviewer count. An administrator can vote
  only once. Insufficient distinct votes retain pending review. Missing legacy
  receipts preserve single-review behavior.

## Cooling And Unknown Outcomes

- Address identity is user + normalized network + case-sensitive address.
  Registration requires existing withdrawal security verification, cannot
  backdate and does not restart maturity on exact replay.
- Credential clocks track actual password/contact/fund-password/TOTP changes.
  Ordinary verification must not prolong cooling. Existing `updated_at` is a
  conservative historical bound, not an exact reconstructed change timestamp.
- Recheck cooling at admission, review and approved-to-broadcasting claim.
  A cooled approved request waits without releasing funds or consuming attempts.
- A broadcast with unknown result stays frozen until authoritative gateway
  evidence resolves it. Policy changes never authorize speculative refunds.
- Configured Redis rate protection fails closed on unavailable/error counters;
  absence of a rate-limit configuration preserves its separate legacy contract.

## Verification

Real MySQL/Redis checks cover concurrent cap boundaries, same-request replay,
distinct reviewer enforcement, frozen policy counts, rejection release, address
maturity, security-clock changes, audit rollback and broadcast deferral.
Frontend checks cover exact amounts, default-disabled state, revision conflicts
and reasoned saves. Chain tests retain unknown-outcome/refund-once assertions.
No test result certifies live gateway custody or production deployment.
