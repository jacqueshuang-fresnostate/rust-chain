// --- Types ---

// Coin configuration per network
export interface CoinNetwork {
    name: string      // e.g. "ERC20", "TRC20", "BTC"
    baseCoin: string  // e.g. "ETH", "TRX", "BTC"
    addressRegex: string
    memoRegex?: string
    minRechargeAmount: number
    withdrawFee: number
    minWithdrawAmount: number
    maxWithdrawAmount: number
    depositEnabled: boolean
    withdrawEnabled: boolean
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
}

// --- API Functions ---

const unavailableMessage = '当前后端暂未开放链上充值和提现接口'

export function fetchSupportedCoins(): Promise<{ data: { code: number, message: string, data: string[] } }> {
    return Promise.resolve({
        data: {
            code: 400,
            message: unavailableMessage,
            data: []
        }
    })
}

export function fetchCoinNetworks(_unit: string): Promise<{ data: { code: number, message: string, data: CoinNetwork[] } }> {
    return Promise.resolve({
        data: {
            code: 400,
            message: unavailableMessage,
            data: []
        }
    })
}

export function getDepositAddress(_unit: string, _network?: string): Promise<{ data: { code: number, message: string, data: WalletAddress | null } }> {
    return Promise.resolve({
        data: {
            code: 400,
            message: unavailableMessage,
            data: null
        }
    })
}

export function submitWithdraw(_params: WithdrawParams): Promise<{ data: { code: number, message: string } }> {
    return Promise.resolve({
        data: {
            code: 400,
            message: unavailableMessage
        }
    })
}
