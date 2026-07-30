# Android WebView Overscroll Research

## Root Cause

The document-level CSS rule owns browser scroll chaining, but Android 12 also
renders native stretch feedback through the host `WebView` EdgeEffect. The
final JavaScript `scrollY` can remain clamped while the compositor temporarily
deforms the rendered page, so checking only the final scroll position does not
prove that the visual stretch is disabled.

## Existing Native Hook

Generated Wry `WryActivity` exposes:

```kotlin
open fun onWebViewCreate(webView: WebView) { }
```

`setWebView` invokes this hook immediately after assigning the `RustWebView`.
The application-owned `MainActivity` can therefore configure the real host
WebView without editing generated Wry sources.

## Native Policy

Set:

```kotlin
webView.overScrollMode = View.OVER_SCROLL_NEVER
```

This disables the Android WebView edge glow/stretch affordance while leaving
the WebView's ordinary scrolling, fling behavior, DOM input handling, and
nested web scroll containers intact.

## Generated Project Constraint

`mobile/src-tauri/gen/` is ignored at three repository levels. A direct edit to
the generated `MainActivity.kt` would work only on the current machine and
would not survive a clean checkout or Android project regeneration. Keep the
application-owned template outside `gen/` and make the existing Android runner
synchronize it at the correct side of the Tauri command:

- before `build` and `dev`;
- after a successful `init`.
