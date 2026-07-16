import request from './request'
import { backendApiUrl, mapPcWithdrawalRequest } from './backendAdapters'

// --- Types ---

// Coin configuration per network
export interface CoinNetwork {
    name: string      // e.g. "ERC20", "TRC20", "BTC"
    networkKey?: string
    baseCoin: string  // e.g. "ETH", "TRX", "BTC"
    addressRegex: string
    memoRegex?: string
    minRechargeAmount: number
    depositFee: number
    withdrawFee: number
    withdrawFeeTiers: WithdrawFeeTier[]
    minWithdrawAmount: number
    maxWithdrawAmount: number
    depositEnabled: boolean
    withdrawEnabled: boolean
}

export interface WithdrawFeeTier {
    minAmount: number
    maxAmount?: number
    feeRatePercent: number
}

export interface WalletAddress {
    address: string
    qrcode: string
    unit: string
    network?: string // The selected network
    coin: CoinNetwork // Configuration for this specific network
}

export interface WithdrawParams {
    unit: string
    network?: string // Selected network
    address: string
    amount: number
    fee: number
    code: string // email/sms code
    fundPassword?: string
    totpCode?: string
}

interface BackendDepositAddressResponse {
    id: number
    asset_symbol: string
    network: string
    address: string
    memo?: string | null
    assigned_at: number
}

interface BackendDepositAssetResponse {
    symbol: string
    deposit_enabled?: boolean | null
    withdraw_enabled?: boolean | null
    min_deposit_amount?: string | number | null
    deposit_fee?: string | number | null
    withdraw_fee?: string | number | null
    withdraw_fee_tiers?: BackendWithdrawFeeTier[] | null
}

interface BackendWithdrawFeeTier {
    min_amount?: string | number | null
    max_amount?: string | number | null
    fee_rate_percent?: string | number | null
}

interface BackendDepositNetworkResponse {
    network: string
    display_name: string
    address_group_code: string
    address_group_name?: string | null
    asset_symbols?: string[] | null
}

export type AssetPurpose = 'deposit' | 'withdraw'

export interface QuickRechargeConfig {
    enabled: boolean
    currency: string
    token: string
    network: string
    min_amount: string
    max_amount?: string | null
}

export type QuickRechargeReturnTarget =
    | 'pc_app'
    | 'mac_app'
    | 'ios_app'
    | 'android_app'
    | 'mobile_web'
    | 'desktop_web'

export interface QuickRechargeOrder {
    id: number
    order_id: string
    asset_symbol: string
    currency: string
    token: string
    network: string
    fiat_amount: string
    actual_amount?: string | null
    provider_trade_id?: string | null
    receive_address?: string | null
    payment_url?: string | null
    return_target?: QuickRechargeReturnTarget | null
    redirect_url?: string | null
    expiration_time?: number | null
    status: string
    block_transaction_id?: string | null
    paid_at?: number | null
    created_at: number
    updated_at: number
}

// --- API Functions ---

export async function fetchSupportedCoins(): Promise<{ data: { code: number, message: string, data: string[] } }> {
    return fetchDepositCoins()
}

export async function fetchDepositCoins(): Promise<{ data: { code: number, message: string, data: string[] } }> {
    const assets = await fetchBackendWalletAssets('deposit')
    return normalizeCoinListResponse(assets)
}

export async function fetchWithdrawCoins(): Promise<{ data: { code: number, message: string, data: string[] } }> {
    const assets = await fetchBackendWalletAssets('withdraw')
    return normalizeCoinListResponse(assets)
}

function normalizeCoinListResponse(assets: BackendDepositAssetResponse[]): { data: { code: number, message: string, data: string[] } } {
    const symbols = [...new Set(assets.map((asset) => asset.symbol.toUpperCase()).filter(Boolean))]
    return {
        data: {
            code: 0,
            message: 'success',
            data: symbols,
        }
    }
}

export async function fetchCoinNetworks(unit: string, purpose: AssetPurpose = 'deposit'): Promise<{ data: { code: number, message: string, data: CoinNetwork[] } }> {
    const asset = await fetchAssetSetting(unit, purpose)
    if (purpose === 'deposit') {
        try {
            const networks = await fetchBackendDepositNetworks(unit)
            if (networks.length > 0) {
                return {
                    data: {
                        code: 0,
                        message: 'success',
                        data: networks.map((network) => defaultNetworkFromConfig(unit, network, asset)),
                    }
                }
            }
        } catch {
            // Fall back to the built-in network list so the recharge page stays usable during local setup.
        }
    }
    return {
        data: {
            code: 0,
            message: 'success',
            data: supportedDepositNetworks(unit).map((network) => defaultNetwork(unit, network, asset)),
        }
    }
}

