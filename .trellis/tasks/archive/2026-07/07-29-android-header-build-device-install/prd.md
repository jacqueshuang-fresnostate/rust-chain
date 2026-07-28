# Android Device Install For Latest Header Build

## Goal

Build the current committed mobile client as an Android aarch64 Debug APK,
install it on the connected Android device, cold-start the application, and
verify that the current Header-control implementation is the code installed.

## Requirements

- Use the current clean `main` worktree containing commit `02457eb`.
- Build with the existing Tauri Android script and aarch64 debug APK target.
- Install with `adb install -r` without clearing application data.
- Cold-stop and start `com.hippo.exchange.mobile/.MainActivity`.
- Verify device identity, APK hash, successful install, launch result, and
  resumed foreground state.
- Record the result in `docs/superpowers/PROGRESS.md`.

## Acceptance Criteria

- [x] Connected device reports `device`.
- [x] Android build exits successfully.
- [x] APK exists and has a recorded SHA-256.
- [x] `adb install -r` returns `Success`.
- [x] Cold launch succeeds.
- [x] `MainActivity` is resumed in the foreground.
- [x] Progress, task archive, and journal are committed.

## Out Of Scope

- Source-code or UI changes.
- Device data reset or uninstall.
- Release signing and store distribution.
