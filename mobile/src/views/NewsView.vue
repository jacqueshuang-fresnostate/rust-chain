<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { CircleAlert, LoaderCircle, Newspaper, Search, X } from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNews, type NewsItem } from '@/api/news'
import { formatDateTime } from '@/core/format'

type NewsCategory = 'all' | 'market' | 'product' | 'research'

const NEWS_CATEGORIES: readonly NewsCategory[] = ['all', 'market', 'product', 'research']

function normalizeNewsCategory(value: unknown): NewsCategory {
  return typeof value === 'string' && NEWS_CATEGORIES.includes(value as NewsCategory)
    ? value as NewsCategory
    : 'all'
}

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const rows = ref<NewsItem[]>([])
const loading = ref(false)
const error = ref('')
const activeCategory = ref<NewsCategory>(normalizeNewsCategory(route.query.category))
const query = ref('')
const searchOpen = ref(false)
const categories = computed<Array<{ value: NewsCategory; label: string }>>(() => [
  { value: 'all', label: t('news.all') },
  { value: 'market', label: t('news.market') },
  { value: 'product', label: t('news.product') },
  { value: 'research', label: t('news.research') },
])
const categoryMarkers = computed<Record<string, string[]>>(() => ({
  market: ['market', t('news.market'), t('nav.markets')],
  product: ['product', t('news.product')],
  research: ['research', t('news.research')],
}))

function matchesCategory(category: string | undefined, selected: NewsCategory): boolean {
  if (selected === 'all') return true
  const normalized = category?.trim().toLocaleLowerCase() || ''
  return Boolean(normalized && categoryMarkers.value[selected]
    ?.some((marker) => normalized.includes(marker.toLocaleLowerCase())))
}

const visibleRows = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return rows.value.filter((item) => (
    matchesCategory(item.category, activeCategory.value)
    && (!needle || item.title.toLocaleLowerCase().includes(needle))
  ))
})
const featured = computed(() => visibleRows.value[0])
const listRows = computed(() => visibleRows.value.slice(1))

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try { rows.value = await fetchNews(50) } catch (reason) { error.value = apiErrorMessage(reason, t('news.loadFailed')) } finally { loading.value = false }
}

watch(
  () => route.query.category,
  (category) => { activeCategory.value = normalizeNewsCategory(category) },
)

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain pencil-page news-pencil" data-pencil-source="VGPW0 b6EGF">
    <PageHeader class="news-pencil__header" :back="true" :pencil="true" :title="t('news.title')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('news.search')" :aria-pressed="searchOpen" @click="searchOpen = !searchOpen">
          <X v-if="searchOpen" :size="20" /><Search v-else :size="20" />
        </button>
      </template>
    </PageHeader>

    <div class="pencil-content news-pencil__content" :aria-busy="loading">
      <label v-if="searchOpen" class="news-search">
        <Search :size="17" />
        <input v-model="query" type="search" :placeholder="t('news.searchPlaceholder')" autofocus />
      </label>

      <nav class="pencil-segmented news-categories" :aria-label="t('news.categories')">
        <button
          v-for="category in categories.slice(0, 4)"
          :key="category.value"
          type="button"
          :aria-pressed="activeCategory === category.value"
          @click="activeCategory = category.value"
        >
          {{ category.label }}
        </button>
      </nav>

      <div v-if="loading" class="pencil-state" role="status">
        <LoaderCircle :size="22" class="spin" /><span>{{ t('news.loading') }}</span>
      </div>
      <div v-else-if="error" class="pencil-state news-state--error" role="alert">
        <CircleAlert :size="22" /><span>{{ error }}</span>
        <button class="pencil-secondary" type="button" @click="load">{{ t('common.retry') }}</button>
      </div>

      <template v-else-if="featured">
        <button
          class="news-feature"
          type="button"
          :aria-label="featured.title"
          @click="router.push({ name: 'news-detail', params: { id: featured.id } })"
        >
          <span
            class="news-feature__visual"
            :class="{ 'news-feature__visual--empty': !featured.bannerUrl }"
            :style="featured.bannerUrl ? { backgroundImage: `url(${featured.bannerUrl})` } : undefined"
          />
          <span class="news-feature__meta">
            <span>{{ featured.category || t('news.title') }}</span>
            <template v-if="featured.publishedAt">
              <i aria-hidden="true">·</i>
              <time>{{ formatDateTime(featured.publishedAt) }}</time>
            </template>
          </span>
          <strong>{{ featured.title }}</strong>
        </button>

        <div v-if="listRows.length" class="news-list-pencil">
          <button
            v-for="notice in listRows"
            :key="notice.id"
            type="button"
            :aria-label="notice.title"
            @click="router.push({ name: 'news-detail', params: { id: notice.id } })"
          >
            <span
              class="news-list-pencil__visual"
              :style="notice.bannerUrl ? { backgroundImage: `url(${notice.bannerUrl})` } : undefined"
            />
            <span class="news-list-pencil__copy">
              <small>{{ notice.category || t('news.title') }}</small>
              <strong>{{ notice.title }}</strong>
              <time v-if="notice.publishedAt">{{ formatDateTime(notice.publishedAt) }}</time>
            </span>
          </button>
        </div>
      </template>

      <div v-else class="pencil-state">
        <Newspaper :size="23" /><span>{{ query ? t('news.noSearchResults') : t('news.empty') }}</span>
      </div>
    </div>
  </main>
