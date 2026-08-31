import type { AuthScope, AuthSession } from '../auth/authStore';
import { canonicalDecimalText } from './decimal';

type KeyFactory = (prefix: string) => string;

export const FINANCIAL_COMMAND_STORAGE_KEY = 'exchange_admin_financial_command_intents_v1';
const COMMAND_LOG_VERSION = 1;
const decimalIntentFields = /(?:^|_)(?:amount|balance|fee|price|quantity|qty|rate|ratio|value)(?:$|_)/i;

function defaultKeyFactory(prefix: string): string {
  const randomPart = globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2, 10);
  return `${prefix}-${Date.now()}-${randomPart}`;
}

function canonicalValue(key: string, value: unknown): string {
  if (value === null || value === undefined) return 'none:';
  if (typeof value === 'boolean') return `boolean:${value ? 'true' : 'false'}`;
  const text = typeof value === 'string' ? value.trim() : String(value);
  if (decimalIntentFields.test(key)) {
    const decimal = canonicalDecimalText(text);
    if (decimal !== null) return `decimal:${decimal}`;
  }
  return `${typeof value}:${text}`;
}

/** 使用带长度的稳定序列化生成业务意图；金额字段先按十进制语义规范化。 */
export function canonicalRequestIntent(values: Record<string, unknown>): string {
  return Object.keys(values)
    .sort()
    .flatMap((key) => [key, canonicalValue(key, values[key])])
    .map((part) => `${part.length}:${part}`)
    .join('');
}

/** 将同一规范意图的失败重试固定到一枚键，只在首次成功后释放。 */
export class RetryStableIdempotencyKeys {
  private readonly pending = new Map<string, string>();
  private readonly prefix: string;
  private readonly keyFactory: KeyFactory;

  constructor(prefix: string, keyFactory: KeyFactory = defaultKeyFactory) {
    this.prefix = prefix;
    this.keyFactory = keyFactory;
  }

  acquire(intent: string): string {
    const existing = this.pending.get(intent);
    if (existing) return existing;
    const key = this.keyFactory(this.prefix);
    this.pending.set(intent, key);
    return key;
  }

  complete(intent: string, key: string): void {
    if (this.pending.get(intent) === key) this.pending.delete(intent);
  }
}

type FinancialCommandRecord = {
  command: string;
  createdAt: number;
  identity: string;
  intent: string;
  key: string;
  state: 'pending' | 'uncertain';
  updatedAt: number;
};

type FinancialCommandLog = {
  records: FinancialCommandRecord[];
  version: typeof COMMAND_LOG_VERSION;
};

type StorageLike = Pick<Storage, 'getItem' | 'removeItem' | 'setItem'>;

export type FinancialCommandScope = {
  assetId: string | number;
  authScope: AuthScope;
  command: string;
  generation: string;
  subject: string;
  userId: string | number;
};

export type FinancialCommandLease = {
  identity: string;
  intent: string;
  key: string;
};

export type RecoverableFinancialCommandOptions<T> = {
  /** 只有服务端明确拒绝且确定未执行时才能释放幂等键。 */
  isDefinitiveFailure?: (error: unknown) => boolean;
  request: (idempotencyKey: string) => Promise<T>;
  scope: FinancialCommandScope;
  store: FinancialCommandIntentStore;
  values: Record<string, unknown>;
};

function browserSessionStorage(): StorageLike | null {
  try {
    return typeof window === 'undefined' ? null : window.sessionStorage;
  } catch {
    return null;
  }
}

function validRecord(value: unknown): value is FinancialCommandRecord {
  if (!value || typeof value !== 'object') return false;
  const record = value as Partial<FinancialCommandRecord>;
  return (
    typeof record.command === 'string' &&
    typeof record.createdAt === 'number' &&
    typeof record.identity === 'string' &&
    typeof record.intent === 'string' &&
    typeof record.key === 'string' &&
    (record.state === 'pending' || record.state === 'uncertain') &&
    typeof record.updatedAt === 'number'
  );
}

