export interface SocketTimeoutScheduler {
  setTimeout(callback: () => void, delay: number): unknown
  clearTimeout(handle: unknown): void
}

export interface InboundSilenceWatchdog {
  arm(onTimeout: () => void): void
  clear(): void
}

/**
 * 创建一次性入站静默看门狗。每次收到任意 WebSocket 帧都重新 arm；旧定时回调即使已经进入
 * 任务队列，也会因 generation 不匹配而失效，不能关闭后续已经恢复活性的连接。
 */
export function createInboundSilenceWatchdog(
  scheduler: SocketTimeoutScheduler,
  timeoutMs: number,
): InboundSilenceWatchdog {
  let timer: unknown = null
  let generation = 0

  const clear = (): void => {
    generation += 1
    if (timer === null) return
    scheduler.clearTimeout(timer)
    timer = null
  }

  const arm = (onTimeout: () => void): void => {
    clear()
    const expectedGeneration = generation
    timer = scheduler.setTimeout(() => {
      if (expectedGeneration !== generation) return
      timer = null
      onTimeout()
    }, timeoutMs)
  }

  return { arm, clear }
}
