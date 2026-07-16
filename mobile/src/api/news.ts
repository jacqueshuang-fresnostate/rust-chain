import { client, requestUrl } from './client'
import type { NewsItem } from '@/core/types'
import { currentApiLocale, i18n } from '@/i18n'

export type { NewsItem }

export interface NewsDetail extends NewsItem {
  category: string
  content: string
  bannerUrl?: string
}

interface BackendNewsItem {
  id: number
  title: string
  category?: string | null
  banner_url?: string | null
  published_at?: number | null
  content_json?: unknown
}

export async function fetchNews(limit = 3): Promise<NewsItem[]> {
  const locale = currentApiLocale()
  const response = await client.get<{ news?: BackendNewsItem[] }>(requestUrl('/news'), {
    params: { limit, locale },
  })
  return (response.data.news || []).map((item) => ({
    id: item.id,
    title: item.title,
    publishedAt: item.published_at || undefined,
  }))
}

export async function fetchNewsDetail(id: number): Promise<NewsDetail> {
  const locale = currentApiLocale()
  const response = await client.get<BackendNewsItem>(requestUrl(`/news/${id}`), { params: { locale } })
  return {
    id: response.data.id,
    title: response.data.title,
    publishedAt: response.data.published_at || undefined,
    category: response.data.category || i18n.global.t('news.title'),
    content: extractNewsContent(response.data.content_json, locale),
    bannerUrl: response.data.banner_url || undefined,
  }
}

function extractNewsContent(value: unknown, locale: string): string {
  if (typeof value === 'string') return value.trim()
  if (!value || typeof value !== 'object') return ''
  const document = value as { default_locale?: unknown; items?: unknown }
  const items = Array.isArray(document.items) ? document.items : []
  const normalizedLocale = locale.trim().toLowerCase()
  const language = normalizedLocale.split('-')[0]
  const preferred = String(document.default_locale || '').trim().toLowerCase()
  const itemLocale = (item: unknown) => String((item as { locale?: unknown }).locale || '').trim().toLowerCase()
  const content = (items.find((item) => itemLocale(item) === normalizedLocale)
    || items.find((item) => itemLocale(item).split('-')[0] === language)
    || items.find((item) => itemLocale(item) === preferred)
    || items[0]) as { content?: unknown } | undefined
  if (typeof content?.content === 'string') return content.content.trim()
  if (!Array.isArray(content?.content)) return ''
  return content.content.map((block) => {
    const children = Array.isArray((block as { children?: unknown }).children) ? (block as { children: Array<{ text?: unknown }> }).children : []
    return children.map((child) => String(child.text || '')).join('')
  }).filter(Boolean).join('\n\n')
}
