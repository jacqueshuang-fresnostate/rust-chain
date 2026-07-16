import { client, persistAuthTokens, requestUrl } from './client'
import { i18n } from '@/i18n'

interface BackendLoginResponse {
  access_token?: string
  refresh_token?: string
  requires_2fa?: boolean
  requires_2fa_setup?: boolean
  challenge_id?: string
  setup_challenge_id?: string
}

export interface CountryOption {
  code: string
  name: string
}

export interface LoginConfig {
  usernameLoginEnabled: boolean
}

export interface RegisterConfig {
  emailCodeRequired: boolean
  inviteCodeRequired: boolean
}

export type LoginOutcome =
  | { type: 'authenticated' }
  | { type: 'two-factor'; challengeId: string }
  | { type: 'two-factor-setup'; setupChallengeId: string }

export async function fetchLoginConfig(): Promise<LoginConfig> {
  const response = await client.get<{ username_login_enabled?: boolean }>(requestUrl('/auth/login/config'))
  return { usernameLoginEnabled: Boolean(response.data.username_login_enabled) }
}

export async function fetchRegisterConfig(): Promise<RegisterConfig> {
  const response = await client.get<{ email_code_required?: boolean; invite_code_required?: boolean }>(requestUrl('/auth/register/config'))
  return {
    emailCodeRequired: Boolean(response.data.email_code_required),
    inviteCodeRequired: Boolean(response.data.invite_code_required),
  }
}

export async function loginWithPassword(account: string, password: string): Promise<LoginOutcome> {
  const identifier = account.trim()
  const response = await client.post<BackendLoginResponse>(requestUrl('/auth/login'), {
    ...(identifier.includes('@') ? { email: identifier } : { username: identifier }),
    password,
  })
  const result = response.data

  if (result.requires_2fa && result.challenge_id) return { type: 'two-factor', challengeId: result.challenge_id }
  if (result.requires_2fa_setup && result.setup_challenge_id) return { type: 'two-factor-setup', setupChallengeId: result.setup_challenge_id }
  if (!result.access_token) {
    throw new Error(i18n.global.t('auth.invalidSession'))
  }

  persistAuthTokens(result.access_token, result.refresh_token)
  return { type: 'authenticated' }
}

export async function submitLoginTwoFactor(challengeId: string, totpCode: string): Promise<void> {
  const response = await client.post<BackendLoginResponse>(requestUrl('/auth/login/2fa'), {
    challenge_id: challengeId,
    totp_code: totpCode.trim(),
  })
  if (!response.data.access_token) throw new Error(i18n.global.t('auth.invalidSession'))
  persistAuthTokens(response.data.access_token, response.data.refresh_token)
}

export async function sendLoginTwoFactorResetCode(challengeId: string): Promise<void> {
  await client.post(requestUrl('/auth/login/2fa/reset-code'), { challenge_id: challengeId })
}

export async function resetLoginTwoFactor(challengeId: string, code: string): Promise<void> {
  await client.post(requestUrl('/auth/login/2fa/reset'), { challenge_id: challengeId, code: code.trim() })
}

export async function sendRegistrationCode(email: string): Promise<void> {
  await client.post(requestUrl('/auth/register/email-code'), { email: email.trim() })
}

export async function registerWithEmail(input: { email: string; password: string; code: string; countryCode: string; inviteCode?: string }): Promise<void> {
  const response = await client.post<BackendLoginResponse>(requestUrl('/auth/register'), {
    email: input.email.trim(),
    password: input.password,
    code: input.code.trim(),
    country_code: input.countryCode,
    invite_code: input.inviteCode?.trim() || undefined,
  })
  if (!response.data.access_token) throw new Error(i18n.global.t('auth.invalidSession'))
  persistAuthTokens(response.data.access_token, response.data.refresh_token)
}

export async function sendPasswordResetCode(email: string): Promise<void> {
  await client.post(requestUrl('/auth/password/reset-code'), { email: email.trim() })
}

export async function resetPasswordWithCode(input: { email: string; code: string; password: string }): Promise<void> {
  await client.post(requestUrl('/auth/password/reset'), {
    email: input.email.trim(),
    code: input.code.trim(),
    password: input.password,
  })
}

export async function fetchCountries(): Promise<CountryOption[]> {
  const response = await client.get<{ countries?: Array<Record<string, unknown>> }>(requestUrl('/countries'))
  return (response.data.countries || [])
    .map((row) => ({
      code: String(row.code || row.country_code || row.value || '').trim().toUpperCase(),
      name: String(row.name || row.country_name || row.label || row.zh_name || row.code || '').trim(),
    }))
    .filter((country) => Boolean(country.code))
}
