/** Numeric IDs retain the existing JSON number contract, without rounded identities. */
export function requiredSafeInteger(value: unknown, field = 'integer', minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number {
  if (typeof value === 'string' && !/^\d+$/.test(value.trim())) {
    throw new TypeError(`invalid ${field}`)
  }
  const parsed = typeof value === 'string' ? Number(value.trim()) : value
  if (typeof parsed !== 'number' || !Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    throw new TypeError(`invalid ${field}`)
  }
  return parsed
}

export function requiredId(value: unknown, field = 'id'): number {
  return requiredSafeInteger(value, field, 1)
}

/** String/opaque identities stay strings; numeric identities must already be safe. */
export function requiredIdentityText(value: unknown, field = 'id'): string {
  if (typeof value === 'number') return String(requiredId(value, field))
  if (typeof value !== 'string' || !value.trim()) throw new TypeError(`invalid ${field}`)
  return value.trim()
}

export const MAX_DATE_TIMESTAMP = 8_640_000_000_000_000

/** Legacy endpoints allow seconds or milliseconds, but never fractional/unsafe epochs. */
export function normalizeTimestamp(value: unknown): number {
  if (value === null || value === undefined) return 0
  const timestamp = requiredSafeInteger(value, 'timestamp', 0, MAX_DATE_TIMESTAMP)
  const milliseconds = timestamp > 0 && timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
  return requiredSafeInteger(milliseconds, 'timestamp', 0, MAX_DATE_TIMESTAMP)
}

/** Reject already-rounded numeric JSON tokens before a passthrough DTO can use them. */
export function assertSafeJsonNumbers(value: unknown): void {
  const pending: unknown[] = [value]
  const seen = new Set<object>()
  while (pending.length) {
    const item = pending.pop()
    if (typeof item === 'number' && (!Number.isFinite(item) || Math.abs(item) > Number.MAX_SAFE_INTEGER)) {
      throw new TypeError('unsafe JSON number')
    }
    if (!item || typeof item !== 'object' || seen.has(item)) continue
    seen.add(item)
    if (Array.isArray(item)) {
      for (const entry of item) pending.push(entry)
    } else if (Object.getPrototypeOf(item) === Object.prototype) {
      for (const [key, entry] of Object.entries(item)) {
        if (typeof entry === 'number' && /(^id$|_id$|_count$|^(limit|offset|total|total_elements|total_pages|size|number|term_days|duration_seconds|precision_scale)$)/.test(key)) {
          requiredSafeInteger(entry, key)
        }
        if (typeof entry === 'number' && /_at$/.test(key)) {
          requiredSafeInteger(entry, key, 0, MAX_DATE_TIMESTAMP)
        }
        pending.push(entry)
      }
    }
  }
}
