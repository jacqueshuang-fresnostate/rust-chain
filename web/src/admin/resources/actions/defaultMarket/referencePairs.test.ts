import { beforeEach, expect, it, vi } from 'vitest';
import { apiRequest } from '../../../../api/client';
import { defaultMarketDraft } from './model';
import { loadDefaultMarketReferences, validateDefaultMarketReference } from './referencePairs';
import { responseFixture } from './testFixtures';

vi.mock('../../../../api/client', async (original) => ({ ...await original<typeof import('../../../../api/client')>(), apiRequest: vi.fn() }));
beforeEach(() => vi.mocked(apiRequest).mockReset());
const pair = (id: number) => ({ id, symbol: id === 101 ? 'BTCUSDT' : `REF${id}USDT`, status: 'active', market_type: 'external' });

it('loads every filtered directory page rather than silently losing BTC beyond the first 100', async () => {
  vi.mocked(apiRequest).mockResolvedValueOnce({ pairs: Array.from({ length: 100 }, (_, index) => pair(index + 1)), total: 101 }).mockResolvedValueOnce({ pairs: [pair(101)], total: 101 });
  const signal = new AbortController().signal;
  const result = await loadDefaultMarketReferences(signal);
  expect(result).toHaveLength(101);
  expect(result[100]).toEqual(pair(101));
  expect(apiRequest).toHaveBeenNthCalledWith(1, '/admin/api/v1/market-pairs?status=active&market_type=external&limit=100&offset=0', { signal });
  expect(apiRequest).toHaveBeenNthCalledWith(2, '/admin/api/v1/market-pairs?status=active&market_type=external&limit=100&offset=100', { signal });
});

it('fails closed on an invalid directory, partial failure or non-progress page', async () => {
  for (const row of [{ ...pair(1), status: 'disabled' }, { ...pair(1), market_type: 'strategy' }, { ...pair(1), id: 0 }]) {
    vi.mocked(apiRequest).mockResolvedValueOnce({ pairs: [row], total: 1 });
    await expect(loadDefaultMarketReferences(new AbortController().signal)).rejects.toThrow('参考交易对目录');
  }
  vi.mocked(apiRequest).mockResolvedValueOnce({ pairs: [pair(1)], total: 3 }).mockRejectedValueOnce(new Error('目录请求失败'));
  await expect(loadDefaultMarketReferences(new AbortController().signal)).rejects.toThrow('目录请求失败');
  vi.mocked(apiRequest).mockResolvedValueOnce({ pairs: [pair(1)], total: 3 }).mockResolvedValueOnce({ pairs: [pair(1)], total: 3 });
  await expect(loadDefaultMarketReferences(new AbortController().signal)).rejects.toThrow('分页未前进');
});

it('requires a current eligible reference but never blocks unrelated independent saves', () => {
  const draft = { ...defaultMarketDraft(responseFixture()), mode: 'follow' as const, reference_pair_id: '101' };
  const loaded = { data: [pair(101)], loading: false, error: null };
  expect(validateDefaultMarketReference(draft, '3', loaded, true)).toBe('');
  expect(validateDefaultMarketReference(draft, '101', loaded, true)).toContain('不得选择当前交易对');
  expect(validateDefaultMarketReference(draft, '3', { ...loaded, data: [] }, true)).toContain('请选择');
  expect(validateDefaultMarketReference(draft, '3', loaded, false)).toContain('读取权限');
  expect(validateDefaultMarketReference(draft, '3', { ...loaded, loading: true }, true)).toContain('正在读取');
  expect(validateDefaultMarketReference(draft, '3', { ...loaded, error: new Error('offline') }, true)).toContain('加载失败');
  expect(validateDefaultMarketReference({ ...draft, mode: 'independent' }, '3', { data: [], loading: true, error: new Error('offline') }, false)).toBe('');
});
