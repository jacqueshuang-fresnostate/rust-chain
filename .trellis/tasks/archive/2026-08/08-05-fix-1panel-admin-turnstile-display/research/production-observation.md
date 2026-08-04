# Production Turnstile observation

Observed on 2026-08-05 against `https://hipoex.cllbmz.kdns.fr` before the fix is deployed.

## Responses

- `GET /admin/api/v1/auth/login/config` returned HTTP 403 with
  `cf-mitigated: challenge`; Cloudflare applied a Managed Challenge to the admin path.
- `GET /admin/login` returned the same Cloudflare challenge response for a client without an
  existing clearance cookie.
- `GET /api/v1/auth/login/config` remained publicly reachable and returned HTTP 200 with:

```json
{
  "username_login_enabled": false,
  "cf_turnstile_enabled": false,
  "cf_turnstile_site_key": null
}
```

## Root cause mapping

1. The deployed API did not expose a complete runtime policy. At least the Site Key was missing
   from the process environment, and the old policy also treated
   `CF_TURNSTILE_ENFORCE_TOKEN=false` as complete feature disablement.
2. The admin SPA requested the challenge-prone admin config route before the public equivalent.
3. The previous React lifecycle could initialize more than once and discarded a successfully reset
   widget id, making recovery after token errors unreliable.

## Deployment verification after publishing

1. Recreate the API container with matching `CF_TURNSTILE_SECRET` and
   `CF_TURNSTILE_SITE_KEY` values.
2. Set `CF_TURNSTILE_ENFORCE_TOKEN=true` when every login must submit a widget token.
3. Confirm `/api/v1/auth/login/config` returns `cf_turnstile_enabled=true` and the expected Site
   Key.
4. Complete the outer Cloudflare page challenge if configured, then confirm the application login
   page renders the embedded Turnstile widget.
