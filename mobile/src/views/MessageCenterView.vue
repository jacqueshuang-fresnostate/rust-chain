<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, CircleAlert, Megaphone, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { apiErrorMessage } from '@/api/client'
import { fetchNews, type NewsItem } from '@/api/news'
import { formatDateTime } from '@/core/format'
import { goBackOr } from '@/core/navigation'

type MessageCategory = 'all' | 'account' | 'funds' | 'trade'

const MESSAGE_CATEGORIES: Array<{ value: MessageCategory; labelKey: string }> = [
  { value: 'all', labelKey: 'messageCenter.filterAll' },
  { value: 'account', labelKey: 'messageCenter.categoryAccount' },
  { value: 'funds', labelKey: 'messageCenter.categoryFunds' },
  { value: 'trade', labelKey: 'messageCenter.categoryTrade' },
]

const READ_IDS_STORAGE_KEY = 'hippo_mobile_message_read_ids'
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const messages = ref<NewsItem[]>([])
const readIds = ref(readStoredIds())
const activeCategory = ref<MessageCategory>('all')
const loading = ref(true)
const error = ref('')

const unreadCount = computed(() => messages.value.reduce(
  (count, message) => count + (readIds.value.has(message.id) ? 0 : 1),
  0,
))
const visibleMessages = computed(() => activeCategory.value === 'all' ? messages.value : [])
const activeCategoryLabel = computed(() => {
  const category = MESSAGE_CATEGORIES.find((item) => item.value === activeCategory.value)
  return category ? t(category.labelKey) : ''
})
const emptyTitle = computed(() => activeCategory.value === 'all'
  ? t('messageCenter.empty')
  : t('messageCenter.categoryEmpty', { category: activeCategoryLabel.value }))
const emptyDescription = computed(() => activeCategory.value === 'all'
  ? t('messageCenter.announcementEmptyDescription')
  : t('messageCenter.categoryEmptyDescription'))

async function loadMessages(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    messages.value = await fetchNews(40)
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('news.loadFailed'))
  } finally {
    loading.value = false
  }
}

function markRead(id: number): void {
  if (readIds.value.has(id)) return
  readIds.value = new Set([...readIds.value, id])
  persistReadIds()
}

function markAllRead(): void {
  readIds.value = new Set([...readIds.value, ...messages.value.map((message) => message.id)])
  persistReadIds()
}

function goBack(): void {
  void goBackOr(router, route.meta.backFallback || { name: 'home' })
}

function openMessage(message: NewsItem): void {
  markRead(message.id)
  void router.push({ name: 'news-detail', params: { id: String(message.id) } })
}

function readStoredIds(): Set<number> {
  try {
    const stored = globalThis.localStorage?.getItem(READ_IDS_STORAGE_KEY)
    const values = stored ? JSON.parse(stored) : []
    if (!Array.isArray(values)) return new Set()
    return new Set(values.map(Number).filter((value) => Number.isSafeInteger(value) && value > 0))
  } catch {
    return new Set()
  }
}

function persistReadIds(): void {
  try {
    const values = [...readIds.value].slice(-500)
    globalThis.localStorage?.setItem(READ_IDS_STORAGE_KEY, JSON.stringify(values))
  } catch {
    // Local read state is optional and must never block announcement access.
  }
}

onMounted(() => { void loadMessages() })
</script>

