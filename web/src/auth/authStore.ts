export type AuthScope = 'admin' | 'agent' | 'user';

export interface AuthSession {
  accessToken: string;
  refreshToken: string;
  scope: AuthScope;
  subject: string;
}

export const SESSION_STORAGE_KEY = 'exchange_admin_session';

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

export const authStore = {
  getSession(): AuthSession | null {
    return parseSession(localStorage.getItem(SESSION_STORAGE_KEY));
  },

  setSession(session: AuthSession): void {
    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session));
  },

  clearSession(): void {
    localStorage.removeItem(SESSION_STORAGE_KEY);
  }
};
