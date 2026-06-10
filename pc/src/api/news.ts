import request from './request'
import {
  backendApiUrl,
  mapPublicNewsItemsToPcNewsCards,
  type BackendPublicNewsItem,
  type BackendPublicNewsItemsResponse,
} from './backendAdapters'

export interface FetchPublicNewsParams {
  category?: string
  countryCode?: string
  locale?: string
  q?: string
  limit?: number
  offset?: number
}

export async function fetchPublicNews(params: FetchPublicNewsParams = {}): Promise<{ data: any }> {
  const response = await request.instance.get<BackendPublicNewsItemsResponse>(backendApiUrl('/news'), {
    params: normalizeNewsParams(params),
  })
  return {
    data: {
      code: 0,
      message: 'success',
      data: mapPublicNewsItemsToPcNewsCards(response.data),
    },
  }
}

export async function fetchPublicNewsDetail(id: number | string): Promise<{ data: any }> {
  const response = await request.instance.get<BackendPublicNewsItem>(backendApiUrl(`/news/${encodeURIComponent(String(id))}`))
  return {
    data: {
      code: 0,
      message: 'success',
      data: mapPublicNewsItemsToPcNewsCards({ news: [response.data] })[0],
    },
  }
}

function normalizeNewsParams(params: FetchPublicNewsParams): Record<string, string | number> {
  const normalized: Record<string, string | number> = {}
  if (params.category) normalized.category = pcCategoryToBackend(params.category)
  if (params.countryCode) normalized.country_code = params.countryCode
  if (params.locale) normalized.locale = params.locale
  if (params.q) normalized.q = params.q
  if (params.limit !== undefined) normalized.limit = params.limit
  if (params.offset !== undefined) normalized.offset = params.offset
  return normalized
}

function pcCategoryToBackend(category: string): string {
  switch (category) {
    case 'announcement': return 'system'
    case 'flash': return 'market'
    case 'deep': return 'general'
    default: return category
  }
}