export async function getDepositAddress(unit: string, network?: string): Promise<{ data: { code: number, message: string, data: WalletAddress | null } }> {
    const networkValue = backendNetworkValue(network || supportedDepositNetworks(unit)[0])
    const res = await request.instance.post<BackendDepositAddressResponse>(backendApiUrl('/wallet/deposit-address'), {
        asset_symbol: unit.toUpperCase(),
        network: networkValue,
    })
    const asset = await fetchAssetSetting(res.data.asset_symbol, 'deposit')
    const coin = defaultNetwork(res.data.asset_symbol, depositNetworkLabel(res.data.network), asset)
    return {
        data: {
            code: 0,
            message: 'success',
            data: {
                address: res.data.address,
                qrcode: res.data.address,
                unit: res.data.asset_symbol,
                network: coin.name,
                coin,
            },
        }
    }
}

export async function fetchQuickRechargeConfig(): Promise<{ data: { code: number, message: string, data: QuickRechargeConfig } }> {
    const res = await request.instance.get<QuickRechargeConfig>(backendApiUrl('/wallet/quick-recharge/config'))
    return {
        data: {
            code: 0,
            message: 'success',
            data: res.data,
        }
    }
}

export async function createQuickRechargeOrder(
    amount: string | number,
    returnTarget?: QuickRechargeReturnTarget,
): Promise<{ data: { code: number, message: string, data: QuickRechargeOrder } }> {
    const payload: { amount: string; return_target?: QuickRechargeReturnTarget } = {
        amount: String(amount),
    }
    if (returnTarget) payload.return_target = returnTarget
    const res = await request.instance.post<QuickRechargeOrder>(backendApiUrl('/wallet/quick-recharge/orders'), payload)
    return {
        data: {
            code: 0,
            message: 'success',
            data: res.data,
        }
    }
}

export async function fetchQuickRechargeOrders(limit = 20): Promise<{ data: { code: number, message: string, data: QuickRechargeOrder[] } }> {
    const res = await request.instance.get<{ orders?: QuickRechargeOrder[] }>(backendApiUrl('/wallet/quick-recharge/orders'), {
        params: { limit },
    })
    return {
        data: {
            code: 0,
            message: 'success',
            data: Array.isArray(res.data?.orders) ? res.data.orders : [],
        }
    }
}

export async function getNetworkInfo(unit: string, network?: string, purpose: AssetPurpose = 'deposit'): Promise<{ data: { code: number, message: string, data: WalletAddress | null } }> {
    const asset = await fetchAssetSetting(unit, purpose)
    const coin = defaultNetwork(unit, depositNetworkLabel(backendNetworkValue(network || supportedDepositNetworks(unit)[0])), asset)
    return {
        data: {
            code: 0,
            message: 'success',
            data: {
                address: '',
                qrcode: '',
                unit: unit.toUpperCase(),
                network: coin.name,
                coin,
            },
        }
    }
}

export async function submitWithdraw(params: WithdrawParams): Promise<{ data: { code: number, message: string, data?: unknown } }> {
    const res = await request.instance.post(backendApiUrl('/wallet/withdrawals'), mapPcWithdrawalRequest(params))
    return {
        data: {
            code: 0,
            message: 'success',
            data: res.data,
        }
    }
}

function supportedDepositNetworks(unit: string): string[] {
    const symbol = unit.trim().toUpperCase()
    if (symbol === 'BTC') return ['BTC']
    if (symbol === 'ETH') return ['ETH', 'Base']
    if (symbol === 'TRX' || symbol === 'TRON') return ['Tron']
    if (symbol === 'SOL' || symbol === 'SOLANA') return ['Solana']
    if (symbol === 'USDT') return ['ETH', 'Base', 'BTC', 'Tron', 'Solana']
    if (symbol === 'USDC') return ['ETH', 'Base', 'Tron', 'Solana']
    return ['ETH', 'Base', 'Tron', 'BTC', 'Solana']
}

function backendNetworkValue(network: string): string {
    const normalized = network.trim().toLowerCase()
    if (normalized === 'ethereum' || normalized === 'erc20') return 'eth'
    if (normalized === 'trx' || normalized === 'trc20') return 'tron'
    if (normalized === 'bitcoin') return 'btc'
    if (normalized === 'sol') return 'solana'
    if (normalized === 'base') return 'base'
    if (normalized === 'tron') return 'tron'
    if (normalized === 'btc') return 'btc'
    if (normalized === 'solana') return 'solana'
    return 'eth'
}

function depositNetworkLabel(network: string): string {
    switch (backendNetworkValue(network)) {
        case 'eth':
            return 'ETH'
        case 'base':
            return 'Base'
        case 'tron':
            return 'Tron'
        case 'btc':
            return 'BTC'
        case 'solana':
            return 'Solana'
        default:
            return network
    }
}

function networkBaseCoin(network: string, unit: string): string {
    switch (backendNetworkValue(network)) {
        case 'eth':
        case 'base':
            return 'ETH'
        case 'tron':
            return 'TRX'
        case 'btc':
            return 'BTC'
        case 'solana':
            return 'SOL'
        default:
            return unit.toUpperCase()
    }
}

