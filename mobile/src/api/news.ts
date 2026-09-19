import { client, requestUrl } from './client'
import type { NewsItem } from '@/core/types'
import { selectLocalizedNewsRichText, type NewsRichTextBlock } from '@/core/newsRichText'
import { currentApiLocale, i18n } from '@/i18n'
import { requiredId, requiredSafeInteger, normalizeTimestamp } from '@/core/numeric'

export type { NewsItem }

export interface NewsDetail extends NewsItem {
  category: string
  content: NewsRichTextBlock[]
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
  requiredSafeInteger(limit, 'limit', 1)
  const locale = currentApiLocale()
  const response = await client.get<{ news?: BackendNewsItem[] }>(requestUrl('/news'), {
    params: { limit, locale },
  })
  return (response.data.news || []).map((item) => ({
    id: requiredId(item.id),
    title: item.title,
    category: item.category || undefined,
    bannerUrl: item.banner_url || undefined,
    publishedAt: normalizeTimestamp(item.published_at) || undefined,
  }))
}

export async function fetchNewsDetail(id: number): Promise<NewsDetail> {
  requiredId(id)
  const locale = currentApiLocale()
  const response = await client.get<BackendNewsItem>(requestUrl(`/news/${id}`), { params: { locale } })
  return {
    id: requiredId(response.data.id),
    title: response.data.title,
    publishedAt: normalizeTimestamp(response.data.published_at) || undefined,
    category: response.data.category || i18n.global.t('news.title'),
    content: selectLocalizedNewsRichText(response.data.content_json, locale),
    bannerUrl: response.data.banner_url || undefined,
  }
}
