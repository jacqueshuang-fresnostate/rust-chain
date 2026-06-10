import request from './request'
import { backendApiUrl, normalizeAuthResponse } from './backendAdapters'

export interface LoginParams {
  email: string
  password?: string
  code?: string
  type: 'password' | 'code'
}

export interface RegisterParams {
  email: string
  password?: string
  code: string
  promotion?: string
}

export async function login(data: LoginParams) {
    const res = await request.instance.post(backendApiUrl('/auth/login'), {
        email: data.email,
        password: data.password,
    })
    return normalizeAuthResponse(res.data)
}

export async function sendVerifyCode(_email: string) {
    return {
        code: 0,
        message: '当前后端注册接口不需要邮箱验证码',
        data: { sent: true },
    }
}

export async function register(data: RegisterParams) {
    const res = await request.instance.post(backendApiUrl('/auth/register'), {
        email: data.email,
        password: data.password,
    })
    return normalizeAuthResponse(res.data)
}

export async function resetPassword(_data: { mode: number, account: string, code: string, password?: string }) {
    return {
        code: 400,
        message: '当前后端暂未开放登录密码找回接口',
        data: null,
    }
}

export async function sendResetVerifyCode(_account: string) {
    return {
        code: 400,
        message: '当前后端暂未开放登录密码找回验证码接口',
        data: null,
    }
}
