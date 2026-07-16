import request from './request'
import { backendApiUrl, normalizeAuthResponse } from './backendAdapters'

export interface LoginParams {
  email?: string
  username?: string
  password?: string
  code?: string
  type: 'password' | 'code'
}

export interface RegisterParams {
  email: string
  password?: string
  code: string
  countryCode: string
  inviteCode?: string
  promotion?: string
}

export interface RegisterConfig {
  emailCodeRequired: boolean
  inviteCodeRequired: boolean
}

export interface LoginConfig {
  usernameLoginEnabled: boolean
}

export async function login(data: LoginParams) {
    const payload = data.username
        ? { username: data.username, password: data.password }
        : { email: data.email, password: data.password }
    const res = await request.instance.post(backendApiUrl('/auth/login'), {
        ...payload,
    })
    return normalizeAuthResponse(res.data)
}

export async function submitLoginTwoFactor(challengeId: string, totpCode: string) {
    const res = await request.instance.post(backendApiUrl('/auth/login/2fa'), {
        challenge_id: challengeId,
        totp_code: totpCode,
    })
    return normalizeAuthResponse(res.data)
}

export async function sendLoginTwoFactorResetCode(challengeId: string) {
    const res = await request.instance.post(backendApiUrl('/auth/login/2fa/reset-code'), {
        challenge_id: challengeId,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function resetLoginTwoFactor(challengeId: string, code: string) {
    const res = await request.instance.post(backendApiUrl('/auth/login/2fa/reset'), {
        challenge_id: challengeId,
        code,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function getRegisterConfig(): Promise<{ code: number; message: string; data: RegisterConfig }> {
    const res = await request.instance.get(backendApiUrl('/auth/register/config'))
    return {
        code: 0,
        message: 'success',
        data: {
            emailCodeRequired: Boolean(res.data?.email_code_required),
            inviteCodeRequired: Boolean(res.data?.invite_code_required),
        },
    }
}

export async function getLoginConfig(): Promise<{ code: number; message: string; data: LoginConfig }> {
    const res = await request.instance.get(backendApiUrl('/auth/login/config'))
    return {
        code: 0,
        message: 'success',
        data: {
            usernameLoginEnabled: Boolean(res.data?.username_login_enabled),
        },
    }
}

export async function sendVerifyCode(email: string) {
    const res = await request.instance.post(backendApiUrl('/auth/register/email-code'), {
        email,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}

export async function register(data: RegisterParams) {
    const res = await request.instance.post(backendApiUrl('/auth/register'), {
        email: data.email,
        password: data.password,
        code: data.code,
        country_code: data.countryCode,
        invite_code: data.inviteCode || data.promotion || undefined,
    })
    return normalizeAuthResponse(res.data)
}

export async function resetPassword(data: { mode: number, account: string, code: string, password?: string }) {
    const res = await request.instance.post(backendApiUrl('/auth/password/reset'), {
        email: data.account,
        code: data.code,
        password: data.password,
    })
    return {
        code: 0,
        message: 'success',
        data: {
            reset: Boolean(res.data?.reset),
            requiresRelogin: Boolean(res.data?.requires_relogin),
        },
    }
}

export async function sendResetVerifyCode(account: string) {
    const res = await request.instance.post(backendApiUrl('/auth/password/reset-code'), {
        email: account,
    })
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}
