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
    ['conflict: 策略已经变化', '策略已经变化'],
    ['amount exceeds asset precision_scale 2', '充值金额最多支持 2 位小数，不会自动舍入'],
    ['pricing_mode must be fixed or market', '定价模式请选择固定价格或市场价格'],
    ['spread_rate must be greater than or equal to 0 and less than 1', '价差率必须大于等于 0 且小于 1（0.01 表示 1%）'],
    ['fee_rate supports at most 8 decimal places', '手续费率最多支持 8 位有效小数，不会自动舍入']
  ])('presents %s in Chinese', (raw, label) => {
    const error = new ApiError(400, 'VALIDATION_ERROR', raw);
    expect(adminErrorMessage(error)).toBe(label);
    expect(error.message).toBe(raw);
    expect(error.code).toBe('VALIDATION_ERROR');
    expect(error.status).toBe(400);
  });

  it.each([
    ['config_json must be an object', '风控配置（config_json）必须为 JSON 对象'],
    ['config_json.operations must be an array of nonblank strings without outer whitespace', '生效操作（config_json.operations）必须为字符串数组，每项须为非空字符串且首尾不含空白'],
    ['config_json.blocked_operations must be an array of nonblank strings without outer whitespace', '禁止操作（config_json.blocked_operations）必须为字符串数组，每项须为非空字符串且首尾不含空白'],
    ['config_json.max_amount must be a nonnegative decimal string or number', '单笔金额上限（config_json.max_amount）必须为大于等于 0 的十进制数字或数字字符串'],
    ['config_json.max_price_deviation_bps must be an unsigned 32-bit integer', '价格偏离上限（config_json.max_price_deviation_bps）必须为 0 至 4294967295 的整数或整数字符串（单位：基点）'],
    ['config_json.max_requests must be an unsigned 32-bit integer', '请求次数上限（config_json.max_requests）必须为 0 至 4294967295 的整数或整数字符串'],
    ['config_json.window_seconds must be a positive unsigned 32-bit integer', '限频窗口（config_json.window_seconds）必须为 1 至 4294967295 的整数或整数字符串（单位：秒）']
  ])('localizes the exact risk validator error %s without mutating it', (raw, label) => {
    for (const message of [raw, `validation error: ${raw}`]) {
      const error = new ApiError(400, 'VALIDATION_ERROR', message);
      expect(adminErrorMessage(error)).toBe(label);
      expect(error.message).toBe(message);
      expect(error.code).toBe('VALIDATION_ERROR');
      expect(error.status).toBe(400);
    }
  });

  it('keeps unknown risk field errors as diagnostics rather than inferring a translation', () => {
    const raw = 'config_json.future_limit must be an unsigned 32-bit integer';
    const error = new ApiError(400, 'VALIDATION_ERROR', raw);
    expect(adminErrorMessage(error)).toBe(`参数校验失败，请检查填写内容（诊断：VALIDATION_ERROR；${raw}）`);
    expect(error.message).toBe(raw);
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
