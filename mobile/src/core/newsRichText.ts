export type NewsRichTextBlockType = 'p' | 'h1' | 'h2' | 'h3' | 'blockquote'

export interface NewsRichTextLeaf {
  text: string
  bold: boolean
  italic: boolean
  underline: boolean
  href?: string
}

export type NewsRichTextBlock =
  | { type: NewsRichTextBlockType; children: NewsRichTextLeaf[] }
  | { type: 'image'; url: string; alt: string }

export function selectLocalizedNewsRichText(value: unknown, locale: string): NewsRichTextBlock[] {
  if (typeof value === 'string') return normalizeNewsRichText(value)
  if (!value || typeof value !== 'object') return []
  const document = value as { default_locale?: unknown; items?: unknown }
  const items = Array.isArray(document.items) ? document.items : []
  const normalizedLocale = normalizeLocale(locale)
  const language = normalizedLocale.split('-')[0]
  const preferred = normalizeLocale(document.default_locale)
  const itemLocale = (item: unknown) => normalizeLocale((item as { locale?: unknown }).locale)
  const selected = (items.find((item) => itemLocale(item) === normalizedLocale)
    || items.find((item) => itemLocale(item).split('-')[0] === language)
    || items.find((item) => itemLocale(item) === preferred)
    || items[0]) as { content?: unknown } | undefined
  return normalizeNewsRichText(selected?.content)
}

export function normalizeNewsRichText(value: unknown): NewsRichTextBlock[] {
  if (typeof value === 'string') {
    const text = value.trim()
    return text ? [{ type: 'p', children: [normalizeLeaf({ text })] }] : []
  }
  if (!Array.isArray(value)) return []
  return value.flatMap((entry): NewsRichTextBlock[] => {
    if (!entry || typeof entry !== 'object') return []
    const block = entry as Record<string, unknown>
    if (block.type === 'image') {
      const url = safeNewsResourceUrl(block.url)
      return url ? [{ type: 'image', url, alt: typeof block.alt === 'string' ? block.alt.trim() : '' }] : []
    }
    const type = normalizeBlockType(block.type)
    const children = Array.isArray(block.children)
      ? block.children
        .filter((child): child is Record<string, unknown> => Boolean(child && typeof child === 'object'))
        .map(normalizeLeaf)
      : []
    return [{ type, children }]
  })
}

function normalizeLeaf(leaf: Record<string, unknown>): NewsRichTextLeaf {
  const link = leaf.href ?? leaf.link ?? leaf.url
  return {
    text: typeof leaf.text === 'string' ? leaf.text : '',
    bold: leaf.bold === true,
    italic: leaf.italic === true,
    underline: leaf.underline === true,
    href: safeNewsResourceUrl(link),
  }
}

function normalizeBlockType(value: unknown): NewsRichTextBlockType {
  return value === 'h1' || value === 'h2' || value === 'h3' || value === 'blockquote' ? value : 'p'
}

function normalizeLocale(value: unknown): string {
  return String(value || '').trim().replace('_', '-').toLowerCase()
}

function safeNewsResourceUrl(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const url = value.trim()
  if (!url) return undefined
  if (url.startsWith('/') && !url.startsWith('//')) return url
  try {
    const parsed = new URL(url)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.toString() : undefined
  } catch {
    return undefined
  }
}
