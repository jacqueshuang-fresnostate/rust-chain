import request from './request'
import { backendApiUrl, mapWalletAccountsToMemberWallets } from './backendAdapters'

// --- Types ---

export interface Coin {
    name: string
    unit: string
    coinGroup: string
    canWithdraw: number | boolean
    canRecharge: number | boolean
}

export interface MemberWallet {
    id: string | number
    memberId: string | number
    coin: Coin
    balance: number
    frozenBalance: number
    address: string
}

// --- API Functions ---

/**
 * 查询当前登录用户的钱包列表
 */
export async function getWallets() {
    const res = await request.instance.get(backendApiUrl('/wallet/accounts'))
    return { data: mapWalletAccountsToMemberWallets(res.data) }
}

/**
 * 按币种查询用户钱包
 * @param symbol 币种单位
 */
export async function getWalletBySymbol(symbol: string) {
    const res = await getWallets()
    const target = symbol.trim().toUpperCase()
    return {
        data: {
            ...res.data,
            data: res.data.data.find((wallet) => wallet.coin.coinGroup.toUpperCase() === target) || null,
        },
    }
}
