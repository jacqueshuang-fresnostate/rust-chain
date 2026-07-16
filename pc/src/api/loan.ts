import request from './request'
import { backendApiUrl, type PcApiResponse, type PcPageData } from './backendAdapters'

export type LoanType = 'credit' | 'collateralized'
export type InterestCalculationMode = 'full_term' | 'actual_days'
export type LoanOrderStatus = 'pending' | 'disbursed' | 'rejected' | 'cancelled' | 'repaid'

export interface LocalizedLoanNameItem {
    locale?: string
    country?: string
    country_code?: string
    title?: string
}

export interface LocalizedLoanNameDocument {
    version?: number
    default_locale?: string
    items?: LocalizedLoanNameItem[]
}

export enum OrderStatus {
    PROCESSING = 'pending',
    PULLING = 'disbursed',
    SUCCESS = 'repaid',
    FAIL = 'rejected',
    CANCEL = 'cancelled',
}

export interface LoanProduct {
    id: number
    loan_type: LoanType
    asset_id: number
    asset_symbol: string
    name: string
    name_json?: LocalizedLoanNameDocument | null
    term_days: number
    interest_rate: string | number
    interest_calculation_mode: InterestCalculationMode
    min_kyc_level: number
    min_amount: string | number
    max_amount?: string | number | null
    status: string
    created_at: number
    updated_at: number
}

export interface InstallmentsOrder {
    id: number
    user_id: number
    user_email?: string | null
    product_id: number
    product_name: string
    product_name_json?: LocalizedLoanNameDocument | null
    loan_type: LoanType
    asset_id: number
    asset_symbol: string
    amount: string | number
    interest_rate: string | number
    interest_calculation_mode: InterestCalculationMode
    term_days: number
    min_kyc_level: number
    collateral_asset_id?: number | null
    collateral_asset_symbol?: string | null
    collateral_amount?: string | number | null
    status: LoanOrderStatus
    interest_amount: string | number
    repayment_amount: string | number
    approved_at?: number | null
    rejected_at?: number | null
    disbursed_at?: number | null
    due_at?: number | null
    cancelled_at?: number | null
    repaid_at?: number | null
    collateral_released_at?: number | null
    created_at: number
    updated_at: number
}

export interface ApplyLoanPayload {
    productId: number
    amount: string | number
    collateralAssetId?: number
    collateralAmount?: string | number
}

interface BackendLoanProductsResponse {
    products?: LoanProduct[]
}

interface BackendLoanOrdersResponse {
    orders?: InstallmentsOrder[]
}

interface BackendLoanOrderActionResponse {
    order: InstallmentsOrder
    changed: boolean
}

let loanRequestSequence = 0

function decimalField(value: unknown, fallback = '0'): string | number {
    if (typeof value === 'string' || typeof value === 'number') return value
    if (value && typeof value === 'object') {
        const record = value as { value?: unknown; int_val?: unknown; scale?: unknown }
        if (typeof record.value === 'string' || typeof record.value === 'number') return record.value
        const intValue = Number(record.int_val)
        const scaleValue = Number(record.scale)
        if (Number.isFinite(intValue) && Number.isFinite(scaleValue)) return String(intValue / 10 ** scaleValue)
    }
    return fallback
}

function optionalDecimalField(value: unknown): string | number | null {
    if (value === null || value === undefined) return null
    return decimalField(value)
}

function normalizeLoanProduct(product: LoanProduct & Record<string, unknown>): LoanProduct {
    return {
        ...product,
        interest_rate: decimalField(product.interest_rate ?? product.interestRate ?? product.rate ?? product.depositsDays),
        max_amount: optionalDecimalField(product.max_amount ?? product.maxAmount),
        min_amount: decimalField(product.min_amount ?? product.minAmount ?? product.minInstallments),
    }
}

