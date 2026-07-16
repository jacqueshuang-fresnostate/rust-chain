import { client, persistAuthTokens, requestUrl } from './client'
import { asNumber } from '@/core/format'

export interface UserProfile {
  id: number
  username?: string
  email?: string
  phone?: string
  avatarUrl?: string
  countryCode?: string
  kycLevel: number
  emailVerified: boolean
  fundPasswordSet: boolean
  createdAt: number
}

export interface KycCountryDocumentRule {
  country: string
  documentTypes: string[]
  handheldDocumentTypes: string[]
}

export interface KycConfig {
  enabled: boolean
  targetKycLevel: number
  requiredDocuments: string[]
  allowedCountries: string[]
  countryDocumentTypes: KycCountryDocumentRule[]
  maxDocumentSizeBytes: number
}

export interface KycSubmission {
  id: number
  realName: string
  submissionType: 'personal' | 'enterprise'
  country: string
  idNumber: string
  enterpriseName?: string
  businessRegistrationNumber?: string
  documentType: string
  status: 'pending' | 'approved' | 'rejected'
  targetKycLevel: number
  reviewReason?: string
  submittedAt: number
}

export interface KycStatus {
  config: KycConfig
  latestSubmission?: KycSubmission
}

export interface TwoFactorStatus {
  totpEnabled: boolean
  loginTwoFactorEnabled: boolean
  loginTwoFactorMode: string
  canToggleLoginTwoFactor: boolean
}

export interface TwoFactorSetup {
  secret: string
  otpAuthUri: string
}

export type ThirdPartyProvider = 'coinbase_wallet' | 'telegram_account'

export interface ThirdPartyBinding {
  provider: ThirdPartyProvider
  accountIdentifier: string
  displayName?: string
  status: string
  createdAt: number
  updatedAt: number
}

export interface ThirdPartyBindingStatus {
  coinbaseWalletEnabled: boolean
  telegramAccountEnabled: boolean
  bindings: ThirdPartyBinding[]
}

export interface ReferralCode {
  code: string
  usedCount: number
  usageLimit?: number
  status: string
}

export interface InviteRecord {
  userId: number
  email?: string
  phone?: string
  status: string
  createdAt: number
}

interface BackendTokenResponse {
  access_token?: string
  refresh_token?: string
}

export async function fetchUserProfile(): Promise<UserProfile> {
  const response = await client.get<Record<string, unknown>>(requestUrl('/user/profile'))
  const profile = response.data
  return {
    id: asNumber(profile.id),
    username: optionalText(profile.username),
    email: optionalText(profile.email),
    phone: optionalText(profile.phone),
    avatarUrl: optionalText(profile.avatar_url),
    countryCode: optionalText(profile.country_code),
    kycLevel: asNumber(profile.kyc_level),
    emailVerified: Boolean(profile.email_verified_at),
    fundPasswordSet: Boolean(profile.fund_password_set),
    createdAt: normalizeTimestamp(profile.created_at),
  }
}

export async function updateUsername(username: string): Promise<string> {
  const response = await client.patch<{ username?: string }>(requestUrl('/user/username'), { username: username.trim() })
  return response.data.username?.trim() || username.trim()
}

export async function uploadUserAvatar(file: File): Promise<string> {
  const formData = new FormData()
  formData.append('file', file)
  const response = await client.post<{ avatar_url?: string }>(requestUrl('/user/avatar'), formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  })
  return response.data.avatar_url?.trim() || ''
}

export async function fetchKycStatus(): Promise<KycStatus> {
  const response = await client.get<{ config?: Record<string, unknown>; latest_submission?: Record<string, unknown> | null }>(requestUrl('/user/kyc'))
  const config = response.data.config || {}
  return {
    config: {
      enabled: Boolean(config.enabled),
      targetKycLevel: asNumber(config.target_kyc_level),
      requiredDocuments: stringArray(config.required_documents),
      allowedCountries: stringArray(config.allowed_countries),
      countryDocumentTypes: (Array.isArray(config.country_document_types) ? config.country_document_types : []).map((item) => {
        const rule = item as Record<string, unknown>
        return {
          country: String(rule.country || '').trim(),
          documentTypes: stringArray(rule.document_types),
          handheldDocumentTypes: stringArray(rule.handheld_document_types),
        }
      }).filter((rule) => Boolean(rule.country)),
      maxDocumentSizeBytes: asNumber(config.max_document_size_bytes, 5 * 1024 * 1024),
    },
    latestSubmission: response.data.latest_submission ? mapKycSubmission(response.data.latest_submission) : undefined,
  }
}