function readLog(storage: StorageLike | null): FinancialCommandLog {
  if (!storage) return { records: [], version: COMMAND_LOG_VERSION };
  try {
    const raw = storage.getItem(FINANCIAL_COMMAND_STORAGE_KEY);
    if (!raw) return { records: [], version: COMMAND_LOG_VERSION };
    const parsed = JSON.parse(raw) as Partial<FinancialCommandLog>;
    if (parsed.version !== COMMAND_LOG_VERSION || !Array.isArray(parsed.records)) {
      return { records: [], version: COMMAND_LOG_VERSION };
    }
    return { records: parsed.records.filter(validRecord), version: COMMAND_LOG_VERSION };
  } catch {
    return { records: [], version: COMMAND_LOG_VERSION };
  }
}

function writeLog(storage: StorageLike | null, log: FinancialCommandLog): void {
  if (!storage) return;
  if (log.records.length === 0) {
    storage.removeItem(FINANCIAL_COMMAND_STORAGE_KEY);
    return;
  }
  storage.setItem(FINANCIAL_COMMAND_STORAGE_KEY, JSON.stringify(log));
}

export function financialCommandScopeFromSession(
  session: AuthSession,
  command: string,
  userId: string | number,
  assetId: string | number
): FinancialCommandScope {
  return {
    assetId,
    authScope: session.scope,
    command,
    generation: session.generation,
    subject: session.subject,
    userId
  };
}

/** 按会话/管理员/用户/资产保存未决资金命令，超时、响应丢失和页面重挂载都复用原键。 */
export class FinancialCommandIntentStore {
  private readonly keyFactory: KeyFactory;
  private readonly now: () => number;
  private readonly prefix: string;
  private readonly storage: StorageLike | null;

  constructor({
    keyFactory = defaultKeyFactory,
    now = Date.now,
    prefix = 'financial-command',
    storage = browserSessionStorage()
  }: {
    keyFactory?: KeyFactory;
    now?: () => number;
    prefix?: string;
    storage?: StorageLike | null;
  } = {}) {
    this.keyFactory = keyFactory;
    this.now = now;
    this.prefix = prefix;
    this.storage = storage;
  }

  acquire(scope: FinancialCommandScope, values: Record<string, unknown>): FinancialCommandLease {
    const now = this.now();
    const identity = canonicalRequestIntent({
      asset_id: scope.assetId,
      auth_scope: scope.authScope,
      command: scope.command,
      generation: scope.generation,
      subject: scope.subject,
      user_id: scope.userId
    });
    const intent = canonicalRequestIntent(values);
    const log = readLog(this.storage);
    const existing = log.records.find((record) => record.identity === identity && record.intent === intent);
    if (existing) {
      existing.updatedAt = now;
      writeLog(this.storage, log);
      return { identity, intent, key: existing.key };
    }

    const key = this.keyFactory(this.prefix);
    log.records.push({ command: scope.command, createdAt: now, identity, intent, key, state: 'pending', updatedAt: now });
    writeLog(this.storage, log);
    return { identity, intent, key };
  }

  markUncertain(lease: FinancialCommandLease): void {
    const log = readLog(this.storage);
    const record = log.records.find((item) => item.identity === lease.identity && item.intent === lease.intent && item.key === lease.key);
    if (!record) return;
    record.state = 'uncertain';
    record.updatedAt = this.now();
    writeLog(this.storage, log);
  }

  complete(lease: FinancialCommandLease): void {
    const log = readLog(this.storage);
    log.records = log.records.filter((record) => !(record.identity === lease.identity && record.intent === lease.intent && record.key === lease.key));
    writeLog(this.storage, log);
  }
}

/**
 * 执行可恢复资金命令。超时、断网、响应体丢失和取消都保留原键；
 * 成功或调用方判定为“服务端明确未执行”的失败才释放。
 */
export async function runRecoverableFinancialCommand<T>({
  isDefinitiveFailure = () => false,
  request,
  scope,
  store,
  values
}: RecoverableFinancialCommandOptions<T>): Promise<T> {
  const lease = store.acquire(scope, values);
  try {
    const result = await request(lease.key);
    store.complete(lease);
    return result;
  } catch (error) {
    if (isDefinitiveFailure(error)) {
      store.complete(lease);
    } else {
      store.markUncertain(lease);
    }
    throw error;
  }
}

export const financialCommandIntents = new FinancialCommandIntentStore({ prefix: 'admin-recharge' });
