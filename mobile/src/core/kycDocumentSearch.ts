import { normalizeCountrySearchText } from './countrySearch.ts'

export interface KycDocumentTypeSearchOption {
  value: string
  label: string
  searchAliases?: readonly string[]
}

/** Filter configured KYC document types without changing their backend order or raw value. */
export function filterDocumentTypeOptions<T extends KycDocumentTypeSearchOption>(
  options: readonly T[],
  query: unknown,
): T[] {
  const normalizedQuery = normalizeCountrySearchText(query)
  if (!normalizedQuery) return [...options]

  const tokens = normalizedQuery.split(' ')
  return options.filter((option) => {
    const searchableText = normalizeCountrySearchText([
      option.value,
      option.label,
      ...(option.searchAliases || []),
    ].join(' '))
    return tokens.every((token) => searchableText.includes(token))
  })
}