export async function submitKycApplication(input: {
  realName: string
  submissionType: 'personal' | 'enterprise'
  enterpriseName?: string
  businessRegistrationNumber?: string
  country: string
  idNumber: string
  documentType: string
  documentFrontImage: string
  documentBackImage: string
  documentHandheldImage?: string
}): Promise<KycSubmission> {
  const response = await client.post<Record<string, unknown>>(requestUrl('/user/kyc/submissions'), {
    real_name: input.realName.trim(),
    submission_type: input.submissionType,
    enterprise_name: input.enterpriseName?.trim() || undefined,
    business_registration_number: input.businessRegistrationNumber?.trim() || undefined,
    country: input.country,
    id_number: input.idNumber.trim(),
    document_type: input.documentType,
    document_front_image: input.documentFrontImage,
    document_back_image: input.documentBackImage,
    document_handheld_image: input.documentHandheldImage,
  })
  return mapKycSubmission(response.data)
}

export async function fetchTwoFactorStatus(): Promise<TwoFactorStatus> {
  const response = await client.get<Record<string, unknown>>(requestUrl('/user/2fa'))
  return {
    totpEnabled: Boolean(response.data.totp_enabled),
    loginTwoFactorEnabled: Boolean(response.data.login_2fa_enabled),
    loginTwoFactorMode: String(response.data.login_2fa_mode || 'none'),
    canToggleLoginTwoFactor: Boolean(response.data.can_toggle_login_2fa),
  }
}

export async function setupTwoFactor(): Promise<TwoFactorSetup> {
  const response = await client.post<{ secret: string; otpauth_uri: string }>(requestUrl('/user/2fa/setup'))
  return { secret: response.data.secret, otpAuthUri: response.data.otpauth_uri }
}

export async function confirmTwoFactor(totpCode: string): Promise<void> {
  await client.post(requestUrl('/user/2fa/confirm'), { totp_code: totpCode.trim() })
}

export async function updateLoginTwoFactor(enabled: boolean): Promise<void> {
  await client.patch(requestUrl('/user/2fa/login'), { enabled })
}

export async function sendUserTwoFactorResetCode(): Promise<void> {
  await client.post(requestUrl('/user/2fa/reset-code'), {})
}

export async function resetUserTwoFactor(code: string): Promise<void> {
  await client.post(requestUrl('/user/2fa/reset'), { code: code.trim() })
}

export async function fetchThirdPartyBindings(): Promise<ThirdPartyBindingStatus> {
  const response = await client.get<{ policy?: Record<string, unknown>; bindings?: Array<Record<string, unknown>> }>(requestUrl('/user/third-party-bindings'))
  return mapThirdPartyBindings(response.data)
}

export async function bindThirdPartyAccount(input: { provider: ThirdPartyProvider; accountIdentifier: string; displayName?: string }): Promise<ThirdPartyBindingStatus> {
  const response = await client.post<{ policy?: Record<string, unknown>; bindings?: Array<Record<string, unknown>> }>(requestUrl('/user/third-party-bindings'), {
    provider: input.provider,
    account_identifier: input.accountIdentifier.trim(),
    display_name: input.displayName?.trim() || undefined,
  })
  return mapThirdPartyBindings(response.data)
}

export async function sendEmailBindCode(email: string): Promise<void> {
  await client.post(requestUrl('/user/email/bind-code'), { email: email.trim() })
}

export async function bindEmail(email: string, code: string): Promise<string> {
  const response = await client.post<{ email?: string }>(requestUrl('/user/email/bind'), { email: email.trim(), code: code.trim() })
  return response.data.email?.trim() || email.trim()
}

