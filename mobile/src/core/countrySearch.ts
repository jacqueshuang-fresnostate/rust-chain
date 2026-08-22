export interface CountrySearchOption {
  code: string
  name: string
}

const COMBINING_MARKS_PATTERN = /\p{M}+/gu
const SEARCH_SEPARATOR_PATTERN = /[^\p{L}\p{N}]+/gu
const REPEATED_SPACE_PATTERN = /\s+/g

/** Normalize country-search input while preserving every writing system. */
export function normalizeCountrySearchText(value: unknown): string {
  return String(value ?? '')
    .normalize('NFKD')
    .replace(COMBINING_MARKS_PATTERN, '')
    .toLowerCase()
    .replace(SEARCH_SEPARATOR_PATTERN, ' ')
    .replace(REPEATED_SPACE_PATTERN, ' ')
    .trim()
}

/** Match every query token against ISO code, backend name, and localized name. */
export function filterCountryOptions<T extends CountrySearchOption>(
  countries: readonly T[],
  query: unknown,
  localizedLabel: (country: T) => string,
): T[] {
  const normalizedQuery = normalizeCountrySearchText(query)
  if (!normalizedQuery) return [...countries]

  const tokens = normalizedQuery.split(' ')
  return countries.filter((country) => {
    const searchableText = normalizeCountrySearchText([
      country.code,
      country.name,
      localizedLabel(country),
    ].join(' '))
    return tokens.every((token) => searchableText.includes(token))
  })
}
