import { describe, expect, it } from 'vitest';
import { ApiError, ApiNetworkError, ContractError } from '../api/client';
import { adminErrorMessage, adminErrorFieldValue } from './adminErrorMessage';

describe('Admin error presentation', () => {
  it.each([
    ['validation error: target_price must be positive', '目标价必须大于零'],
    ['validation error: end_time must be after start_time', '结束时间必须晚于开始时间'],
    ['volume_max must be greater than or equal to volume_min', '最大成交量不得小于最小成交量'],
    ['strategy_type is required', '策略类型不能为空'],
    ['active market strategy must be paused or disabled before update', '请先暂停或禁用行情策略后再修改配置'],
    ['unauthorized', '登录状态已失效，请重新登录'],
    ['forbidden', '当前账号没有此操作权限'],
    ['Bad Gateway', '服务网关暂时异常，请稍后重试'],
    ['conflict: 策略已经变化', '策略已经变化']
  ])('presents %s in Chinese', (raw, label) => {
    const error = new ApiError(400, 'VALIDATION_ERROR', raw);
    expect(adminErrorMessage(error)).toBe(label);
    expect(error.message).toBe(raw);
    expect(error.code).toBe('VALIDATION_ERROR');
    expect(error.status).toBe(400);
  });

  it('preserves sanitized unknown diagnostics and error identity for retries/conflicts', () => {
    const error = new ApiError(409, 'FUTURE_CODE', 'unexpected value token=secret-value Bearer abc.def');
    expect(adminErrorMessage(error)).toContain('操作失败');
    expect(adminErrorMessage(error)).toContain('FUTURE_CODE');
    expect(adminErrorMessage(error)).toContain('unexpected value');
    expect(adminErrorMessage(error)).not.toMatch(/secret-value|abc\.def/);
    expect(error.message).toContain('secret-value');
    const contract = new ContractError('missing decimal field amount', { path: '/admin/api/v1/example' });
    expect(adminErrorMessage(contract)).toContain('接口数据格式异常');
    expect(adminErrorMessage(contract)).toContain('missing decimal field amount');
    expect(adminErrorMessage(new ApiNetworkError(null))).toBe('网络连接失败，请检查网络后重试');
    expect(adminErrorMessage(null, '加载失败')).toBe('加载失败');
    expect(adminErrorFieldValue('error_message', 'Bad Gateway')).toBe('服务网关暂时异常，请稍后重试');
    expect(adminErrorFieldValue('reason', 'Bad Gateway')).toBeNull();
    expect(adminErrorMessage(new Error('constructor'))).toBe('操作失败，请稍后重试（诊断：constructor）');
  });
});
