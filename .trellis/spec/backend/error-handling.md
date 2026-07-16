# Error Handling

> How errors are handled in this project.

---

## Overview

<!--
Document your project's error handling conventions here.

Questions to answer:
- What error types do you define?
- How are errors propagated?
- How are errors logged?
- How are errors returned to clients?
-->

(To be filled by the team)

---

## Error Types

<!-- Custom error classes/types -->

(To be filled by the team)

---

## Error Handling Patterns

<!-- Try-catch patterns, error propagation -->

(To be filled by the team)

---

## API Error Responses

<!-- Standard error response format -->

### Scenario: External Provider HTML / Cloudflare Responses

#### 1. Scope / Trigger

- Trigger: Backend code calls a third-party HTTP API and exposes the result through an app API.
- Applies to payment/provider integrations such as GMPay quick recharge.

#### 2. Signatures

- Use `AppError::Api { status: StatusCode::BAD_GATEWAY, code: "<PROVIDER>_REQUEST_FAILED", message }` for upstream provider failures that are not caused by local validation.
- Keep the provider-specific `code` stable so frontend toast/error handling does not need a special case.

#### 3. Contracts

- Request: provider requests should include a service User-Agent and `Accept: application/json` when the provider is expected to return JSON.
- Response to frontend/admin UI: JSON error body remains `{ "code": "...", "message": "..." }`.
- Do not include provider secrets, signatures, form payloads, or full HTML bodies in `message`.

#### 4. Validation & Error Matrix

- Non-2xx JSON/text provider response -> `502` with provider code and a compact response snippet.
- Non-2xx HTML provider response -> `502` with a short actionable message that the configured API endpoint returned HTML, not JSON.
- Cloudflare challenge markers such as `__cf_chl`, `challenge-platform`, `challenges.cloudflare.com`, or `Just a moment` -> `502` with a message telling the operator to use the provider backend API domain or request IP/API-path allowlisting.
- 2xx response with HTML body -> `502` with the same HTML endpoint guidance.
- 2xx response with malformed non-HTML JSON -> internal error may include only a compact body snippet for debugging.

#### 5. Good/Base/Bad Cases

- Good: `GMPAY_REQUEST_FAILED` message says Cloudflare blocked the server request and names the operational fix.
- Base: a short provider JSON error message is surfaced with the provider failure code.
- Bad: raw `<!DOCTYPE html>...` or Cloudflare challenge JavaScript is returned to the admin UI.

#### 6. Tests Required

- Unit test the provider adapter with a mocked Cloudflare/HTML response.
- Assert status `BAD_GATEWAY`, code stability, actionable message content, and absence of raw HTML/challenge markers.
- Keep an existing success-path test to prove request signing and parsing still work.

#### 7. Wrong vs Correct

##### Wrong

```rust
message: format!("provider returned http status {status}: {body}")
```

##### Correct

```rust
message: format_provider_http_error(status, content_type, &body)
```

---

## Common Mistakes

<!-- Error handling mistakes your team has made -->

- Passing third-party HTML error pages directly to frontend users. Sanitize and translate them into an operator-facing action instead.
