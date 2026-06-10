export type AuthScope = 'admin' | 'agent' | 'user';

export interface AuthSession {
  accessToken: string;
  refreshToken: string;
  scope: AuthScope;
  subject: string;
}

export const SESSION_STORAGE_KEY = 'exchange_admin_session';
export const AGENT_SESSION_STORAGE_KEY = 'exchange_agent_session';

const authScopes = new Set<AuthScope>(['admin', 'agent', 'user']);

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function isAuthScope(value: unknown): value is AuthScope {
  return typeof value === 'string' && authScopes.has(value as AuthScope);
}

function parseSession(raw: string | null): AuthSession | null {
  if (!raw) {
    return null;
  }

  try {
    const value = JSON.parse(raw) as Partial<AuthSession>;
    if (
      !isNonEmptyString(value.accessToken) ||
      !isNonEmptyString(value.refreshToken) ||
      !isAuthScope(value.scope) ||
      !isNonEmptyString(value.subject)
    ) {
      return null;
    }

    return {
      accessToken: value.accessToken,
      refreshToken: value.refreshToken,
      scope: value.scope,
      subject: value.subject
    };
  } catch {
    return null;
  }
}

function storageKeyForScope(scope: AuthScope = 'admin'): string {
  return scope === 'agent' ? AGENT_SESSION_STORAGE_KEY : SESSION_STORAGE_KEY;
}

export const authStore = {
  getSession(scope: AuthScope = 'admin'): AuthSession | null {
    return parseSession(localStorage.getItem(storageKeyForScope(scope)));
  },

  setSession(session: AuthSession): void {
    localStorage.setItem(storageKeyForScope(session.scope), JSON.stringify(session));
  },

  clearSession(scope: AuthScope = 'admin'): void {
    localStorage.removeItem(storageKeyForScope(scope));
  }
};
