/** 滑动确认达到该归一化进度后才允许提交，避免临界抖动造成金融操作误触。 */
export const SLIDE_CONFIRM_THRESHOLD = 0.9

function clampUnitInterval(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(1, Math.max(0, value))
}

/**
 * 把指针横坐标映射为滑块的 0..1 进度。
 *
 * 起止点按手柄中心计算，确保手柄视觉边缘始终留在轨道的左右内边距内；异常或退化尺寸
 * 返回 0，避免布局尚未完成时把一次触摸误判为确认。
 */
export function slideProgressFromClientX(
  clientX: number,
  trackLeft: number,
  trackWidth: number,
  handleWidth: number,
  inset: number,
): number {
  const start = trackLeft + inset + handleWidth / 2
  const end = trackLeft + trackWidth - inset - handleWidth / 2
  const travel = end - start
  if (![clientX, trackLeft, trackWidth, handleWidth, inset, travel].every(Number.isFinite) || travel <= 0) {
    return 0
  }
  return clampUnitInterval((clientX - start) / travel)
}

/** 判断当前进度是否已经越过金融操作的显式确认阈值。 */
export function isSlideConfirmComplete(
  progress: number,
  threshold = SLIDE_CONFIRM_THRESHOLD,
): boolean {
  return clampUnitInterval(progress) >= clampUnitInterval(threshold)
}

/**
 * 为可聚焦滑块提供原生 range 一致的键盘语义。
 * 返回 null 表示当前按键不属于滑动控件，调用方应继续让浏览器处理。
 */
export function slideProgressForKey(
  current: number,
  key: string,
  step = 0.1,
): number | null {
  if (key === 'Home') return 0
  if (key === 'End') return 1
  if (key === 'ArrowRight' || key === 'ArrowUp') {
    return clampUnitInterval(Number((current + step).toFixed(6)))
  }
  if (key === 'ArrowLeft' || key === 'ArrowDown') {
    return clampUnitInterval(Number((current - step).toFixed(6)))
  }
  return null
}
