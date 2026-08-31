# Admin Authentication Turnstile Contract

## 1. Scope / Trigger

Apply this contract when changing the admin/agent password-login page,
Cloudflare Turnstile script loading, runtime login configuration, two-factor
transitions, or the `cf_turnstile_token` request field in `web/src/auth/`.
It prevents detached challenge iframes and stale callbacks from accumulating
cross-origin `postMessage` warnings in the React SPA.

## 2. Signatures

```ts
type TurnstileWidgetId = string | number

loadTurnstileApi(): Promise<TurnstileApi>

createTurnstileLifecycle(options?: TurnstileLifecycleOptions): {
  render(request: TurnstileRenderRequest): Promise<TurnstileWidgetId | null>
  reset(): boolean
  remove(): void
  getWidgetId(): TurnstileWidgetId | null
}

type AdminLoginPayload = {
  username: string
  password: string
  cf_turnstile_token?: string
}
```

The only accepted client script URL is:

```text
https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit
```

## 3. Contracts

- Keep one module-scoped loader Promise and reuse an existing exact-URL script.
  A failed load clears the Promise and removes the failed script for retry.
- A script created by this application sets `async=false` and `defer=false`
  before insertion, then resolves through `turnstile.ready()`. If an enclosing
  deployment already loaded an async/defer script and exposed the API, reuse
  that API directly instead of calling `ready()` through that unsupported
  loading pattern.
- Every render owns an incrementing generation. Recheck the generation,
  `container.isConnected`, and `isContainerCurrent(container)` after every
  asynchronous boundary and after synchronous `render()` completion.
- `remove()` invalidates first and then removes the current widget. All token,
  expiry, error, timeout, and interactive callbacks ignore stale generations.
- `LoginPage` owns one lifecycle instance for its mounted lifetime. Switching
  admin/agent scope does not rebuild the widget. Unmount, disabled config, and
  transition to the two-factor form remove it before the old container leaves.
- A nonblank token is sent through the existing `cf_turnstile_token` field.
  Siteverify, secret handling, login authorization, and two-factor semantics
  remain backend-owned and unchanged.
- Cloudflare Hostname Management must authorize the deployed admin/mobile
  origins. A parent hostname such as `cllbmz.kdns.fr` may authorize its direct
  subdomains; otherwise list the exact production hostnames.

## 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Script or API load fails | Clear loader cache, remove a failed owned script, show the existing Chinese load failure, and permit retry |
| Render generation is stale or container is disconnected | Return `null`; if `render()` already returned an ID, remove that stale widget immediately |
| Old widget emits a token/error/timeout callback | Ignore it; do not mutate the current token or toast state |
| Login switches admin/agent scope | Preserve the current widget and token; do not add a script, render, or iframe |
| Password login enters two-factor mode | Remove the password-step widget and clear its token before rendering the two-factor form |
| Reset succeeds for widget ID `0` or another valid ID | Keep the widget ID and clear the token for a new challenge |
| Reset throws | Invalidate/remove the widget and explicitly render a replacement when still enabled |
| Cloudflare returns hostname error `110200` or no token | Correct Hostname Management; do not alter browser-origin checks or intercept `postMessage` |
| One origin mismatch appears during a successful iframe initialization | Treat it as provider-internal navigation; repeated warnings or missing tokens require lifecycle/hostname investigation |

## 5. Good / Base / Bad Cases

- Good: Login unmounts while the script is loading; the resolved task never
  renders into the detached node, and revisiting Login reuses one API script.
- Good: a successful password response requires TOTP; the current widget is
  removed before the two-factor form appears and its old callback is inert.
- Base: Cloudflare emits one transient target-origin warning while its iframe
  changes origins, but a valid token is returned and route changes do not
  multiply the warning.
- Bad: each React effect appends a script, login-scope changes rerender the
  challenge, or code patches `window.postMessage`/`console` to hide the symptom.

## 6. Tests Required

- Unit-test loader singleton reuse, failed-load retry, owned synchronous script
  readiness, and direct reuse of an existing async/defer API script.
- Unit-test generation cancellation before and after API load, detached
  containers, synchronous stale-widget removal, stale callbacks, ID `0`, reset,
  and remove ordering.
