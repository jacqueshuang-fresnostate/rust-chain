import request from './request'
import {
  backendApiUrl,
  mapPublicCountriesToPcOptions,
  type BackendPublicCountriesResponse,
} from './backendAdapters'

export async function fetchPublicCountries() {
  const response = await request.instance.get<BackendPublicCountriesResponse>(backendApiUrl('/countries'))
  return {
    code: 0,
    message: 'success',
    data: mapPublicCountriesToPcOptions(response.data),
  }
}
