export interface ReferenceRequestOptions {
  force?: boolean
}

export interface MemoryRequestRegistry {
  request<T>(
    key: string,
    ttlMs: number,
    loader: () => Promise<T>,
    options?: ReferenceRequestOptions,
  ): Promise<T>
  invalidate(key?: string): void
}

type CacheEntry = {
  expiresAt: number
  value: unknown
}

/**
 * 仅缓存调用点明确白名单化的内存参考数据；TTL 从成功响应完成时起算，错误与取消不会落缓存。
 */
export function createMemoryRequestRegistry(
  now: () => number = Date.now,
): MemoryRequestRegistry {
  const cache = new Map<string, CacheEntry>()
  const inFlight = new Map<string, Promise<unknown>>()
  const keyGenerations = new Map<string, number>()
  let globalGeneration = 0

  async function request<T>(
    key: string,
    ttlMs: number,
    loader: () => Promise<T>,
    options: ReferenceRequestOptions = {},
  ): Promise<T> {
    const pending = inFlight.get(key) as Promise<T> | undefined
    if (pending) return cloneReferenceValue(await pending)

    const cached = cache.get(key)
    if (!options.force && cached && cached.expiresAt > now()) {
      return cloneReferenceValue(cached.value as T)
    }
    if (cached) cache.delete(key)

    const requestGlobalGeneration = globalGeneration
    const requestKeyGeneration = keyGenerations.get(key) || 0
    let current: Promise<T>
    current = loader()
      .then((value) => {
        if (
          globalGeneration === requestGlobalGeneration
          && (keyGenerations.get(key) || 0) === requestKeyGeneration
        ) {
          cache.set(key, {
            expiresAt: now() + Math.max(0, ttlMs),
            value: cloneReferenceValue(value),
          })
        }
        return value
      })
      .finally(() => {
        if (inFlight.get(key) === current) inFlight.delete(key)
      })
    inFlight.set(key, current)
    return cloneReferenceValue(await current)
  }

  function invalidate(key?: string): void {
    if (key === undefined) {
      globalGeneration += 1
      cache.clear()
      inFlight.clear()
      return
    }
    keyGenerations.set(key, (keyGenerations.get(key) || 0) + 1)
    cache.delete(key)
    inFlight.delete(key)
  }

  return { request, invalidate }
}

/** 参数按键名排序，避免对象构造顺序让相同目录请求落入不同缓存槽。 */
export function createReferenceRequestKey(
  url: string,
  params: Readonly<Record<string, unknown>> = {},
  scope = 'public',
): string {
  return `${scope}:${url}:${stableSerialize(params)}`
}

export const referenceRequestRegistry = createMemoryRequestRegistry()

function stableSerialize(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableSerialize).join(',')}]`
  if (value && typeof value === 'object') {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => `${JSON.stringify(key)}:${stableSerialize(entry)}`)
      .join(',')}}`
  }
  return JSON.stringify(value) ?? 'undefined'
}

function cloneReferenceValue<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((entry) => cloneReferenceValue(entry)) as T
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .map(([key, entry]) => [key, cloneReferenceValue(entry)]),
    ) as T
  }
  return value
}
