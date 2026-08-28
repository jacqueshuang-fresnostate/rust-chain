/** 手机端部分平仓允许的最小整数比例。 */
export const MARGIN_CLOSE_MIN_PERCENTAGE = 1
/** 手机端部分平仓允许的最大整数比例。 */
export const MARGIN_CLOSE_MAX_PERCENTAGE = 100

/**
 * 把滑杆输入归一为后端接受的整数百分比。
 *
 * 该值只描述用户意图；真实资金金额由后端基于事务内仓位和 BigDecimal 计算。
 */
export function normalizeMarginClosePercentage(value: number): number {
  if (!Number.isFinite(value)) return MARGIN_CLOSE_MAX_PERCENTAGE
  return Math.min(
    MARGIN_CLOSE_MAX_PERCENTAGE,
    Math.max(MARGIN_CLOSE_MIN_PERCENTAGE, Math.round(value)),
  )
}

/**
 * 计算平仓弹窗中的比例预览。
 *
 * 结果仅用于数量和预计收益展示，不回写钱包、仓位，也不进入请求金额字段；缺失或非有限
 * 服务端数据返回 null。有效结果压缩浮点尾噪，避免 UI 显示 0.23565299999999998。
 */
export function marginClosePreviewAmount(
  amount: number | null | undefined,
  percentage: number,
): number | null {
  if (amount === null || amount === undefined || !Number.isFinite(amount)) return null
  const normalizedPercentage = normalizeMarginClosePercentage(percentage)
  const result = amount * normalizedPercentage / 100
  return Number(result.toPrecision(15))
}