- Component-test runtime sitekey rendering, token submission, no rerender when
  switching admin/agent scope, two-factor cleanup, and unmount cleanup.
- Run `npm --prefix web run typecheck`, `npm --prefix web run lint`,
  `npm --prefix web run test`, `npm --prefix web run build`, and
  `git diff --check`.
- Browser-test both deployed login origins: at most one exact API script and
  one current challenge iframe; successful token creation; navigate away/back;
  no accumulating stale-widget warnings.

## 7. Wrong vs Correct

### Wrong

```tsx
useEffect(() => {
  const script = document.createElement('script')
  script.async = true
  script.src = TURNSTILE_SCRIPT_URL
  document.head.append(script)
  script.onload = () => window.turnstile?.render(containerRef.current!, options)
}, [loginScope])
```

### Correct

```tsx
const lifecycleRef = useRef<TurnstileLifecycle>()
lifecycleRef.current ??= createTurnstileLifecycle()

useEffect(() => {
  void lifecycleRef.current?.render({
    resolveContainer: () => containerRef.current,
    isContainerCurrent: (node) => mountedRef.current && containerRef.current === node,
    options: { sitekey },
    callbacks: { callback: setCfTurnstileToken },
  })
  return () => lifecycleRef.current?.remove()
}, [sitekey, enabled, challengeId])
```

## 8. Admin Session Ownership and Login Mutation Contract

### 1. Scope / Trigger

- Trigger: changing Admin/Agent login, refresh, logout, protected redirects,
  cross-tab synchronization, or login/2FA/Turnstile mutation policy.

### 2. Signatures

```ts
interface AuthSession {
  accessToken: string
  refreshToken: string
  generation: string
  scope: 'admin' | 'agent' | 'user'
  subject: string
}

authStore.compareAndSetSession(scope, expectedGeneration, expectedRefreshToken, tokens): boolean
authStore.clearSession(scope, expectedGeneration?): boolean
```

### 3. Contracts

- Access/refresh tokens live in `sessionStorage`, not durable local storage.
  `localStorage` and `BroadcastChannel` carry only non-sensitive replacement/
  clear signals. A legacy local-storage session may migrate once and is removed.
- Every interactive login creates a generation. Refresh compares generation and
  refresh token; a late refresh or old 401 cannot overwrite/clear a newer login.
- Cross-tab login/logout invalidates the local tab's session instead of copying
  credentials between tabs. Query/cache identity includes subject + generation.
- Login, two-factor, Turnstile, and other mutations use `retry: false` globally;
  an explicit user action is required to retry. Redirects accept only normalized
  internal Admin/Agent paths, never protocol-relative or external URLs.

### 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Refresh returns after logout/new login | CAS fails; preserve current/empty session |
| Old request returns 401 | Do not refresh or clear a different generation |
| Another tab replaces or clears session | Clear this tab and require re-authentication |
| Storage access throws | Fail closed for protected route without crashing login |
| Turnstile token missing when enforced | Keep form recoverable; send no login mutation |
| Redirect begins `//`, contains another origin, or targets wrong scope | Use the scope dashboard fallback |

### 5. Good / Base / Bad Cases

- Good: logout occurs while refresh is in flight; the response is ignored and
  the protected query cache is invalidated.
- Base: an Admin deep link survives login only when it is a valid internal Admin path.
- Bad: persist refresh tokens in local storage, broadcast credentials, enable
  automatic mutation retry, or let an old 401 clear a new session.

### 6. Tests Required

- Test logout/new-login races, refresh CAS, old 401 handling, cross-tab signals,
  storage exceptions, legacy migration/removal, and query key isolation.
- Test Admin/Agent internal redirect allowlists and external/protocol-relative rejection.
- Test QueryClient mutation defaults and login/2FA/Turnstile single submission.

### 7. Wrong vs Correct

```ts
// Wrong
localStorage.setItem('session', JSON.stringify(tokens))
useMutation({ mutationFn: login, retry: 3 })

// Correct
const session = authStore.setSession({ ...tokens, scope, subject })
useMutation({ mutationFn: login, retry: false })
authStore.compareAndSetSession(scope, session.generation, session.refreshToken, refreshed)
```
