import { ApiError } from '../../api/client';
import { safeSingleLineText } from '../../shared/sensitiveText';

export const adminSettingsQueryKeys = {
  all: ['admin-settings'] as const,
  detail: (setting: string) => [...adminSettingsQueryKeys.all, setting] as const
};

export const SETTINGS_CONFLICT_MESSAGE = '配置已被其他管理员更新，当前草稿尚未覆盖；请重新加载最新配置后再修改。';

/**
 * 设置读取只重试一次网络错误和服务端错误；明确的 4xx 响应不会因重试而重复打扰服务端。
 */
export function settingsQueryRetry(failureCount: number, error: Error): boolean {
  if (error instanceof ApiError && error.status < 500) {
    return false;
  }

  return failureCount < 1;
}

/**
 * 配置写入可能已经在服务端生效，统一关闭自动重试，避免重复提交审计或覆盖新版本。
 */
export const settingsMutationRetry = false;

export function settingsErrorMessage(error: unknown, fallback = '配置操作失败，请稍后重试。'): string {
  if (error instanceof ApiError && error.status === 409) {
    return SETTINGS_CONFLICT_MESSAGE;
  }

  if (error instanceof ApiError || error instanceof Error) {
    return safeSingleLineText(error.message, fallback);
  }

  return fallback;
}