<template>
  <main
    class="page page--plain pencil-page message-center-page"
    data-message-workspace="live"
    data-pencil-source="FkZ6j bRz9K"
  >
    <header class="message-root-header">
      <button
        class="message-header-back"
        type="button"
        :aria-label="t('common.back')"
        @click="goBack"
      >
        <ArrowLeft :size="22" />
      </button>
      <h1>{{ t('messageCenter.title') }}</h1>
      <button
        class="message-read-all"
        type="button"
        :aria-label="t('messageCenter.markAllRead')"
        :disabled="loading || unreadCount === 0"
        @click="markAllRead"
      >
        {{ t('messageCenter.markAllReadShort') }}
      </button>
    </header>

    <nav class="message-filter-bar" role="group" :aria-label="t('messageCenter.categoryLabel')">
      <button
        v-for="category in MESSAGE_CATEGORIES"
        :key="category.value"
        type="button"
        :class="{ active: activeCategory === category.value }"
        :aria-pressed="activeCategory === category.value"
        @click="activeCategory = category.value"
      >
        {{ t(category.labelKey) }}
      </button>
    </nav>

    <section
      class="message-list"
      data-message-source="live"
      :aria-busy="loading"
      aria-live="polite"
    >
      <div v-if="loading && !messages.length" class="message-state" role="status">
        <span class="message-icon"><RefreshCw :size="17" class="spin" /></span>
        <span><strong>{{ t('news.loading') }}</strong><small>{{ t('messageCenter.summarySource') }}</small></span>
      </div>

      <div v-else-if="error && !messages.length" class="message-state message-state--error" role="alert">
        <span class="message-icon"><CircleAlert :size="17" /></span>
        <span><strong>{{ error }}</strong><small>{{ t('messageCenter.announcementEmptyDescription') }}</small></span>
        <button type="button" :disabled="loading" @click="loadMessages">{{ t('messageCenter.retry') }}</button>
      </div>

      <button
        v-for="message in visibleMessages"
        v-else
        :key="message.id"
        class="message-row"
        :class="{ 'message-row--unread': !readIds.has(message.id) }"
        type="button"
        @click="openMessage(message)"
      >
        <span class="message-icon"><Megaphone :size="17" /></span>
        <span class="message-row-copy">
          <span class="message-row-title">
            <strong>{{ message.title }}</strong>
            <i v-if="!readIds.has(message.id)" aria-hidden="true" />
          </span>
          <small>{{ message.category || t('messageCenter.categoryPlatform') }}</small>
        </span>
        <time class="numeric">
          {{ message.publishedAt ? formatDateTime(message.publishedAt) : t('messageCenter.latest') }}
        </time>
      </button>

      <div v-if="!loading && !error && !visibleMessages.length" class="message-state" role="status">
        <span class="message-icon"><Megaphone :size="17" /></span>
        <span><strong>{{ emptyTitle }}</strong><small>{{ emptyDescription }}</small></span>
      </div>

      <div v-if="error && messages.length" class="message-inline-error" role="alert">
        <CircleAlert :size="16" />
        <span>{{ error }}</span>
        <button type="button" :aria-label="t('messageCenter.retry')" :disabled="loading" @click="loadMessages">
          <RefreshCw :size="16" :class="{ spin: loading }" />
        </button>
      </div>
    </section>
  </main>
</template>

<style scoped>
.page.pencil-page.message-center-page {
  background: var(--page);
  background-image: none;
  min-height: 100dvh;
  padding-bottom: calc(20px + env(safe-area-inset-bottom));
}

.message-root-header {
  align-items: center;
  background: var(--surface);
  box-sizing: border-box;
  display: grid;
  grid-template-columns: 40px minmax(0, 1fr) 49px;
  height: 56px;
  min-height: 56px;
  padding: 12px 20px 4px;
  position: sticky;
  top: env(safe-area-inset-top);
  z-index: var(--layer-sticky-header);
}