function networkAddressRegex(network: string): string {
    switch (backendNetworkValue(network)) {
        case 'eth':
        case 'base':
            return '^0x[a-fA-F0-9]{40}$'
        case 'tron':
            return '^T[1-9A-HJ-NP-Za-km-z]{33}$'
        case 'btc':
            return '^(bc1|[13])[a-zA-HJ-NP-Z0-9]{25,64}$'
        case 'solana':
            return '^[1-9A-HJ-NP-Za-km-z]{32,44}$'
        default:
            return '.+'
    }
}

async function fetchBackendWalletAssets(purpose: AssetPurpose): Promise<BackendDepositAssetResponse[]> {
    const endpoint = purpose === 'withdraw' ? '/wallet/withdraw-assets' : '/wallet/deposit-assets'
    const res = await request.instance.get<{ assets?: BackendDepositAssetResponse[] }>(backendApiUrl(endpoint))
    return Array.isArray(res.data?.assets) ? res.data.assets : []
}

async function fetchBackendDepositNetworks(unit: string): Promise<BackendDepositNetworkResponse[]> {
    const res = await request.instance.get<{ networks?: BackendDepositNetworkResponse[] }>(backendApiUrl('/wallet/deposit-networks'), {
        params: { asset_symbol: unit.trim().toUpperCase() },
    })
    return Array.isArray(res.data?.networks) ? res.data.networks : []
}

async function fetchAssetSetting(unit: string, purpose: AssetPurpose): Promise<BackendDepositAssetResponse | undefined> {
    const symbol = unit.trim().toUpperCase()
    const assets = await fetchBackendWalletAssets(purpose)
    return assets.find((asset) => asset.symbol.toUpperCase() === symbol)
}

function amountToNumber(value: string | number | null | undefined, fallback = 0): number {
    const amount = Number(value)
    return Number.isFinite(amount) ? amount : fallback
}

function normalizeWithdrawFeeTiers(asset?: BackendDepositAssetResponse): WithdrawFeeTier[] {
    if (!Array.isArray(asset?.withdraw_fee_tiers)) return []
    return asset.withdraw_fee_tiers
        .map((tier) => ({
            minAmount: amountToNumber(tier.min_amount),
            maxAmount: tier.max_amount === null || tier.max_amount === undefined ? undefined : amountToNumber(tier.max_amount),
            feeRatePercent: amountToNumber(tier.fee_rate_percent),
        }))
        .filter((tier) => tier.minAmount >= 0 && tier.feeRatePercent >= 0 && (tier.maxAmount === undefined || tier.maxAmount > tier.minAmount))
        .sort((left, right) => left.minAmount - right.minAmount)
}

export function calculateWithdrawFee(amount: number, coin?: Pick<CoinNetwork, 'withdrawFee' | 'withdrawFeeTiers'>): number {
    if (!coin) return 0
    const value = Number(amount)
    if (!Number.isFinite(value) || value <= 0) return coin.withdrawFee
    const tier = coin.withdrawFeeTiers.find((item) => value >= item.minAmount && (item.maxAmount === undefined || value < item.maxAmount))
    if (!tier) return coin.withdrawFee
    return (value * tier.feeRatePercent) / 100
}

function defaultNetwork(unit: string, network = 'ETH', asset?: BackendDepositAssetResponse): CoinNetwork {
    const symbol = unit.toUpperCase()
    const label = depositNetworkLabel(network)
    return {
        name: label,
        networkKey: backendNetworkValue(network),
        baseCoin: networkBaseCoin(label, symbol),
        addressRegex: networkAddressRegex(label),
        minRechargeAmount: amountToNumber(asset?.min_deposit_amount),
        depositFee: amountToNumber(asset?.deposit_fee),
        withdrawFee: amountToNumber(asset?.withdraw_fee),
        withdrawFeeTiers: normalizeWithdrawFeeTiers(asset),
        minWithdrawAmount: 0,
        maxWithdrawAmount: Number.MAX_SAFE_INTEGER,
        depositEnabled: asset?.deposit_enabled !== false,
        withdrawEnabled: asset?.withdraw_enabled !== false,
    }
}

function defaultNetworkFromConfig(unit: string, config: BackendDepositNetworkResponse, asset?: BackendDepositAssetResponse): CoinNetwork {
    const symbol = unit.toUpperCase()
    const networkKey = backendNetworkValue(config.network)
    const label = config.display_name?.trim() || depositNetworkLabel(networkKey)
    return {
        name: label,
        networkKey,
        baseCoin: networkBaseCoin(networkKey, symbol),
        addressRegex: networkAddressRegex(networkKey),
        minRechargeAmount: amountToNumber(asset?.min_deposit_amount),
        depositFee: amountToNumber(asset?.deposit_fee),
        withdrawFee: amountToNumber(asset?.withdraw_fee),
        withdrawFeeTiers: normalizeWithdrawFeeTiers(asset),
        minWithdrawAmount: 0,
        maxWithdrawAmount: Number.MAX_SAFE_INTEGER,
        depositEnabled: asset?.deposit_enabled !== false,
        withdrawEnabled: asset?.withdraw_enabled !== false,
    }
}
