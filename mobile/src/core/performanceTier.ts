export type PerformanceTier = 'standard' | 'constrained'

export interface DevicePerformanceSignals {
  saveData?: unknown
  deviceMemory?: unknown
  hardwareConcurrency?: unknown
}

/**
 * 只把浏览器明确暴露的节流或低配信号判为受限档；API 缺失、异常值与桌面隐私降级均回退标准档。
 */
export function resolvePerformanceTier(signals: DevicePerformanceSignals): PerformanceTier {
  if (signals.saveData === true) return 'constrained'

  const deviceMemory = positiveFiniteNumber(signals.deviceMemory)
  const hardwareConcurrency = positiveFiniteNumber(signals.hardwareConcurrency)
  if (deviceMemory !== null && deviceMemory <= 2) return 'constrained'
  if (hardwareConcurrency !== null && hardwareConcurrency <= 2) return 'constrained'
  if (
    deviceMemory !== null
    && hardwareConcurrency !== null
    && deviceMemory <= 4
    && hardwareConcurrency <= 4
  ) return 'constrained'
  return 'standard'
}

/** 从 Navigator 的可选能力字段读取信号；旧 WebView 不支持这些字段时不会误判。 */
export function detectPerformanceTier(navigatorLike: object | null | undefined): PerformanceTier {
  if (!navigatorLike) return 'standard'
  const source = navigatorLike as {
    connection?: { saveData?: unknown }
    deviceMemory?: unknown
    hardwareConcurrency?: unknown
  }
  return resolvePerformanceTier({
    saveData: source.connection?.saveData,
    deviceMemory: source.deviceMemory,
    hardwareConcurrency: source.hardwareConcurrency,
  })
}

function positiveFiniteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null
}
