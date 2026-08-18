/**
 * 对错误、原因和备注等非结构化文本做保守凭据脱敏。
 * 只替换带名称的赋值和 Bearer 令牌，普通业务文字保持原样。
 */
export function redactSensitiveText(value: string): string {
  return value
    .replace(
      /(^|[^\p{L}\p{N}])((?:"|'|`)?(?:(?:api|access)[\s_-]*)?(?:tokens?|passwords?|secrets?|keys?|passphrases?|credentials?|ciphertexts?)(?:"|'|`)?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|`[^`]*`|[^\s,;&，；}\]]+)/gimu,
      '$1$2***'
    )
    .replace(/(bearer\s+)[a-z\d._~+/-]+/giu, '$1***');
}

/** 把外部错误压缩成单行、定长且已脱敏的后台可展示文本。 */
export function safeSingleLineText(value: string, fallback: string, maxLength = 240): string {
  const firstLine = value.split(/\r?\n/u, 1)[0]?.trim();
  if (!firstLine) {
    return fallback;
  }
  const redacted = redactSensitiveText(firstLine);
  return redacted.length > maxLength ? `${redacted.slice(0, maxLength)}…` : redacted;
}
