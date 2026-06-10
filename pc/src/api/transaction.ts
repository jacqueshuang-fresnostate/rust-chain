import request from './request'
import { backendApiUrl, normalizeWalletLedgerPage } from './backendAdapters'

// Transaction Type Enum
export enum TransactionType {
    RECHARGE = 0,
    WITHDRAW = 1,
    TRANSFER = 2,
    EXCHANGE = 3,
    OTC_BUY = 4,
    OTC_SELL = 5,
    ACTIVITY_AWARD = 6,
    PROMOTION_AWARD = 7,
    DIVIDEND = 8,
    VOTE = 9,
    ADMIN_RECHARGE = 10,
    MATCH = 11
}

export interface TransactionRecord {
    id: number
    memberId: number
    amount: number
    fee: number
    symbol: string
    type: TransactionType
    createTime: string
    address?: string
    status?: number // 0: pending, 1: success, 2: failed
}

interface ApiResponse<T> {
    code: number
    message: string
    data: T
}

interface PageData<T> {
    content: T[]
    page: {
        number: number
        size: number
        totalElements: number
        totalPages: number
    }
}

export async function fetchTransactionHistory(params: { pageNo: number, pageSize: number, type?: TransactionType, symbol?: string, startTime?: string, endTime?: string }): Promise<{ data: ApiResponse<PageData<TransactionRecord>> }> {
    const res = await request.instance.get(backendApiUrl('/wallet/ledger'), {
        params: { limit: 100 },
    })
    const normalized = normalizeWalletLedgerPage(res.data, { pageNo: 1, pageSize: 100 })
    const symbol = params.symbol?.trim().toUpperCase()
    const records = normalized.data.content.filter((record) => {
        if (params.type !== undefined && record.type !== params.type) return false
        if (symbol && record.symbol.toUpperCase() !== symbol) return false
        const date = record.createTime.slice(0, 10)
        if (params.startTime && date < params.startTime) return false
        if (params.endTime && date > params.endTime) return false
        return true
    })
    const start = Math.max(params.pageNo - 1, 0) * params.pageSize
    const content = records.slice(start, start + params.pageSize)

    return {
        data: {
            code: normalized.code,
            message: normalized.message,
            data: {
                content,
                page: {
                    number: Math.max(params.pageNo - 1, 0),
                    size: params.pageSize,
                    totalElements: records.length,
                    totalPages: Math.max(Math.ceil(records.length / params.pageSize), 1),
                },
            },
        },
    }
}
