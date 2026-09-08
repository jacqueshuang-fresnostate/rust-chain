import { useIsFetching } from '@tanstack/react-query';
import { listAdminResource } from '../../../../api/adminResources';
import { ADMIN_OPTION_QUERY_KEY, useSharedAdminOptionQuery } from '../../../sharedOptionQuery';
import type { DefaultMarketDraft, DefaultMarketReferencePair } from './types';

/** 复用交易对目录及现有服务端过滤/分页；不截断前 100 项，也不按顺序猜测 BTC。 */
export async function loadDefaultMarketReferences(signal: AbortSignal): Promise<DefaultMarketReferencePair[]> {
  const pairs = new Map<number, DefaultMarketReferencePair>();
  let offset = 0;
  for (let page = 0; page < 100; page += 1) {
    const result = await listAdminResource('/admin/api/v1/market-pairs', 'pairs', {
      status: 'active', market_type: 'external', limit: 100, offset
    }, { signal, rowContract: { requiredFields: ['id', 'symbol', 'status', 'market_type'] } });
    const previousSize = pairs.size;
    for (const row of result.rows) {
      if (typeof row.id !== 'number' || !Number.isSafeInteger(row.id) || row.id <= 0 || typeof row.symbol !== 'string') throw new Error('参考交易对目录包含无效身份，请重新加载。');
      if (row.status !== 'active' || row.market_type !== 'external') throw new Error('参考交易对目录状态已变化，请重新加载。');
      pairs.set(row.id, { id: row.id, symbol: row.symbol, status: row.status, market_type: row.market_type });
    }
    if (result.rows.length > 0 && pairs.size - previousSize !== result.rows.length) throw new Error('参考交易对目录分页未前进，请重新加载。');
    offset += result.rows.length;
    if ((result.total !== undefined && offset >= result.total) || (result.total === undefined && result.rows.length < 100)) return [...pairs.values()];
    if (result.rows.length === 0 || pairs.size === previousSize) throw new Error('参考交易对目录分页未前进，请重新加载。');
  }
  throw new Error('参考交易对目录超过读取上限，请联系管理员检查目录。');
}

export function useDefaultMarketReferences(enabled: boolean) {
  const cacheKey = 'market-pairs:active:external:complete';
  const query = useSharedAdminOptionQuery({ cacheKey, empty: [] as DefaultMarketReferencePair[], enabled, load: loadDefaultMarketReferences, staleTime: 0 });
  const fetching = useIsFetching({ predicate: (entry) => entry.queryKey[0] === ADMIN_OPTION_QUERY_KEY && entry.queryKey[2] === cacheKey });
  return { ...query, loading: enabled && (query.loading || fetching > 0) };
}

export function validateDefaultMarketReference(draft: DefaultMarketDraft, pairId: string, references: { data: DefaultMarketReferencePair[]; loading: boolean; error: Error | null }, canRead: boolean): string {
  if (draft.mode !== 'follow') return '';
  if (!canRead) return '缺少参考交易对读取权限，跟随配置暂不可保存；独立模式不受影响。';
  if (references.loading) return '正在读取参考交易对目录，请稍候。';
  if (references.error) return '参考交易对目录加载失败，跟随配置暂不可保存；请重新加载，或明确选择独立模式。';
  const selected = references.data.find((pair) => String(pair.id) === draft.reference_pair_id);
  if (!selected || selected.status !== 'active' || selected.market_type !== 'external' || String(selected.id) === pairId) return '请选择仍启用的外部参考交易对，且不得选择当前交易对。';
  return '';
}
