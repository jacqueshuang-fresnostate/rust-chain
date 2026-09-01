# Admin Web Development Guidelines

> Executable conventions for the React 19 + Semi Design operations console in `web/`.

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Admin UI System](./ui-system.md) | Shell, resource pages, filters, tables, confirmation actions, tabs, responsive layout, and browser verification | Active |
| [Resource Response Contract](./resource-response-contract.md) | API-backed versus derived table columns, strict row validation, and business order-number fallbacks | Active |
| [Admin Authentication Turnstile](./auth-turnstile.md) | Explicit-render script loading, React SPA widget lifecycle, token ownership, and two-factor cleanup | Active |
| [Backend Origin and Integrated Image](./backend-origin.md) | Vite compile-time API mode, integrated Docker same-origin wiring, validation, and release checks | Active |

## Quality Gate

Run from the repository root after shared admin UI or page changes:

```bash
npm --prefix web run typecheck
npm --prefix web run lint
npm --prefix web run test
npm --prefix web run test:production-policy
npm --prefix web run test:coverage
npm --prefix web run build
npm --prefix web run budget
git diff --check
```

For visual changes, also use Ego Browser against the local Vite application
with a real API origin. Verify login, Dashboard, one empty resource page, one
populated resource page, KYC, Security Policy, and one SideSheet at 1728px;
repeat the empty resource page at 1280px.

**Language**: Code-spec documentation is written in English. User-facing admin
copy remains Chinese unless product localization is implemented separately.
