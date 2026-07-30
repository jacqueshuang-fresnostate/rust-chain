export const LAUNCH_INTRO_SESSION_KEY = 'hippo_mobile_launch_intro_v1'

export interface LaunchIntroStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

export function shouldPlayLaunchIntro(storage: LaunchIntroStorage | null): boolean {
  if (!storage) return true

  try {
    return storage.getItem(LAUNCH_INTRO_SESSION_KEY) !== 'seen'
  } catch {
    return true
  }
}

export function rememberLaunchIntro(storage: LaunchIntroStorage | null): void {
  if (!storage) return

  try {
    storage.setItem(LAUNCH_INTRO_SESSION_KEY, 'seen')
  } catch {
    // Storage can be unavailable in restricted WebViews or privacy modes.
  }
}
