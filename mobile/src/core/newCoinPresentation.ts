import {
  decimalCompare,
  decimalDivide,
  decimalMinimum,
  decimalUnitRatioToNumber,
  normalizeDecimalText,
  type DecimalText,
} from './decimal.ts'
import type {
  NewCoinDistribution,
  NewCoinProject,
  NewCoinPurchase,
  NewCoinSubscription,
  NewCoinUnlock,
} from './newCoinModel.ts'
import type { MarketTicker } from './types.ts'

export type NewCoinLifecycleFilter = 'all' | 'preheat' | 'subscription' | 'distribution' | 'listed'
export type NewCoinOpportunityFilter = 'all' | 'upcoming' | 'listedToday' | 'hotGains'
export type NewCoinRecordKind = 'subscription' | 'distribution' | 'purchase' | 'unlock'
export type NewCoinRecordTypeFilter = 'all' | `${NewCoinRecordKind}s`
export type NewCoinRecordStatusFilter = 'all' | 'inProgress' | 'pendingSettlement' | 'completed'
export type NewCoinUnlockTypeTranslationKey =
  | 'newCoin.immediateUnlock'
  | 'newCoin.fixedUnlock'
  | 'newCoin.relativeUnlock'

const NEW_COIN_UNLOCK_TYPE_TRANSLATION_KEYS: Readonly<Record<string, NewCoinUnlockTypeTranslationKey>> = {
  immediate_on_listing: 'newCoin.immediateUnlock',
  fixed_time: 'newCoin.fixedUnlock',
  relative_period: 'newCoin.relativeUnlock',
}

export interface NewCoinOpportunity {
  project: NewCoinProject
  ticker: MarketTicker
}

export interface UnifiedNewCoinRecord {
  key: string
  kind: NewCoinRecordKind
  id: number
  recordNo: string
  createdAt: number
  status: string
  statusBucket: Exclude<NewCoinRecordStatusFilter, 'all'>
  project?: NewCoinProject
  assetId?: number
  assetSymbol?: string
  assetLogoUrl?: string
  subscription?: NewCoinSubscription
  distribution?: NewCoinDistribution
  purchase?: NewCoinPurchase
  unlock?: NewCoinUnlock
}

export function filterNewCoinProjects(
  projects: readonly NewCoinProject[],
  filter: NewCoinLifecycleFilter,
): NewCoinProject[] {
  if (filter === 'all') return [...projects]
  return projects.filter((project) => project.lifecycleStatus.toLowerCase() === filter)
}

export function newCoinProjectProgress(project: NewCoinProject): {
  ratio: DecimalText
  percentage: number
} {
  const zero = normalizeDecimalText('0')
  const one = normalizeDecimalText('1')
  if (decimalCompare(project.totalSupplyText, zero) <= 0) return { ratio: zero, percentage: 0 }
  const nonNegativeReserved = decimalCompare(project.reservedSupplyText, zero) < 0
    ? zero
    : project.reservedSupplyText
  const ratio = decimalMinimum(decimalDivide(nonNegativeReserved, project.totalSupplyText, 18), one) || zero
  return { ratio, percentage: decimalUnitRatioToNumber(ratio) * 100 }
}

export function newCoinLifecycleMilestone(lifecycleStatus: string): -1 | 0 | 1 | 2 | 3 {
  switch (lifecycleStatus.toLowerCase()) {
    case 'preheat': return 0
    case 'subscription': return 1
    case 'distribution': return 2
    case 'listed':
    case 'closed': return 3
    default: return -1
  }
}

export function buildNewCoinOpportunities(
  projects: readonly NewCoinProject[],
  tickers: readonly MarketTicker[],
  filter: NewCoinOpportunityFilter,
  now = Date.now(),
): NewCoinOpportunity[] {
  const tickerById = new Map(
    tickers
      .filter((ticker): ticker is MarketTicker & { id: number } => Number.isSafeInteger(ticker.id) && (ticker.id || 0) > 0)
      .map((ticker) => [ticker.id, ticker]),
  )
  const opportunities = projects.flatMap((project) => {
    if (!project.postListingPairId) return []
    const ticker = tickerById.get(project.postListingPairId)
    return ticker ? [{ project, ticker }] : []
  })

  const filtered = opportunities.filter(({ project, ticker }) => {
    if (filter === 'all') return true
    if (filter === 'upcoming') return project.listedAt !== undefined && project.listedAt > now
    if (filter === 'hotGains') return ticker.changePercent > 0
    return project.listedAt !== undefined && sameLocalDay(project.listedAt, now)
  })

  return filtered.sort((left, right) => {
    if (filter === 'hotGains') return right.ticker.changePercent - left.ticker.changePercent
    return (right.project.listedAt || 0) - (left.project.listedAt || 0)
      || right.project.id - left.project.id
  })
}

