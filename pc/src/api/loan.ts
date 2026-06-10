const unavailableMessage = '当前后端暂未开放借贷接口'

export interface LoanProduct {
    id: number
    status: number
    minInstallments: number // min amount
    maxInstallments: number // max amount
    noDepositsDays: number // interest free days
    depositsDays: number // daily interest rate
    cycle: number // days
    unit: string // currency unit
    fee: number // fee
    createTime?: number
}

export enum OrderStatus {
    PROCESSING = 0, // 审核中
    PULLING = 1,    // 已发放
    SUCCESS = 2,    // 已完成
    FAIL = 3,       // 审核失败
    CANCEL = 4      // 取消
}

export interface InstallmentsOrder {
    id: number
    orderSn: string
    status: OrderStatus
    unit: string
    memberId: number
    loanProductId: number
    amount: number
    deposits: number
    noDepositsDays: number
    depositsDays: number
    cycle: number
    pushTime?: string | number
    createTime: string | number
}

/**
 * Fetch available loan products/cycles
 */
export function fetchLoanProducts(): Promise<{ data: { code: number, message: string, data: LoanProduct[] } }> {
    return Promise.resolve({ data: { code: 400, message: unavailableMessage, data: [] } })
}

/**
 * Apply for a loan
 */
export function applyLoan(_data: { productId: number, amount: number }): Promise<{ data: { code: number, message: string } }> {
    return Promise.resolve({ data: { code: 400, message: unavailableMessage } })
}

/**
 * Fetch user loan orders by status
 */
export function fetchLoanOrders(_status: OrderStatus, pageNo: number = 1, pageSize: number = 10): Promise<{ data: { code: number, message: string, data: { content: InstallmentsOrder[], page: { number: number, size: number, totalElements: number, totalPages: number } } } }> {
    return Promise.resolve({
        data: {
            code: 400,
            message: unavailableMessage,
            data: {
                content: [],
                page: { number: pageNo - 1, size: pageSize, totalElements: 0, totalPages: 0 }
            }
        }
    })
}

/**
 * Cancel a loan order
 */
export function cancelLoanOrder(_id: number): Promise<{ data: { code: number, message: string } }> {
    return Promise.resolve({ data: { code: 400, message: unavailableMessage } })
}

/**
 * Confirm / Pay a loan order
 */
export function confirmLoanOrder(_id: number, _jyPassword: string): Promise<{ data: { code: number, message: string } }> {
    return Promise.resolve({ data: { code: 400, message: unavailableMessage } })
}
