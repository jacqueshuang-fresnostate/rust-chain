type BetContentRecord = Record<string, unknown>;

const BET_CONTENT_KEY_PARTS = [
  'betcontent',
  'betinfo',
  'betdetail',
  'betnumbers',
  'ticketcontent',
  'wagercontent',
  'wagerinfo'
];

const BET_CONTENT_LABEL_PARTS = ['投注内容', '下注内容', '选号内容'];

const POSITION_LIST_KEYS = ['positions', 'position_list', 'rows', 'items', 'details', 'selections', 'bets'];
const NUMBER_VALUE_KEYS = ['numbers', 'selected_numbers', 'selectedNumbers', 'values', 'digits', 'nums', 'balls', 'codes', 'number', 'selection'];
const LABEL_VALUE_KEYS = ['label', 'name', 'position_name', 'positionName', 'title'];

const BET_FIELD_LABELS: Record<string, string> = {
  amount: '金额',
  bet_amount: '投注金额',
  betAmount: '投注金额',
  count: '注数',
  multiple: '倍数',
  multiplier: '倍数',
  name: '名称',
  number: '号码',
  numbers: '号码',
  odds: '赔率',
  play: '玩法',
  play_code: '玩法',
  play_name: '玩法',
  playCode: '玩法',
  playName: '玩法',
  position: '位置',
  position_name: '位置',
  positionName: '位置',
  selected_numbers: '号码',
  selectedNumbers: '号码',
  unit_price: '单价',
  unitPrice: '单价',
  values: '号码'
};

function isRecord(value: unknown): value is BetContentRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizedKey(value: string) {
  return value.replace(/[\s_-]/g, '').toLowerCase();
}

export function isAdminBetContentField(key = '', label = '') {
  const keyText = normalizedKey(key);
  const labelText = label.trim();
  return BET_CONTENT_KEY_PARTS.some((part) => keyText.includes(part)) || BET_CONTENT_LABEL_PARTS.some((part) => labelText.includes(part));
}

function parseStructuredString(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) {
    return value;
  }

  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    return value;
  }
}

function formatPlainText(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return '';
  }

  if (!trimmed.includes(',') && !trimmed.includes('，')) {
    return trimmed;
  }

  const parts = trimmed
    .split(/[，,]/)
    .map((part) => part.trim())
    .filter(Boolean);
  return parts.length > 1 ? parts.join('、') : trimmed;
}

function getFirstValue(record: BetContentRecord, keys: string[]) {
  for (const key of keys) {
    if (record[key] !== undefined && record[key] !== null && record[key] !== '') {
      return { key, value: record[key] };
    }
  }
  return null;
}

function formatFieldLabel(key: string) {
  return BET_FIELD_LABELS[key] ?? key.replace(/_/g, ' ');
}

function formatPositionLabel(value: unknown, fallbackIndex?: number) {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return `第 ${value} 位`;
  }
  if (typeof value === 'string' && value.trim()) {
    const trimmed = value.trim();
    return /^\d+$/.test(trimmed) ? `第 ${trimmed} 位` : trimmed;
  }
  return fallbackIndex === undefined ? '' : `第 ${fallbackIndex + 1} 位`;
}

function formatPrimitive(value: unknown) {
  if (value === null || value === undefined || value === '') {
    return '';
  }
  if (typeof value === 'string') {
    const parsed = parseStructuredString(value);
    return parsed === value ? formatPlainText(value) : formatBetValue(parsed);
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  return '';
}

function formatArray(value: unknown[]): string {
  if (value.length === 0) {
    return '';
  }

  if (value.every((item) => !isRecord(item) && !Array.isArray(item))) {
    return value.map(formatPrimitive).filter(Boolean).join('、');
  }

  return value
    .map((item, index) => {
      if (Array.isArray(item)) {
        const numbers = formatArray(item);
        return numbers ? `第 ${index + 1} 位：${numbers}` : '';
      }
      if (isRecord(item)) {
        return formatObject(item, index);
      }
      return formatPrimitive(item);
    })
    .filter(Boolean)
    .join('；');
}

function formatObject(record: BetContentRecord, fallbackIndex?: number): string {
  const listValue = getFirstValue(record, POSITION_LIST_KEYS);
  if (listValue && Array.isArray(listValue.value)) {
    const usedKeys = new Set([listValue.key]);
    const prefix = Object.entries(record)
      .filter(([key, value]) => !usedKeys.has(key) && value !== null && value !== undefined && value !== '')
      .map(([key, value]) => `${formatFieldLabel(key)}：${formatBetValue(value)}`)
      .filter((item) => !item.endsWith('：'));
    const listText = formatArray(listValue.value);
    return [...prefix, listText].filter(Boolean).join('；');
  }

  const numberValue = getFirstValue(record, NUMBER_VALUE_KEYS);
  const labelValue = getFirstValue(record, LABEL_VALUE_KEYS);
  const positionValue = record.position ?? record.pos ?? record.place;
  const label = formatPositionLabel(labelValue?.value ?? positionValue, fallbackIndex);

  if (numberValue) {
    const usedKeys = new Set([numberValue.key, labelValue?.key, 'position', 'pos', 'place'].filter((key): key is string => Boolean(key)));
    const numbers = formatBetValue(numberValue.value);
    const extra = Object.entries(record)
      .filter(([key, value]) => !usedKeys.has(key) && value !== null && value !== undefined && value !== '')
      .map(([key, value]) => `${formatFieldLabel(key)}：${formatBetValue(value)}`)
      .filter((item) => !item.endsWith('：'));
    const main = label ? `${label}：${numbers}` : `号码：${numbers}`;
    return extra.length > 0 ? `${main}（${extra.join('，')}）` : main;
  }

  return Object.entries(record)
    .map(([key, value]) => `${formatFieldLabel(key)}：${formatBetValue(value)}`)
    .filter((item) => !item.endsWith('：'))
    .join('；');
}

function formatBetValue(value: unknown): string {
  if (Array.isArray(value)) {
    return formatArray(value);
  }
  if (isRecord(value)) {
    return formatObject(value);
  }
  return formatPrimitive(value);
}

export function formatAdminBetContent(value: unknown): string | null {
  const formatted = formatBetValue(value);
  return formatted.trim() ? formatted : null;
}
