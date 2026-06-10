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
    return normalizeProfileForSecurity(res.data)
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

export async function resetTransactionPassword(_newPassword: string, _code: string) {
    return {
        code: 400,
        message: '当前后端暂未开放资金密码重置接口',
        data: null,
    }
}
