# Public News Contract

## Scenario: PC public news API integration

### 1. Scope / Trigger
- Trigger: Public news spans admin content creation, MySQL storage, `/api/v1/news` responses, PC API adapters, and Vue rendering.
- Any change to public news response fields, locale filtering, or rich-text content must preserve this cross-layer contract.

### 2. Signatures
- List API: `GET /api/v1/news`
- Detail API: `GET /api/v1/news/{id}`
- Query fields: `category`, `country_code`, `locale`, `q`, `limit`, `offset`
- Backend storage table: `admin_news_items`
- Content fields: `content_json.items[*].summary`, `content_json.items[*].content`

### 3. Contracts
- `content_json.items[*].locale` stores full or short locales such as `zh-CN`, `en-US`, `zh`, or `en`.
- PC locale values are short codes (`zh`, `en`) and must match backend locale families (`zh-CN`, `en-US`) when selecting or filtering news.
- `content_json.items[*].summary` and `content_json.items[*].content` are rich-text blocks, not raw HTML strings:
  ```json
  [
    { "type": "p", "children": [{ "text": "正文", "bold": true }] },
    { "type": "image", "url": "https://cdn.example.test/news/body.png", "alt": "新闻配图" }
  ]
  ```
- Existing string summaries are accepted for backward compatibility, but new admin submissions should send summary rich-text blocks.
- Rich-text image blocks are allowed in admin news content. Text blocks must use `children`; image blocks must use `url` with optional `alt`.
- Public response must not expose admin-only fields such as `created_by_admin_id`, `updated_by_admin_id`, secrets, tokens, or ciphertext.
- Image fields are public: `banner_url`, `small_logo_url`.
- Public `category` is the admin-configured backend category and PC must preserve it verbatim:
  `general | market | product | system | promotion`. Do not remap these into PC-only categories such as
  `flash`, `deep`, or `announcement`.

### 4. Validation & Error Matrix
- Unsupported `category` -> `validation error: unsupported news category`
- Invalid `country_code` -> `validation error: news country_code format is invalid`
- Invalid `locale` -> `validation error: news locale format is invalid`
- Missing MySQL pool -> internal error from public news route
- Missing published item on detail route -> `404 NotFound`

### 5. Good/Base/Bad Cases
- Good: PC requests `locale=zh`; backend returns news with `zh` or `zh-CN` translations; PC adapter chooses `zh-CN` content.
- Base: PC locale is unsupported; adapter falls back to `default_locale`, then first content item.
- Bad: Treating rich-text blocks as strings renders `[object Object]` in PC details.

### 6. Tests Required
- Backend unit test for locale search patterns: `zh -> ["zh", "zh-%"]`, `en-US -> ["en-US", "en"]`.
- PC adapter test for selecting `zh-CN` when current locale is `zh`.
- PC adapter test for converting rich-text blocks into escaped HTML and plain-text summaries.
- PC adapter test for converting rich-text summary blocks into plain text for list/detail summary surfaces.
- PC adapter test for rendering rich-text image blocks as escaped `<img>` HTML without adding image URLs to text summaries.
- PC adapter/source test for preserving backend news categories and passing backend category filters unchanged.
- OpenAPI test must keep public response schemas free of admin-only fields.

### 7. Wrong vs Correct
#### Wrong
```typescript
const contentBody = content.content || ''
return { content: contentBody }
```

#### Correct
```typescript
const contentText = newsContentToPlainText(content.content)
const contentHtml = newsContentToHtml(content.content)
return { summary: content.summary || contentText.slice(0, 180), content: contentHtml }
```

#### Wrong
```typescript
normalized.category = pcCategoryToBackend(params.category)
return { category: newsCategoryToPc(item.category) }
```

#### Correct
```typescript
normalized.category = params.category
return { category: normalizeBackendNewsCategory(item.category) }
```
