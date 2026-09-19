type KeyFactory = (prefix: string) => string
type IntentStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

function defaultKeyFactory(prefix: string): string {
  const randomPart = globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2, 10)
  return `${prefix}-${Date.now()}-${randomPart}`
}

function canonicalValue(value: unknown): string {
  if (value === null || value === undefined) return 'none'
  if (typeof value === 'string') return value.trim()
  return String(value)
}

/** 按字段名排序并做长度前缀编码，同一业务意图在重试时得到同一缓存标识。 */
export function canonicalRequestIntent(values: Record<string, unknown>): string {
  return Object.keys(values)
    .sort()
    .flatMap((key) => [key, canonicalValue(values[key])])
    .map((part) => `${part.length}:${part}`)
    .join('')
}

/** 失败保留键、成功释放键；参数改变会自然切换到新意图和新键。 */
export class RetryStableIdempotencyKeys {
  private readonly pending = new Map<string, string>()
  private readonly prefix: string
  private readonly keyFactory: KeyFactory
  private readonly storage?: () => IntentStorage

  constructor(prefix: string, keyFactory: KeyFactory = defaultKeyFactory, storage?: () => IntentStorage) {
    this.prefix = prefix
    this.keyFactory = keyFactory
    this.storage = storage
  }

  acquire(intent: string): string {
    const storage = this.storage?.()
    if (this.storage && !storage) throw new Error('financial intent persistence is required')
    const storageKey = `financial-intent:${this.prefix}:${intent}`
    const persisted = storage?.getItem(storageKey)
    if (persisted) return persisted
    const existing = storage ? undefined : this.pending.get(intent)
    if (existing) return existing
    const key = this.keyFactory(this.prefix)
    // 持久化失败时禁止发起资金请求，避免刷新后遗失结果未知的身份。
    storage?.setItem(storageKey, key)
    if (!storage) this.pending.set(intent, key)
    return key
  }

  complete(intent: string, key: string): void {
    const storage = this.storage?.()
    if (this.storage && !storage) throw new Error('financial intent persistence is required')
    const storageKey = `financial-intent:${this.prefix}:${intent}`
    if (storage?.getItem(storageKey) === key) storage.removeItem(storageKey)
    if (this.pending.get(intent) === key) this.pending.delete(intent)
  }
}
