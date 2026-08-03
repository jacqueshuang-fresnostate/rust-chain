export interface MarketChartThemeObserver {
  observe(target: Node, options?: MutationObserverInit): void
  disconnect(): void
}

export type MarketChartThemeObserverFactory = (
  callback: MutationCallback,
) => MarketChartThemeObserver

export interface MarketChartTheme {
  background: string
  grid: string
  muted: string
  negative: string
  positive: string
  ma5: string
  ma10: string
  ma20: string
}

function createBrowserThemeObserver(callback: MutationCallback): MarketChartThemeObserver {
  return new MutationObserver(callback)
}

export function readMarketChartTheme(element: Element): MarketChartTheme {
  const styles = getComputedStyle(element)
  const ink = styles.getPropertyValue('--ink').trim() || styles.color
  const muted = styles.getPropertyValue('--muted').trim() || ink
  const negative = styles.getPropertyValue('--negative').trim() || ink
  const positive = styles.getPropertyValue('--positive').trim() || ink

  return {
    background: styles.getPropertyValue('--surface').trim() || styles.backgroundColor,
    grid: styles.getPropertyValue('--line').trim() || muted,
    muted,
    negative,
    positive,
    ma5: styles.getPropertyValue('--yellow').trim()
      || styles.getPropertyValue('--signal-yellow').trim()
      || positive,
    ma10: styles.getPropertyValue('--coral').trim()
      || styles.getPropertyValue('--signal-coral').trim()
      || negative,
    ma20: styles.getPropertyValue('--cyan').trim()
      || styles.getPropertyValue('--signal-blue').trim()
      || muted,
  }
}

export function marketChartColorWithAlpha(color: string, alpha: number): string {
  const hex = /^#([\da-f]{3}|[\da-f]{6})$/i.exec(color)
  if (hex) {
    const value = hex[1].length === 3
      ? hex[1].split('').map((part) => `${part}${part}`).join('')
      : hex[1]
    return `rgba(${Number.parseInt(value.slice(0, 2), 16)}, ${Number.parseInt(value.slice(2, 4), 16)}, ${Number.parseInt(value.slice(4, 6), 16)}, ${alpha})`
  }
  const channels = color.match(/[\d.]+/g)
  if (channels && channels.length >= 3) {
    return `rgba(${channels[0]}, ${channels[1]}, ${channels[2]}, ${alpha})`
  }
  return color
}

export function observeMarketChartTheme(
  chartContainer: Element,
  documentRoot: Element,
  applyTheme: () => void,
  createObserver: MarketChartThemeObserverFactory = createBrowserThemeObserver,
): () => void {
  const observer = createObserver(() => applyTheme())
  const stage = chartContainer.closest('.app-stage')

  if (stage === documentRoot) {
    observer.observe(documentRoot, {
      attributes: true,
      attributeFilter: ['class', 'data-theme'],
    })
  } else {
    if (stage) {
      observer.observe(stage, {
        attributes: true,
        attributeFilter: ['class'],
      })
    }
    observer.observe(documentRoot, {
      attributes: true,
      attributeFilter: ['data-theme'],
    })
  }

  return () => observer.disconnect()
}
