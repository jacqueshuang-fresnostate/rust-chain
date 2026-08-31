function containsControlOrBackslash(value: string): boolean {
  return [...value].some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return character === '\\' || codePoint <= 0x1f || codePoint === 0x7f;
  });
}

/** 只接受当前站点内的绝对路径，拒绝协议相对 URL、反斜线和控制字符。 */
export function safeInternalRedirect(value: unknown, fallback: string, allowedPrefix?: string): string {
  if (typeof value !== 'string' || !value.startsWith('/') || value.startsWith('//') || containsControlOrBackslash(value)) return fallback;
  try {
    const parsed = new URL(value, 'https://internal.invalid');
    if (parsed.origin !== 'https://internal.invalid') return fallback;
    const target = `${parsed.pathname}${parsed.search}${parsed.hash}`;
    if (allowedPrefix && !(parsed.pathname === allowedPrefix || parsed.pathname.startsWith(`${allowedPrefix}/`))) return fallback;
    return target;
  } catch {
    return fallback;
  }
}
