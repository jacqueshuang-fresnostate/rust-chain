export type SettingsDifference = {
  after: string;
  before: string;
  field: string;
  key: string;
  sensitive?: boolean;
};

export type SettingsFieldDefinition<T> = {
  field: string;
  format?: (value: unknown) => string;
  impact?: string;
  key: string;
  read: (value: T) => unknown;
  sensitive?: boolean;
  validate?: (value: unknown, settings: T) => string | null | undefined;
};

export type SettingsValidationIssue = {
  field: string;
  key: string;
  message: string;
};

function valuesEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) {
    return true;
  }

  return JSON.stringify(left) === JSON.stringify(right);
}

function hasConfiguredValue(value: unknown): boolean {
  if (value === null || value === undefined || value === '') {
    return false;
  }

  if (Array.isArray(value)) {
    return value.length > 0;
  }

  return true;
}

export function formatSettingsValue(value: unknown): string {
  if (value === null || value === undefined || value === '') {
    return '未设置';
  }

  if (typeof value === 'boolean') {
    return value ? '开启' : '关闭';
  }

  if (Array.isArray(value)) {
    return value.length > 0 ? value.map(formatSettingsValue).join('、') : '未设置';
  }

  return String(value);
}

export function formatSensitiveSettingsValue(value: unknown): string {
  return hasConfiguredValue(value) ? '已配置（内容不回显）' : '未配置';
}

export function buildSettingsDifferences<T>(
  before: T,
  after: T,
  fields: ReadonlyArray<SettingsFieldDefinition<T>>
): SettingsDifference[] {
  return fields.flatMap((definition) => {
    const beforeValue = definition.read(before);
    const afterValue = definition.read(after);
    if (valuesEqual(beforeValue, afterValue)) {
      return [];
    }

    const format = definition.format ?? formatSettingsValue;
    const beforeDisplay = definition.sensitive
      ? formatSensitiveSettingsValue(beforeValue)
      : format(beforeValue);
    const afterDisplay = definition.sensitive
      ? (
          hasConfiguredValue(beforeValue) && hasConfiguredValue(afterValue)
            ? '已更新（内容不回显）'
            : formatSensitiveSettingsValue(afterValue)
        )
      : format(afterValue);

    return [{
      after: afterDisplay,
      before: beforeDisplay,
      field: definition.field,
      key: definition.key,
      sensitive: definition.sensitive
    }];
  });
}

/** 运行字段 schema 中的前端校验，并把错误收敛为可直接展示的中文字段问题列表。 */
export function validateSettingsFields<T>(
  settings: T,
  fields: ReadonlyArray<SettingsFieldDefinition<T>>
): SettingsValidationIssue[] {
  return fields.flatMap((definition) => {
    const message = definition.validate?.(definition.read(settings), settings)?.trim();
    return message
      ? [{ field: definition.field, key: definition.key, message }]
      : [];
  });
}

/** 仅汇总本次实际变化字段声明的影响文案，去重后保持 schema 顺序。 */
export function buildSettingsImpactSummary<T>(
  differences: ReadonlyArray<SettingsDifference>,
  fields: ReadonlyArray<SettingsFieldDefinition<T>>,
  fallback: string
): string {
  const changedKeys = new Set(differences.map((difference) => difference.key));
  const impacts = [...new Set(
    fields
      .filter((definition) => changedKeys.has(definition.key))
      .map((definition) => definition.impact?.trim())
      .filter((impact): impact is string => Boolean(impact))
  )];
  return impacts.length > 0 ? impacts.join('；') : fallback;
}

export function settingsValuesEqual<T>(left: T, right: T): boolean {
  return valuesEqual(left, right);
}
