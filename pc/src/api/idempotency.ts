type KeyFactory = (prefix: string) => string

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

  constructor(prefix: string, keyFactory: KeyFactory = defaultKeyFactory) {
    this.prefix = prefix
    this.keyFactory = keyFactory
  }

  acquire(intent: string): string {
    const existing = this.pending.get(intent)
    if (existing) return existing
    const key = this.keyFactory(this.prefix)
    this.pending.set(intent, key)
    return key
  }

  complete(intent: string, key: string): void {
    if (this.pending.get(intent) === key) this.pending.delete(intent)
  }
}
