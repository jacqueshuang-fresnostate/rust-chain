export function newCoinPurchaseQuantity(
  quoteAvailable: number,
  percentage: number,
  executionPrice: number,
): number {
  if (!Number.isFinite(quoteAvailable) || quoteAvailable <= 0) return 0
  if (!Number.isFinite(percentage) || percentage <= 0) return 0
  if (!Number.isFinite(executionPrice) || executionPrice <= 0) return 0
  return quoteAvailable * Math.min(percentage, 1) / executionPrice
}
