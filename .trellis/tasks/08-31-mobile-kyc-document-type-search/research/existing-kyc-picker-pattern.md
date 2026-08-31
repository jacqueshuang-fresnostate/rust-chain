# Existing KYC picker pattern

## Inspected implementation

- `mobile/src/views/KycView.vue` derives document types from the selected country rule and currently renders them with a native `select`.
- The same view already owns a body-Teleported searchable country picker using `useModalDialog`, including initial focus, body scroll lock, Tab trapping, Escape/backdrop dismissal, focus restoration, localized empty state, theme-safe surfaces, and constrained/reduced-motion blur fallback.
- `mobile/src/core/countrySearch.ts` provides deterministic Unicode normalization that is already tested for punctuation, full-width forms, accents, whitespace, and case.
- `form.documentType` is passed unchanged as `document_type` by `submitKycApplication`; the search UI must therefore preserve each configured raw value.

## Chosen reuse boundary

- Reuse the existing modal helper and visual surface classes for both KYC pickers.
- Keep country and document picker state independent because they have different option structures and selection side effects.
- Add a small pure document-option filter that imports the established normalization function. Two modal templates do not yet justify a generic component; extracting one now would increase migration risk for the already verified country flow.

## Edge cases

- Unknown backend document types remain visible and searchable through their raw value.
- Empty queries return all options in backend order.
- A query with no result leaves `form.documentType` unchanged.
- Country changes continue to use the existing watcher to move an invalid document type to the first valid configured option.
