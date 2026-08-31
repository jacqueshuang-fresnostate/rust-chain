import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest, ContractError } from './client';
import { listAdminResource } from './adminResources';

vi.mock('./client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./client')>();
  return { ...actual, apiRequest: vi.fn() };
});

const apiRequestMock = vi.mocked(apiRequest);

describe('listAdminResource 合约', () => {
  beforeEach(() => apiRequestMock.mockReset());

  it('转发 AbortSignal，并仅接受明确的列表 DTO', async () => {
    const controller = new AbortController();
    apiRequestMock.mockResolvedValue({ assets: [{ id: 1, symbol: 'BTC' }], total: 1 });

    await expect(
      listAdminResource('/admin/api/v1/assets', 'assets', { limit: 10 }, { signal: controller.signal })
    ).resolves.toEqual({ rows: [{ id: 1, symbol: 'BTC' }], raw: { assets: [{ id: 1, symbol: 'BTC' }], total: 1 }, total: 1 });
    expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/assets?limit=10', { signal: controller.signal });
  });

  it('缺失列表字段时失败关闭，不将合约漂移伪装为空列表', async () => {
    apiRequestMock.mockResolvedValue({ items: [] });
    await expect(listAdminResource('/admin/api/v1/assets', 'assets')).rejects.toMatchObject({
      message: expect.stringContaining('缺少列表字段 assets'),
      name: ContractError.name
    });
  });

  it('拒绝非对象数组与非法 total', async () => {
    apiRequestMock.mockResolvedValueOnce({ assets: ['BTC'] }).mockResolvedValueOnce({ assets: [], total: -1 });
    await expect(listAdminResource('/admin/api/v1/assets', 'assets')).rejects.toThrow('assets 必须是对象数组');
    await expect(listAdminResource('/admin/api/v1/assets', 'assets')).rejects.toThrow('total 必须是非负安全整数');
  });

  it('按窄行合同拒绝缺失必填字段和非 Decimal text', async () => {
    const rowContract = { requiredFields: ['id', 'amount'], decimalFields: ['amount'] };
    apiRequestMock
      .mockResolvedValueOnce({ assets: [{ amount: '1.0' }] })
      .mockResolvedValueOnce({ assets: [{ id: 1, amount: 1.5 }] })
      .mockResolvedValueOnce({ assets: [{ id: 1, amount: 'not-a-decimal' }] });

    await expect(listAdminResource('/admin/api/v1/assets', 'assets', {}, { rowContract })).rejects.toThrow('缺少必填字段 id');
    await expect(listAdminResource('/admin/api/v1/assets', 'assets', {}, { rowContract })).rejects.toThrow('amount 必须是 Decimal text');
    await expect(listAdminResource('/admin/api/v1/assets', 'assets', {}, { rowContract })).rejects.toThrow('amount 必须是 Decimal text');
  });

  it('在边界保留微额和超过 2^53 的 Decimal text', async () => {
    const rows = [
      { id: 1, amount: '1e-18' },
      { id: 2, amount: '9007199254740993.000000000000000001' }
    ];
    apiRequestMock.mockResolvedValue({ assets: rows });

    await expect(
      listAdminResource('/admin/api/v1/assets', 'assets', {}, {
        rowContract: { requiredFields: ['id', 'amount'], decimalFields: ['amount'] }
      })
    ).resolves.toMatchObject({ rows });
  });
});
