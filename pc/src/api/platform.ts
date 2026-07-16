import request from './request'
import { backendApiUrl } from './backendAdapters'

export interface PlatformBrand {
    id: number
    name: string
    platform_name: string
    logo_url?: string | null
    chart_provider?: string | null
    updated_by?: number | null
    created_at: number
    updated_at: number
}

export async function getPlatformBrand() {
    const res = await request.instance.get<PlatformBrand>(backendApiUrl('/platform/brand'))
    return {
        code: 0,
        message: 'success',
        data: res.data,
    }
}
