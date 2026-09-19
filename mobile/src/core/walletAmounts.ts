import { decimalAdd, normalizeDecimalText, requiredDecimalText, type DecimalText } from './decimal.ts'
import type { WalletAccount } from './types.ts'

export function walletTotal(account?: WalletAccount): DecimalText {
  return decimalAdd(walletAvailable(account), walletFrozen(account))
}

export function walletAvailable(account?: WalletAccount): DecimalText {
  return account ? requiredDecimalText(account.availableText, 'available', 'wallet') : normalizeDecimalText('0')
}

export function walletFrozen(account?: WalletAccount): DecimalText {
  return account
    ? decimalAdd(requiredDecimalText(account.frozenText, 'frozen', 'wallet'), requiredDecimalText(account.lockedText, 'locked', 'wallet'))
    : normalizeDecimalText('0')
}
