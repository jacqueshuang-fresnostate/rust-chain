import request from './request'
import { backendApiUrl, normalizeWalletLedgerPage } from './backendAdapters'

export const WALLET_LEDGER_TRANSACTION_TYPES = [
    'deposit',
    'admin_recharge',
    'quick_recharge',
    'convert_settlement',
    'spot_freeze',
    'spot_unfreeze',
    'spot_fill',
    'spot_trade_settlement',
    'spot_price_improvement_release',
    'seconds_contract_open',
    'seconds_contract_settle_win',
    'margin_position_open',
    'margin_position_close',
    'margin_position_liquidate',
    'earn_subscribe',
    'earn_redeem',
    'loan_collateral_freeze',
    'loan_collateral_release',
    'loan_disbursement',
    'loan_repayment',
    'new_coin_subscription_payment',
    'new_coin_subscription_lock',
    'new_coin_purchase_payment',
    'new_coin_purchase_lock',
    'new_coin_distribution_lock',
    'new_coin_unlock_release',
    'asset_lock',
    'agent_commission_payout',
] as const

export type TransactionType = (typeof WALLET_LEDGER_TRANSACTION_TYPES)[number] | (string & {})

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

export function normalizeTransactionDateTimeFilter(value: string | undefined, boundary: 'start' | 'end'): string | undefined {
    const trimmed = value?.trim()
    if (!trimmed) return undefined
    if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) {
        return `${trimmed} ${boundary === 'start' ? '00:00:00' : '23:59:59'}`
    }

    const normalized = trimmed.replace('T', ' ')
    if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/.test(normalized)) {
        return `${normalized}:${boundary === 'start' ? '00' : '59'}`
    }
    return normalized
}

export async function fetchTransactionHistory(params: { pageNo: number, pageSize: number, type?: string, symbol?: string, startTime?: string, endTime?: string }): Promise<{ data: ApiResponse<PageData<TransactionRecord>> }> {
    const pageNo = Math.max(params.pageNo, 1)
    const pageSize = Math.max(params.pageSize, 1)
    const symbol = params.symbol?.trim().toUpperCase()
    const startTime = normalizeTransactionDateTimeFilter(params.startTime, 'start')
    const endTime = normalizeTransactionDateTimeFilter(params.endTime, 'end')
    const res = await request.instance.get(backendApiUrl('/wallet/ledger'), {
        params: {
            limit: pageSize,
            offset: (pageNo - 1) * pageSize,
            change_type: params.type || undefined,
            asset_symbol: symbol || undefined,
            start_time: startTime,
            end_time: endTime,
        },
    })

    return {
        data: normalizeWalletLedgerPage(res.data, { pageNo, pageSize }),
    }
}