export async function fetchLoanProducts(): Promise<{ data: PcApiResponse<LoanProduct[]> }> {
    const res = await request.instance.get<BackendLoanProductsResponse>(backendApiUrl('/loan/products'))
    const products = Array.isArray(res.data.products) ? res.data.products.map((product) => normalizeLoanProduct(product as LoanProduct & Record<string, unknown>)) : []
    return {
        data: {
            code: 0,
            message: 'success',
            data: products,
        },
    }
}

export async function applyLoan(data: ApplyLoanPayload): Promise<{ data: PcApiResponse<BackendLoanOrderActionResponse> }> {
    const payload = {
        product_id: data.productId,
        amount: String(data.amount),
        collateral_asset_id: data.collateralAssetId,
        collateral_amount: data.collateralAmount === undefined ? undefined : String(data.collateralAmount),
        idempotency_key: `pc-loan-${Date.now()}-${loanRequestSequence += 1}`,
    }
    const res = await request.instance.post<BackendLoanOrderActionResponse>(backendApiUrl('/loan/orders'), payload)
    return {
        data: {
            code: 0,
            message: 'success',
            data: res.data,
        },
    }
}

export async function fetchLoanOrders(
    status?: LoanOrderStatus | OrderStatus,
    pageNo = 1,
    pageSize = 20,
): Promise<{ data: PcApiResponse<PcPageData<InstallmentsOrder>> }> {
    const res = await request.instance.get<BackendLoanOrdersResponse>(backendApiUrl('/loan/orders'), {
        params: {
            status: status || undefined,
            limit: pageSize,
        },
    })
    const orders = Array.isArray(res.data.orders) ? res.data.orders : []
    return {
        data: {
            code: 0,
            message: 'success',
            data: {
                content: orders,
                page: {
                    number: Math.max(pageNo - 1, 0),
                    size: pageSize,
                    totalElements: orders.length,
                    totalPages: Math.max(Math.ceil(orders.length / pageSize), 1),
                },
            },
        },
    }
}

export async function cancelLoanOrder(id: number): Promise<{ data: PcApiResponse<BackendLoanOrderActionResponse> }> {
    const res = await request.instance.post<BackendLoanOrderActionResponse>(backendApiUrl(`/loan/orders/${id}/cancel`))
    return {
        data: {
            code: 0,
            message: 'success',
            data: res.data,
        },
    }
}

export async function repayLoanOrder(id: number): Promise<{ data: PcApiResponse<BackendLoanOrderActionResponse> }> {
    const res = await request.instance.post<BackendLoanOrderActionResponse>(backendApiUrl(`/loan/orders/${id}/repay`))
    return {
        data: {
            code: 0,
            message: 'success',
            data: res.data,
        },
    }
}

export function confirmLoanOrder(id: number, _fundPassword?: string): Promise<{ data: PcApiResponse<BackendLoanOrderActionResponse> }> {
    return repayLoanOrder(id)
}

export function localizedLoanName(nameJson: LocalizedLoanNameDocument | null | undefined, fallback: string, currentLocale: string): string {
    const items = Array.isArray(nameJson?.items) ? nameJson.items : []
    const normalizedLocale = currentLocale.trim().toLowerCase()
    const normalizedLanguage = normalizedLocale.split('-')[0]
    const defaultLocale = typeof nameJson?.default_locale === 'string' ? nameJson.default_locale.trim().toLowerCase() : ''
    const titleFor = (item: LocalizedLoanNameItem | undefined) => {
        const title = typeof item?.title === 'string' ? item.title.trim() : ''
        return title || ''
    }
    const exactTitle = titleFor(items.find((item) => String(item.locale || '').trim().toLowerCase() === normalizedLocale))
    if (exactTitle) return exactTitle
    const languageTitle = titleFor(items.find((item) => String(item.locale || '').trim().toLowerCase().split('-')[0] === normalizedLanguage))
    if (languageTitle) return languageTitle
    const defaultTitle = titleFor(items.find((item) => String(item.locale || '').trim().toLowerCase() === defaultLocale))
    if (defaultTitle) return defaultTitle
    const firstTitle = titleFor(items[0])
    return firstTitle || fallback
}
