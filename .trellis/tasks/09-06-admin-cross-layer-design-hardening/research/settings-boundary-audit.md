# Settings/request boundary audit

## Inspected and preserved

- `web/src/api/client.ts` bounds header + body consumption, classifies timeout,
  abort, contract and network errors, avoids mutation retry except the existing
  401 refresh path, and protects refresh/clear by captured session generation.
- `web/src/app/providers.tsx` cancels queries/clears caches on identity changes,
  so unscoped singleton settings keys alone are not proof of cross-login leakage.
- `useAdminSettingsEditor.ts` preserves dirty drafts on cache updates and 409,
  and mutations have automatic retry disabled. Do not regress this groundwork.

## Confirmed design debt requiring a dedicated later slice

1. `application/risk_security.rs::update_admin_security_policy` reads before
   outside its write transaction. Multiple admins may overwrite each other's
   settings and the audit before snapshot can be stale. Platform-brand writes
   lock, but the browser sends no revision, so stale user edits still use last
   writer wins. Proper optimistic concurrency needs an explicit revision or
   original-snapshot contract, authoritative read locking and coordinated
   frontend/backend rollout across all write aliases. Do not pretend that UI
   409 handling alone implements it.
2. `application/config_changes.rs::apply_admin_config_change` only marks an
   approved request applied; it does not dispatch any business configuration
   write. Proposed JSON has already been redacted and is unsuitable as an
   executable secret payload. The route exists without a current Admin UI
   workflow. A later dedicated approval integration should either fail closed
   for unsupported domains or atomically apply supported typed commands. Do not
   expose it as a successful generic executor or broaden its permissions now.

## First repair-slice decision

Implement four directly reproducible defects before introducing a new settings
approval protocol: recharge precision at the wallet boundary; executable convert
configuration validation; latest-only resource details; stale batch-selection
isolation during list transitions. These preserve existing business policy and
API payload shapes, and have bounded cross-layer regression surfaces.