.message-root-header h1 {
  color: var(--ink);
  font-size: 22px;
  font-weight: 750;
  justify-self: center;
  line-height: 32px;
  margin: 0;
  overflow: hidden;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.message-header-back,
.message-read-all {
  background: transparent;
  border: 0;
  height: 40px;
  min-height: 40px;
  padding: 0;
  position: relative;
}

.message-header-back {
  color: var(--ink);
  display: grid;
  justify-self: start;
  place-items: center;
  width: 40px;
}

.message-header-back::before,
.message-read-all::before {
  content: '';
  inset: -2px;
  position: absolute;
}

.message-read-all {
  color: var(--positive);
  font-size: 12px;
  font-weight: 600;
  justify-self: end;
  line-height: 17px;
  white-space: nowrap;
  width: 49px;
}

.message-read-all:disabled {
  color: var(--muted);
  opacity: 1;
}

.message-filter-bar {
  align-items: flex-start;
  box-sizing: border-box;
  display: flex;
  gap: 20px;
  height: 38px;
  min-height: 38px;
  min-width: 0;
  overflow: visible;
  padding: 8px 20px 4px;
}

.message-filter-bar button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 0;
  color: var(--muted);
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  font-size: 13px;
  font-weight: 500;
  gap: 5px;
  height: 26px;
  line-height: 19px;
  min-height: 26px;
  padding: 0;
  position: relative;
}

.message-filter-bar button::before {
  content: '';
  inset: -8px -4px;
  position: absolute;
}

.message-filter-bar button::after {
  background: transparent;
  border-radius: 1px;
  content: '';
  height: 2px;
  width: 18px;
}

.message-filter-bar button.active {
  color: var(--ink);
  font-weight: 650;
}

.message-filter-bar button.active::after {
  background: var(--accent);
}

.message-list {
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 6px 20px 0;
}

.message-row,
.message-state {
  align-items: center;
  background: transparent;
  color: var(--ink);
  display: grid;
  gap: 12px;
  grid-template-columns: 40px minmax(0, 1fr) auto;
  min-width: 0;
  padding: 12px 0;
  text-align: left;
  width: 100%;
}

.message-row {
  border: 0;
  height: 64px;
  min-height: 64px;
}

.message-state {
  min-height: 64px;
}

.message-icon {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--ink);
  display: inline-flex;
  height: 40px;
  justify-content: center;
  width: 40px;
}

.message-row-copy,
.message-state > span:nth-child(2) {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.message-row-title {
  align-items: center;
  display: flex;
  gap: 6px;
  min-width: 0;
}

.message-row-title strong,
.message-state strong {
  color: var(--ink);
  font-size: 13px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.message-row-title i {
  background: var(--negative);
  border-radius: 50%;
  flex: 0 0 auto;
  height: 6px;
  width: 6px;
}

.message-row-copy small,
.message-state small {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.message-row time {
  color: var(--muted-strong);
  font-size: 10px;
  font-weight: 600;
  max-width: 78px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.message-row--unread .message-icon {
  border-color: var(--line-strong);
}

.message-state--error .message-icon,
.message-state--error strong {
  color: var(--negative);
}

.message-state > button {
  background: transparent;
  color: var(--positive);
  font-size: 11px;
  font-weight: 600;
  min-height: 44px;
  padding: 0;
}

.message-inline-error {
  align-items: center;
  background: var(--negative-soft);
  border-radius: 8px;
  color: var(--negative);
  display: grid;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 18px minmax(0, 1fr) 44px;
  min-height: 44px;
  padding: 0 0 0 10px;
}

.message-inline-error button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  place-items: center;
}

.message-header-back:focus-visible,
.message-read-all:focus-visible,
.message-filter-bar button:focus-visible,
.message-row:focus-visible,
.message-state button:focus-visible,
.message-inline-error button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 340px) {
  .message-root-header,
  .message-filter-bar,
  .message-list {
    padding-inline: 16px;
  }

  .message-filter-bar { gap: 16px; }

  .message-row,
  .message-state {
    gap: 10px;
    grid-template-columns: 40px minmax(0, 1fr) auto;
  }

  .message-row time { max-width: 62px; }
}

@media (prefers-reduced-motion: reduce) {
  .message-center-page *,
  .message-center-page *::before,
  .message-center-page *::after {
    animation: none !important;
    scroll-behavior: auto !important;
    transition: none !important;
  }
}
</style>
