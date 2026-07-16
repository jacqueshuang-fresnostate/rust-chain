import request from './request'
import {
    backendApiUrl,
    mapMyInvitesToPcInviteRecords,
    normalizeProfileForSecurity,
    type BackendMyInvitesResponse,
} from './backendAdapters'

export interface MemberSecurity {
    username?: string
    id?: number
    createTime?: string
    realName?: string
    idCard?: string
    avatar?: string
    promotionCode?: string
    email?: string
    phone?: string
    fundsVerified?: number
    emailVerified?: number
    phoneVerified?: number
    realVerified?: number
    realAuditing?: number
    avatarString?: string
    transactionStatus?: number
}

export interface UserAvatarUploadResult {
    avatar_url: string
    upload?: {
        provider: string
        object_key: string
        download_url: string
        mime_type: string
        size_bytes: number
    }
}

export interface UpdateUsernameResult {
    username: string
}

export interface KycConfig {
    enabled: boolean
    target_kyc_level: number
    required_documents: string[]
    allowed_countries: string[]
    country_document_types: KycCountryDocumentTypeRule[]
    max_document_size_bytes: number
}

export interface KycCountryDocumentTypeRule {
    country: string
    document_types: string[]
    handheld_document_types?: string[]
}

export interface KycSubmission {
    id: number
    user_id: number
    real_name: string
    submission_type: 'personal' | 'enterprise'
    country: string
    id_number: string
    enterprise_name?: string | null
    business_registration_number?: string | null
    document_type: string
    document_front_image?: string
    document_back_image?: string
    document_handheld_image?: string | null
    status: 'pending' | 'approved' | 'rejected'
    target_kyc_level: number
    review_reason?: string | null
    submitted_at: number
    reviewed_at?: number | null
}

export interface KycStatus {
    config: KycConfig
    latest_submission?: KycSubmission | null
}

export interface SubmitKycPayload {
    real_name: string
    submission_type?: 'personal' | 'enterprise'
    enterprise_name?: string
    business_registration_number?: string
    country: string
    id_number: string
    document_type?: string
    document_front_image: string
    document_back_image: string
    document_handheld_image?: string
}

export type SecurityVerificationMethod = 'fund_password' | 'two_factor' | 'fund_password_and_two_factor'
export type LoginTwoFactorMode = 'none' | 'user_enabled' | 'mandatory'

export interface PaymentPolicy {
    enabled: boolean
    method: SecurityVerificationMethod
}

export interface PaymentPolicies {
    withdraw: PaymentPolicy
    spot_order: PaymentPolicy
    convert: PaymentPolicy
    earn_subscribe: PaymentPolicy
}

export interface ThirdPartyBindingPolicy {
    coinbase_wallet_enabled: boolean
    telegram_account_enabled: boolean
}

export type ThirdPartyProvider = 'coinbase_wallet' | 'telegram_account'

export interface ThirdPartyBinding {
    provider: ThirdPartyProvider
    account_identifier: string
    display_name?: string | null
    status: 'bound' | 'disabled'
    created_at: number
    updated_at: number
}

export interface ThirdPartyBindingStatus {
    policy: ThirdPartyBindingPolicy
    bindings: ThirdPartyBinding[]
}

export interface TwoFactorStatus {
    totp_enabled: boolean
    login_2fa_enabled: boolean
    login_2fa_mode: LoginTwoFactorMode
    can_toggle_login_2fa: boolean
    payment_policies: PaymentPolicies
    third_party_bindings: ThirdPartyBindingPolicy
}

export interface TwoFactorSetup {
    secret: string
    otpauth_uri: string
}

export async function getReferralCode() {
    const res = await request.instance.get(backendApiUrl('/referral/my-code'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function getReferralInvites() {
    const res = await request.instance.get<BackendMyInvitesResponse>(backendApiUrl('/referral/my-invites'))
    return mapMyInvitesToPcInviteRecords(res.data)
}

export async function changeLoginPassword(oldPassword: string, newPassword: string) {
    const res = await request.instance.patch(backendApiUrl('/user/password'), {
        old_password: oldPassword,
        new_password: newPassword,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function getSecuritySetting() {
    const res = await request.instance.get(backendApiUrl('/user/profile'))
    const security = normalizeProfileForSecurity(res.data)
    try {
        const kyc = await getKycStatus()
        const latest = kyc.data.latest_submission
        if (latest) {
            security.data.realName = latest.real_name
            security.data.idCard = latest.id_number
            security.data.realAuditing = latest.status === 'pending' ? 1 : 0
            security.data.realVerified = latest.status === 'approved' || security.data.realVerified === 1 ? 1 : 0
        }
    } catch (error) {
        console.error('Failed to load KYC status', error)
    }
    return security
}

export async function uploadUserAvatar(file: File) {
    const formData = new FormData()
    formData.append('file', file)
    const res = await request.instance.post<UserAvatarUploadResult>(backendApiUrl('/user/avatar'), formData)
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function updateUsername(username: string) {
    const res = await request.instance.patch<UpdateUsernameResult>(backendApiUrl('/user/username'), {
        username,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function getKycStatus() {
    const res = await request.instance.get<KycStatus>(backendApiUrl('/user/kyc'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function submitKycApplication(payload: SubmitKycPayload) {
    const res = await request.instance.post<KycSubmission>(backendApiUrl('/user/kyc/submissions'), payload)
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function getTwoFactorStatus() {
    const res = await request.instance.get<TwoFactorStatus>(backendApiUrl('/user/2fa'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function getThirdPartyBindings() {
    const res = await request.instance.get<ThirdPartyBindingStatus>(backendApiUrl('/user/third-party-bindings'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function bindThirdPartyAccount(provider: ThirdPartyProvider, accountIdentifier: string, displayName?: string) {
    const res = await request.instance.post<ThirdPartyBindingStatus>(backendApiUrl('/user/third-party-bindings'), {
        provider,
        account_identifier: accountIdentifier,
        display_name: displayName || undefined,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function setupTwoFactor() {
    const res = await request.instance.post<TwoFactorSetup>(backendApiUrl('/user/2fa/setup'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function confirmTwoFactor(totpCode: string) {
    const res = await request.instance.post(backendApiUrl('/user/2fa/confirm'), {
        totp_code: totpCode,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function updateLoginTwoFactor(enabled: boolean) {
    const res = await request.instance.patch(backendApiUrl('/user/2fa/login'), {
        enabled,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function sendTwoFactorResetCode() {
    const res = await request.instance.post(backendApiUrl('/user/2fa/reset-code'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function resetTwoFactor(code: string) {
    const res = await request.instance.post(backendApiUrl('/user/2fa/reset'), {
        code,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function setTransactionPassword(fundPassword: string, loginPassword: string) {
    const res = await request.instance.post(backendApiUrl('/user/fund-password'), {
        login_password: loginPassword,
        fund_password: fundPassword,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function updateTransactionPassword(oldPassword: string, newPassword: string) {
    const res = await request.instance.patch(backendApiUrl('/user/fund-password'), {
        old_fund_password: oldPassword,
        new_fund_password: newPassword,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function sendTransactionPasswordResetCode() {
    const res = await request.instance.post(backendApiUrl('/user/fund-password/reset-code'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function resetTransactionPassword(newPassword: string, code: string) {
    const res = await request.instance.post(backendApiUrl('/user/fund-password/reset'), {
        code,
        new_fund_password: newPassword,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}
