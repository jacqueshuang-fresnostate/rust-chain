export type RovingListboxNavigationKey = 'ArrowDown' | 'ArrowUp' | 'Home' | 'End'

/** Keeps the current active option stable while it remains in a filtered list. */
export function stableRovingOptionId<T extends string | number>(
  optionIds: readonly T[],
  activeId: T | null,
  selectedId: T | null,
): T | null {
  if (!optionIds.length) return null
  if (activeId !== null && optionIds.includes(activeId)) return activeId
  if (selectedId !== null && optionIds.includes(selectedId)) return selectedId
  return optionIds[0] ?? null
}

/** Resolves Arrow/Home/End movement for a focus-owning roving option list. */
export function moveRovingOptionId<T extends string | number>(
  optionIds: readonly T[],
  activeId: T | null,
  key: RovingListboxNavigationKey,
): T | null {
  if (!optionIds.length) return null
  if (key === 'Home') return optionIds[0] ?? null
  if (key === 'End') return optionIds.at(-1) ?? null

  const currentIndex = activeId === null ? -1 : optionIds.indexOf(activeId)
  if (key === 'ArrowDown') {
    return optionIds[currentIndex < 0 ? 0 : (currentIndex + 1) % optionIds.length] ?? null
  }
  const previousIndex = currentIndex < 0
    ? optionIds.length - 1
    : (currentIndex - 1 + optionIds.length) % optionIds.length
  return optionIds[previousIndex] ?? null
}

export function isRovingListboxSelectionKey(key: string, code = ''): boolean {
  return key === 'Enter' || key === ' ' || key === 'Spacebar' || code === 'Space'
}