export function newCoinUnlockTypeTranslationKey(
  unlockType: string,
): NewCoinUnlockTypeTranslationKey | undefined {
  return NEW_COIN_UNLOCK_TYPE_TRANSLATION_KEYS[unlockType.trim().toLowerCase()]
}

export function buildUnifiedNewCoinRecords(input: {
  projects: readonly NewCoinProject[]
  subscriptions: readonly NewCoinSubscription[]
  distributions: readonly NewCoinDistribution[]
  purchases: readonly NewCoinPurchase[]
  unlocks: readonly NewCoinUnlock[]
}): UnifiedNewCoinRecord[] {
  const projectById = new Map(input.projects.map((project) => [project.id, project]))
  const projectByAssetId = new Map(input.projects.map((project) => [project.assetId, project]))
  const records: UnifiedNewCoinRecord[] = []

  for (const subscription of input.subscriptions) {
    const project = projectById.get(subscription.projectId)
    records.push(baseRecord('subscription', subscription, project, {
      subscription,
      assetId: project?.assetId,
      assetSymbol: project?.symbol,
      assetLogoUrl: project?.logoUrl,
    }))
  }
  for (const distribution of input.distributions) {
    const project = projectById.get(distribution.projectId) || projectByAssetId.get(distribution.assetId)
    records.push(baseRecord('distribution', distribution, project, {
      distribution,
      assetId: distribution.assetId,
      assetSymbol: project?.symbol,
      assetLogoUrl: project?.logoUrl,
    }))
  }
  for (const purchase of input.purchases) {
    const project = projectById.get(purchase.projectId) || projectByAssetId.get(purchase.baseAssetId)
    records.push(baseRecord('purchase', purchase, project, {
      purchase,
      assetId: purchase.baseAssetId,
      assetSymbol: project?.symbol,
      assetLogoUrl: project?.logoUrl,
    }))
  }
  for (const unlock of input.unlocks) {
    const project = projectByAssetId.get(unlock.assetId)
    const record = baseRecord('unlock', unlock, project, {
      unlock,
      assetId: unlock.assetId,
      assetSymbol: project?.symbol,
      assetLogoUrl: project?.logoUrl,
    })
    if (
      record.statusBucket !== 'completed'
      && unlock.unlockFeeEnabled
      && !['paid', 'not_required'].includes(unlock.feePaidStatus.toLowerCase())
    ) {
      record.statusBucket = 'pendingSettlement'
    }
    records.push(record)
  }

  return records.sort((left, right) => right.createdAt - left.createdAt
    || left.kind.localeCompare(right.kind)
    || right.id - left.id)
}

export function filterUnifiedNewCoinRecords(
  records: readonly UnifiedNewCoinRecord[],
  typeFilter: NewCoinRecordTypeFilter,
  statusFilter: NewCoinRecordStatusFilter,
): UnifiedNewCoinRecord[] {
  return records.filter((record) => {
    const typeMatches = typeFilter === 'all' || typeFilter === `${record.kind}s`
    const statusMatches = statusFilter === 'all' || statusFilter === record.statusBucket
    return typeMatches && statusMatches
  })
}

export function newCoinRecordStatusBucket(
  status: string,
): Exclude<NewCoinRecordStatusFilter, 'all'> {
  const normalized = status.toLowerCase()
  if (['completed', 'distributed', 'available', 'released', 'cancelled', 'canceled'].includes(normalized)) {
    return 'completed'
  }
  if (['allocated', 'locked', 'paid', 'unpaid', 'not_required'].includes(normalized)) {
    return 'pendingSettlement'
  }
  return 'inProgress'
}

function baseRecord(
  kind: NewCoinRecordKind,
  source: { id: number; status: string; idempotencyKey: string; createdAt: number },
  project: NewCoinProject | undefined,
  extension: Partial<UnifiedNewCoinRecord>,
): UnifiedNewCoinRecord {
  return {
    key: `${kind}-${source.id}`,
    kind,
    id: source.id,
    recordNo: source.idempotencyKey,
    createdAt: source.createdAt,
    status: source.status,
    statusBucket: newCoinRecordStatusBucket(source.status),
    project,
    ...extension,
  }
}

function sameLocalDay(left: number, right: number): boolean {
  const leftDate = new Date(left)
  const rightDate = new Date(right)
  return leftDate.getFullYear() === rightDate.getFullYear()
    && leftDate.getMonth() === rightDate.getMonth()
    && leftDate.getDate() === rightDate.getDate()
}
