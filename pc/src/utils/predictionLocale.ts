export type PredictionTextKind = 'title' | 'description' | 'category' | 'outcome'

export interface PredictionLocalizedTextItem {
  locale?: string | null
  country?: string | null
  title?: string | null
  text?: string | null
  value?: string | null
  description?: string | null
  label?: string | null
  name?: string | null
}

export interface PredictionLocalizedTextDocument {
  default_locale?: string | null
  items?: PredictionLocalizedTextItem[] | null
}

const CATEGORY_ZH: Record<string, string> = {
  politics: '政治',
  election: '选举',
  elections: '选举',
  crypto: '加密货币',
  cryptocurrency: '加密货币',
  bitcoin: '比特币',
  ethereum: '以太坊',
  sports: '体育',
  business: '商业',
  economy: '经济',
  economics: '经济',
  finance: '金融',
  technology: '科技',
  tech: '科技',
  culture: '文化',
  'pop culture': '流行文化',
  entertainment: '娱乐',
  science: '科学',
  world: '国际',
  news: '新闻',
  general: '通用',
}

const OUTCOME_ZH: Record<string, string> = {
  yes: '是',
  no: '否',
  up: '上涨',
  down: '下跌',
  over: '高于',
  under: '低于',
}

const MONTH_ZH: Record<string, string> = {
  january: '1月',
  february: '2月',
  march: '3月',
  april: '4月',
  may: '5月',
  june: '6月',
  july: '7月',
  august: '8月',
  september: '9月',
  october: '10月',
  november: '11月',
  december: '12月',
}

const PHRASE_REPLACEMENTS: Array<[RegExp, string]> = [
  [/\bU\.S\.\b/gi, '美国'],
  [/\bUS\b/g, '美国'],
  [/\bUnited States\b/gi, '美国'],
  [/\bPresidential Election\b/gi, '总统选举'],
  [/\bPresident\b/gi, '总统'],
  [/\bElection\b/gi, '选举'],
  [/\bFederal Reserve\b/gi, '美联储'],
  [/\bFed\b/g, '美联储'],
  [/\binterest rates?\b/gi, '利率'],
  [/\binflation\b/gi, '通胀'],
  [/\brecession\b/gi, '衰退'],
  [/\bBitcoin\b/gi, '比特币'],
  [/\bEthereum\b/gi, '以太坊'],
  [/\bBTC\b/g, 'BTC'],
  [/\bETH\b/g, 'ETH'],
  [/\bETF\b/g, 'ETF'],
  [/\bSuper Bowl\b/gi, '超级碗'],
  [/\bNBA\b/g, 'NBA'],
  [/\bNFL\b/g, 'NFL'],
  [/\bWorld Cup\b/gi, '世界杯'],
  [/\bmarket\b/gi, '市场'],
  [/\bmarkets\b/gi, '市场'],
  [/\bprice\b/gi, '价格'],
  [/\bvolume\b/gi, '成交量'],
  [/\bbefore\b/gi, '之前'],
  [/\bafter\b/gi, '之后'],
  [/\bon\b/gi, '在'],
  [/\bby\b/gi, '截至'],
]

export function localizePredictionMarketText(
  fallback: string | null | undefined,
  locale: string,
  kind: PredictionTextKind,
  document?: PredictionLocalizedTextDocument | null,
): string {
  const fallbackText = normalizeText(fallback)
  const configuredText = pickConfiguredText(document, locale)
  if (configuredText) return configuredText
  if (!isChineseLocale(locale) || !fallbackText || containsCjk(fallbackText)) return fallbackText
  if (kind === 'category') return localizeCategory(fallbackText)
  if (kind === 'outcome') return localizeOutcome(fallbackText)
  return localizeEnglishMarketText(fallbackText)
}

function pickConfiguredText(document: PredictionLocalizedTextDocument | null | undefined, locale: string): string {
  const items = Array.isArray(document?.items) ? document.items : []
  if (items.length === 0) return ''
  const normalizedLocale = normalizeLocale(locale)
  const normalizedLanguage = normalizedLocale.split('-')[0]
  const defaultLocale = normalizeLocale(document?.default_locale || '')
  const valueFor = (item: PredictionLocalizedTextItem | undefined) => normalizeText(
    item?.title || item?.text || item?.value || item?.description || item?.label || item?.name || '',
  )
  const localeOf = (item: PredictionLocalizedTextItem) => normalizeLocale(item.locale || item.country || '')

  return (
    valueFor(items.find((item) => localeOf(item) === normalizedLocale)) ||
    valueFor(items.find((item) => localeOf(item).split('-')[0] === normalizedLanguage)) ||
    valueFor(items.find((item) => localeOf(item) === defaultLocale)) ||
    valueFor(items[0])
  )
}

