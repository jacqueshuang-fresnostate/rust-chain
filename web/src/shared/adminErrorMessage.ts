import { adminFieldLabel } from './adminPresentation';
import { safeSingleLineText } from './sensitiveText';

const knownMessages: Record<string, string> = {
  unauthorized: '登录状态已失效，请重新登录',
  forbidden: '当前账号没有此操作权限',
  'not found': '记录不存在或已被移除',
  'failed to fetch': '网络连接失败，请检查网络后重试',
  'network error': '网络连接失败，请检查网络后重试',
  'bad gateway': '服务网关暂时异常，请稍后重试',
  'service unavailable': '服务暂时不可用，请稍后重试',
  'gateway timeout': '服务响应超时，请先核对操作结果',
  'active market strategy must be paused or disabled before update': '请先暂停或禁用行情策略后再修改配置',
  'strategy prices must be positive': '起始价和目标价必须大于零',
  'volatility and volume must be non-negative': '波动率和成交量必须为非负数',
  'unsupported market strategy status': '不支持此行情策略状态',
  'trading pair precision must be non-negative': '交易对价格和数量精度必须为非负整数'
};

const codeMessages: Record<string, string> = {
  UNAUTHORIZED: '登录状态已失效，请重新登录', FORBIDDEN: '当前账号没有此操作权限',
  NOT_FOUND: '记录不存在或已被移除', VALIDATION_ERROR: '参数校验失败，请检查填写内容',
  CONFLICT: '数据状态已变化，请核对最新状态后重试',
  CONFIG_ERROR: '服务配置异常，请联系维护人员', DATABASE_ERROR: '数据库服务异常，请稍后重试',
  MONGO_ERROR: '行情存储服务异常，请稍后重试', REDIS_ERROR: '缓存服务异常，请稍后重试',
  RABBITMQ_ERROR: '消息服务异常，请稍后重试', INTERNAL_ERROR: '服务处理异常，请稍后重试',
  API_CONTRACT_ERROR: '接口数据格式异常，请联系维护人员核对版本'
};

function fieldName(key: string): string {
  const label = adminFieldLabel(key);
  return label === key ? `参数（${key}）` : label;
}

/** Preserve API errors for control flow; localize and redact only at UI boundaries. */
export function adminErrorMessage(error: unknown, fallback = '操作失败，请稍后重试'): string {
  if (!(error instanceof Error) && typeof error !== 'string') return fallback;
  const raw = safeSingleLineText(typeof error === 'string' ? error : error.message, '');
  if (!raw) return fallback;
  const message = raw.replace(/^(?:validation error|conflict):\s*/i, '');
  const known = knownMessages[message.toLowerCase()];
  if (typeof known === 'string') return known;
  // Preserve already-localized backend business explanations and useful diagnostics.
  if (/[\u3400-\u9fff]/u.test(message)) return message;
  const required = message.match(/^([a-z_]+) is required$/i);
  if (required) return `${fieldName(required[1])}不能为空`;
  const bound = message.match(/^([a-z_]+) must be (positive|non-negative|a positive integer|a non-negative integer)$/i);
  if (bound) return `${fieldName(bound[1])}${({ positive: '必须大于零', 'non-negative': '必须为非负数', 'a positive integer': '必须为正整数', 'a non-negative integer': '必须为非负整数' } as Record<string, string>)[bound[2].toLowerCase()]}`;
  const comparison = message.match(/^([a-z_]+) must be (after|greater than or equal to|greater than|less than or equal to|less than) ([a-z_]+)$/i);
  if (comparison) return `${fieldName(comparison[1])}${({ after: '必须晚于', 'greater than or equal to': '不得小于', 'greater than': '必须大于', 'less than or equal to': '不得大于', 'less than': '必须小于' } as Record<string, string>)[comparison[2].toLowerCase()]}${fieldName(comparison[3])}`;
  const code = typeof error === 'object' && 'code' in error && typeof error.code === 'string' ? error.code : '';
  const summary = typeof codeMessages[code] === 'string' ? codeMessages[code] : fallback;
  return `${summary}（诊断：${code ? `${safeSingleLineText(code, '')}；` : ''}${raw}）`;
}

/** Error DTO fields share the same display policy as request failures. */
export function adminErrorFieldValue(key: string, value: unknown): string | null {
  return ['error_message', 'last_error_summary', 'last_reload_error'].includes(key) && typeof value === 'string' && value.trim()
    ? adminErrorMessage(value, '运行异常') : null;
}
