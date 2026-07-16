export type PredictionTextKind = 'title' | 'description' | 'category' | 'outcome'

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
  [/\bSuper Bowl\b/gi, '超级碗'],
  [/\bWorld Cup\b/gi, '世界杯'],
  [/\bmarkets?\b/gi, '市场'],
  [/\bprice\b/gi, '价格'],
  [/\bvolume\b/gi, '成交量'],
]

export function localizePredictionMarketText(
  value: string | null | undefined,
  locale: string,
  kind: PredictionTextKind,
): string {
  const text = normalizeText(value)
  if (!normalizeLocale(locale).startsWith('zh') || !text || containsCjk(text)) return text
  if (kind === 'category') return CATEGORY_ZH[text.toLowerCase()] || applyPhraseReplacements(text)
  if (kind === 'outcome') return OUTCOME_ZH[text.toLowerCase()] || applyPhraseReplacements(text)
  return localizeEnglishMarketText(text)
}

function localizeEnglishMarketText(value: string): string {
  const plainQuestion = value.replace(/\?+$/, '').trim()
  const hitBy = /^Will\s+(.+?)\s+(?:hit|reach)\s+(.+?)\s+by\s+(.+)$/i.exec(plainQuestion)
  if (hitBy) return `${applyPhraseReplacements(hitBy[1])}会在${localizeDateText(hitBy[3])}前达到${applyPhraseReplacements(hitBy[2])}吗？`

  const win = /^Will\s+(.+?)\s+win\s+(.+)$/i.exec(plainQuestion)
  if (win) return `${applyPhraseReplacements(win[1])}会赢得${applyPhraseReplacements(win[2])}吗？`

  const aboveBelow = /^Will\s+(.+?)\s+be\s+(above|below|over|under)\s+(.+?)(?:\s+(?:on|by)\s+(.+))?$/i.exec(plainQuestion)
  if (aboveBelow) {
    const direction = /above|over/i.test(aboveBelow[2]) ? '高于' : '低于'
    const date = aboveBelow[4] ? `，时间为${localizeDateText(aboveBelow[4])}` : ''
    return `${applyPhraseReplacements(aboveBelow[1])}会${direction}${applyPhraseReplacements(aboveBelow[3])}${date}吗？`
  }

  const will = /^Will\s+(.+)$/i.exec(plainQuestion)
  if (will) return `${applyPhraseReplacements(will[1])}吗？`
  return applyPhraseReplacements(value)
}

function applyPhraseReplacements(value: string): string {
  let output = localizeDateText(value)
  for (const [pattern, replacement] of PHRASE_REPLACEMENTS) output = output.replace(pattern, replacement)
  return output.replace(/\s+/g, ' ').replace(/\s+([，。？！])/g, '$1').trim()
}

function localizeDateText(value: string): string {
  return value.replace(
    /\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{1,2})(?:,\s*(\d{4}))?\b/gi,
    (match, month: string, day: string, year?: string) => {
      const localizedMonth = MONTH_ZH[month.toLowerCase()]
      return localizedMonth ? `${year ? `${year}年` : ''}${localizedMonth}${Number(day)}日` : match
    },
  )
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
