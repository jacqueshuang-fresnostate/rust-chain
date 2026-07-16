import { fetchKlineHistory as fetchSwapKLine } from '../../api/contract'
import { fetchHistoryKLine as fetchMarketKLine } from '../../api/market'
import { fetchHistoryKLine as fetchSecondKLine } from '../../api/second'
import { normalizeKlineModule, type KlineFetcher, type KlineModule } from './klineData'

export function resolveKlineHistoryFetcher(module: KlineModule | undefined, override?: KlineFetcher): KlineFetcher {
  if (override) return override

  switch (normalizeKlineModule(module)) {
    case 'margin':
      return (symbol, resolution, from, to) => fetchSwapKLine(symbol, from, to, resolution)
    case 'seconds':
      return fetchSecondKLine
    case 'spot':
    default:
      return fetchMarketKLine
  }
}
