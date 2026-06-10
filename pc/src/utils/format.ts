import numeral from 'numeral'

export type FormatType = 'price' | 'amount' | 'percent' | 'volume'

export function formatNumber(value: number | string, type: FormatType = 'price'): string {
  const val = Number(value)
  if (isNaN(val)) return '0'

  switch (type) {
    case 'price':
      if (val === 0) return '0.00'
      if (val < 0.1) return numeral(val).format('0.000000')
      if (val < 1) return numeral(val).format('0.0000')
      return numeral(val).format('0.00')
    case 'amount':
      if (val === 0) return '0.00'
      if (val < 1) return numeral(val).format('0.0000')
      return numeral(val).format('0.00')
    case 'percent':
      // numeral(0.05).format('0.00%') -> '5.00%'
      // Check if value is whole number representation (e.g., 5.32 for 5.32%) or decimal (0.0532)
      // Based on AGENT.md: "保留 2 位小数 + %"
      // If the API returns raw percentage (e.g., 5.32), we treat it as is.
      // If it's a ratio, we might need to multiply.
      // AGENT.md implies the final output should be `0.00%`.
      // Let's assume the input is ALREADY a number that just needs formatting to 2 decimal places + %.
      // BUT `numeral(val).format('0.00%')` expects a ratio (0.5 -> 50.00%).
      // If input is 5.32 (meaning 5.32%), numeral will output 532.00%.
      // Let's adjust based on typical API behavior or explicit instruction.
      // "保留 2 位小数 + % (0.00%)" suggests the format string.
      // If the input is 5.32, we want "5.32%".
      // We will assume the input is the percentage value itself, not the ratio, unless typical logic dictates otherwise.
      // However, numeral format '0.00%' multiplies by 100.
      // Let's stick to standard number formatting and append % manually in component or here.
      // To satisfy '0.00%', let's use:
      return numeral(val).format('0.00') + '%'
    case 'volume':
      return numeral(val).format('0.00a').toUpperCase()
    default:
      return numeral(val).format('0.00')
  }
}