</template>

<style scoped>
.news-pencil {
  background: var(--page);
  color: var(--ink);
}

.news-pencil :deep(.news-pencil__header.pencil-page-header) {
  --pencil-root-header-margin: 8px;
}

.news-pencil__content {
  padding-top: 8px;
}

.news-search {
  align-items: center;
  background: var(--surface-2);
  border: 1px solid transparent;
  border-radius: 12px;
  display: flex;
  gap: 9px;
  margin: 0 0 8px;
  min-height: 44px;
  padding: 0 12px;
}

.news-search:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.news-search input {
  background: transparent;
  border: 0;
  color: var(--ink);
  min-height: 42px;
  min-width: 0;
  outline: 0;
  width: 100%;
}

.news-search input:focus-visible {
  outline: 0;
}

.news-categories {
  gap: 21px;
}

.news-categories button[aria-pressed='true'] {
  font-weight: 650;
}

.news-feature {
  background: transparent;
  box-sizing: border-box;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-rows: 140px auto minmax(0, 1fr);
  height: 243px;
  margin-top: 8px;
  padding: 8px 0;
  text-align: left;
  width: 100%;
}

.news-feature__visual {
  background-position: center;
  background-size: cover;
  border-radius: 12px;
  display: block;
  height: 140px;
  overflow: hidden;
  width: 100%;
}

.news-feature__visual--empty {
  background: linear-gradient(
    45deg,
    var(--accent),
    color-mix(in srgb, var(--accent) 42%, var(--on-accent)) 55%,
    var(--on-accent)
  );
}

.news-feature__meta {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 10px;
  font-weight: 500;
  gap: 6px;
  line-height: 13px;
  min-width: 0;
}

.news-feature__meta i {
  font-style: normal;
}

.news-feature__meta time {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.news-feature > strong {
  display: -webkit-box;
  font-size: 18px;
  font-weight: 750;
  letter-spacing: 0;
  line-height: 1.25;
  overflow: hidden;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.news-list-pencil {
  display: grid;
  margin-top: 8px;
  padding-top: 4px;
}

.news-list-pencil > button {
  align-items: center;
  background: transparent;
  border: 0;
  box-sizing: border-box;
  color: var(--ink);
  display: grid;
  gap: 12px;
  grid-template-columns: 64px minmax(0, 1fr);
  height: 88px;
  min-height: 88px;
  padding: 12px 0;
  text-align: left;
  width: 100%;
}

.news-feature:focus-visible,
.news-list-pencil > button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.news-list-pencil__visual {
  align-items: center;
  background-color: var(--surface-2);
  background-position: center;
  background-size: cover;
  border-radius: 10px;
  color: var(--positive);
  display: flex;
  height: 64px;
  justify-content: center;
  width: 64px;
}

.news-list-pencil__copy {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.news-list-pencil__copy small {
  color: var(--positive);
  font-size: 10px;
  font-weight: 600;
  line-height: 13px;
}

.news-list-pencil__copy strong {
  display: -webkit-box;
  font-size: 13px;
  font-weight: 650;
  line-height: 16px;
  overflow: hidden;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.news-list-pencil__copy time {
  color: var(--muted);
  font-size: 10px;
  font-weight: 450;
  line-height: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.news-pencil__content > .pencil-state {
  margin-top: 8px;
  min-height: 243px;
}

.news-state--error {
  color: var(--negative);
}

.spin {
  animation: news-spin .8s linear infinite;
}

@keyframes news-spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .news-list-pencil > button {
    gap: 10px;
    grid-template-columns: 58px minmax(0, 1fr);
  }

  .news-list-pencil__visual {
    height: 58px;
    width: 58px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
