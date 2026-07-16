# Tauri 2 Mobile + Vite Research

## Decision

Create a standalone `mobile/` Vue 3 application. It is served as an ordinary Vite static site for H5 and packaged by Tauri v2 for Android and iOS. The mobile client talks to the same versioned backend HTTP contracts used by the PC client; it does not call the database or duplicate backend order/wallet rules.

## Project shape

```
mobile/
  src/          # Vue views, API adapters, stores, shared mobile components
  tests/        # Node tests, intentionally outside production code
  src-tauri/    # Shared Tauri Rust entry point and generated native targets
```

The Rust entry point belongs in `src-tauri/src/lib.rs` and exposes `run()` with `#[cfg_attr(mobile, tauri::mobile_entry_point)]`. `src-tauri/src/main.rs` only calls that shared entry point for desktop preview. This layout is required because Tauri compiles the Rust crate as a library for mobile builds.

## Vite and H5 constraints

* Keep `frontendDist` at `../dist`, so the same Vite output is used by H5 hosting and Tauri packaging.
* Use `TAURI_DEV_HOST` when Tauri provides it, enabling physical iOS devices to reach the Vite dev server. The ordinary H5 dev server remains available on a fixed local port.
* Use hash routing so a static H5 host does not need server-side history fallback rules.
* Use `viewport-fit=cover`, `100dvh`, and `env(safe-area-inset-*)`; do not rely on legacy `100vh` or viewport-scaled font sizes.

## Native delivery constraints

* Tauri v2 exposes `tauri android init/dev/build` and `tauri ios init/dev/build`; native targets are generated after the JavaScript and Rust project configuration exists.
* The local machine has Xcode 26.2, CocoaPods, the iOS and Android Rust targets, and an Android SDK at `~/Library/Android/sdk`. `ANDROID_HOME` / `ANDROID_SDK_ROOT` are not exported by the shell, so package scripts should derive the default SDK path when possible and document the override variables.
* Android production builds depend on the installed Android SDK/NDK and Java configuration. iOS release distribution still requires an Apple signing team and provisioning setup, which must remain environment-specific rather than committed to source control.

## API boundary

* Market list and ticker: `/markets` and `/markets/{symbol}/ticker`.
* K-line/depth/trades: `/markets/{symbol}/klines`, `/depth`, `/trades`.
* Spot order and account data: `/spot/orders`, `/wallet/accounts`.
* Margin data/actions: existing `/margin/*` endpoints.
* Deposit flow: `/wallet/assets`, `/wallet/deposit-networks`, `/wallet/deposit-address`.
* Authentication: `/auth/login` and `/auth/refresh` with Bearer tokens.

Mobile API adapters normalize these boundary payloads in one place and render unavailable/authenticated states explicitly. Demo data, when used to keep an offline interface inspectable, is labeled as sample data and is never submitted as an order or balance.

## Sources

* https://v2.tauri.app/start/project-structure/
* https://v2.tauri.app/start/frontend/vite/
* https://v2.tauri.app/develop/
* https://v2.tauri.app/start/prerequisites/
* https://v2.tauri.app/reference/config/
