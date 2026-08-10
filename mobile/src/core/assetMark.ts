export function buildAssetMarkImageSources(...values: readonly unknown[]): string[] {
  return [...new Set(
    values
      .map((value) => typeof value === 'string' ? value.trim() : '')
      .filter(Boolean),
  )]
}

export function assetMarkImageSourceAt(
  sources: readonly string[],
  imageIndex: number,
): string | undefined {
  return Number.isSafeInteger(imageIndex) && imageIndex >= 0
    ? sources[imageIndex]
    : undefined
}