export async function changeLoginPassword(oldPassword: string, newPassword: string): Promise<void> {
  const response = await client.patch<BackendTokenResponse>(requestUrl('/user/password'), {
    old_password: oldPassword,
    new_password: newPassword,
  })
  if (response.data.access_token && response.data.refresh_token) persistAuthTokens(response.data.access_token, response.data.refresh_token)
}

export async function setFundPassword(loginPassword: string, fundPassword: string): Promise<void> {
  await client.post(requestUrl('/user/fund-password'), { login_password: loginPassword, fund_password: fundPassword })
}

export async function changeFundPassword(oldFundPassword: string, newFundPassword: string): Promise<void> {
  await client.patch(requestUrl('/user/fund-password'), { old_fund_password: oldFundPassword, new_fund_password: newFundPassword })
}

export async function sendFundPasswordResetCode(): Promise<void> {
  await client.post(requestUrl('/user/fund-password/reset-code'), {})
}

export async function resetFundPassword(code: string, newFundPassword: string): Promise<void> {
  await client.post(requestUrl('/user/fund-password/reset'), { code: code.trim(), new_fund_password: newFundPassword })
}

export async function fetchReferralCode(): Promise<ReferralCode> {
  const response = await client.get<Record<string, unknown>>(requestUrl('/referral/my-code'))
  return {
    code: String(response.data.code || '').trim(),
    usedCount: asNumber(response.data.used_count),
    usageLimit: response.data.usage_limit === null || response.data.usage_limit === undefined ? undefined : asNumber(response.data.usage_limit),
    status: String(response.data.status || ''),
  }
}

export async function fetchReferralInvites(): Promise<InviteRecord[]> {
  const response = await client.get<{ users?: Array<Record<string, unknown>> }>(requestUrl('/referral/my-invites'))
  return (response.data.users || []).map((user) => ({
    userId: asNumber(user.user_id),
    email: optionalText(user.email),
    phone: optionalText(user.phone),
    status: String(user.status || ''),
    createdAt: normalizeTimestamp(user.created_at),
  }))
}

export async function bindReferralCode(code: string): Promise<void> {
  await client.post(requestUrl('/referral/bind'), { code: code.trim() })
}

function mapThirdPartyBindings(response: { policy?: Record<string, unknown>; bindings?: Array<Record<string, unknown>> }): ThirdPartyBindingStatus {
  const policy = response.policy || {}
  return {
    coinbaseWalletEnabled: Boolean(policy.coinbase_wallet_enabled),
    telegramAccountEnabled: Boolean(policy.telegram_account_enabled),
    bindings: (response.bindings || []).map((binding) => ({
      provider: String(binding.provider || '') === 'telegram_account' ? 'telegram_account' : 'coinbase_wallet',
      accountIdentifier: String(binding.account_identifier || ''),
      displayName: optionalText(binding.display_name),
      status: String(binding.status || ''),
      createdAt: normalizeTimestamp(binding.created_at),
      updatedAt: normalizeTimestamp(binding.updated_at),
    })),
  }
}

function mapKycSubmission(submission: Record<string, unknown>): KycSubmission {
  const status = String(submission.status || 'pending').toLowerCase()
  return {
    id: asNumber(submission.id),
    realName: String(submission.real_name || ''),
    submissionType: String(submission.submission_type || 'personal').toLowerCase() === 'enterprise' ? 'enterprise' : 'personal',
    country: String(submission.country || ''),
    idNumber: String(submission.id_number || ''),
    enterpriseName: optionalText(submission.enterprise_name),
    businessRegistrationNumber: optionalText(submission.business_registration_number),
    documentType: String(submission.document_type || ''),
    status: status === 'approved' || status === 'rejected' ? status : 'pending',
    targetKycLevel: asNumber(submission.target_kyc_level),
    reviewReason: optionalText(submission.review_reason),
    submittedAt: normalizeTimestamp(submission.submitted_at),
  }
}

function optionalText(value: unknown): string | undefined {
  const text = typeof value === 'string' ? value.trim() : ''
  return text || undefined
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.map((item) => String(item).trim()).filter(Boolean) : []
}

function normalizeTimestamp(value: unknown): number {
  const timestamp = asNumber(value)
  return timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
}