function localizeCategory(value: string): string {
  const normalized = value.trim().toLowerCase()
  return CATEGORY_ZH[normalized] || applyPhraseReplacements(value)
}

function localizeOutcome(value: string): string {
  const normalized = value.trim().toLowerCase()
  return OUTCOME_ZH[normalized] || applyPhraseReplacements(value)
}

function localizeEnglishMarketText(value: string): string {
  const text = normalizeText(value)
  const plainQuestion = text.replace(/\?+$/, '').trim()
  const fedCuts = /^Will\s+(no|\d+|\d+\s+or\s+more)\s+Fed\s+rate\s+cuts?\s+happen\s+in\s+(\d{4})$/i.exec(plainQuestion)
  if (fedCuts) {
    const cutCount = fedCuts[1].toLowerCase()
    const countText = cutCount === 'no' ? '不' : `${cutCount.replace(/\s+or\s+more/i, ' 次或更多')}次`
    return `${fedCuts[2]}年美联储会${countText}降息吗？`
  }

  const fdvAfterLaunch = /^(.+?)\s+FDV\s+(above|below|over|under)\s+(.+?)\s+one\s+day\s+after\s+launch$/i.exec(plainQuestion)
  if (fdvAfterLaunch) {
    const direction = /above|over/i.test(fdvAfterLaunch[2]) ? '高于' : '低于'
    return `${applyPhraseReplacements(fdvAfterLaunch[1])} 代币上线后一天 FDV 会${direction}${applyPhraseReplacements(fdvAfterLaunch[3])}吗？`
  }

  const hitBy = /^Will\s+(.+?)\s+(?:hit|reach)\s+(.+?)\s+by\s+(.+)$/i.exec(plainQuestion)
  if (hitBy) {
    return `${applyPhraseReplacements(hitBy[1])}会在${localizeDateText(hitBy[3])}前达到${applyPhraseReplacements(hitBy[2])}吗？`
  }

  const win = /^Will\s+(.+?)\s+win\s+(.+)$/i.exec(plainQuestion)
  if (win) {
    return `${applyPhraseReplacements(win[1])}会赢得${applyPhraseReplacements(win[2])}吗？`
  }

  const aboveBelow = /^Will\s+(.+?)\s+be\s+(above|below|over|under)\s+(.+?)(?:\s+(?:on|by)\s+(.+))?$/i.exec(plainQuestion)
  if (aboveBelow) {
    const direction = /above|over/i.test(aboveBelow[2]) ? '高于' : '低于'
    const date = aboveBelow[4] ? `，时间为${localizeDateText(aboveBelow[4])}` : ''
    return `${applyPhraseReplacements(aboveBelow[1])}会${direction}${applyPhraseReplacements(aboveBelow[3])}${date}吗？`
  }

  const will = /^Will\s+(.+)$/i.exec(plainQuestion)
  if (will) {
    return `${applyPhraseReplacements(will[1])}吗？`
  }

  return applyPhraseReplacements(text)
}

function applyPhraseReplacements(value: string): string {
  let output = localizeDateText(value)
  for (const [pattern, replacement] of PHRASE_REPLACEMENTS) {
    output = output.replace(pattern, replacement)
  }
  return output
    .replace(/\s+/g, ' ')
    .replace(/\s+([，。？！])/g, '$1')
    .trim()
}

function localizeDateText(value: string): string {
  return value.replace(
    /\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{1,2})(?:,\s*(\d{4}))?\b/gi,
    (match, month: string, day: string, year?: string) => {
      const localizedMonth = MONTH_ZH[month.toLowerCase()]
      if (!localizedMonth) return match
      return `${year ? `${year}年` : ''}${localizedMonth}${Number(day)}日`
    },
  )
}

function isChineseLocale(locale: string): boolean {
  return normalizeLocale(locale).startsWith('zh')
}

function normalizeLocale(locale: string): string {
  return String(locale || '').trim().replace('_', '-').toLowerCase()
}

function normalizeText(value: string | null | undefined): string {
  return String(value || '').replace(/\s+/g, ' ').trim()
}

function containsCjk(value: string): boolean {
  return /[\u3400-\u9fff]/.test(value)
}
