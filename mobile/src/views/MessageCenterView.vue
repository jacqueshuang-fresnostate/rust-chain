<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Bell, CheckCheck, ChevronRight, Megaphone, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNews, type NewsItem } from '@/api/news'
import { formatDateTime } from '@/core/format'

type MessageFilter = 'all' | 'unread'

const READ_IDS_STORAGE_KEY = 'hippo_mobile_message_read_ids'
const router = useRouter()
const { t } = useI18n()
const messages = ref<NewsItem[]>([])
const readIds = ref(readStoredIds())
const activeFilter = ref<MessageFilter>('all')
const loading = ref(false)
const error = ref('')

const unreadCount = computed(() => messages.value.reduce((count, message) => count + (readIds.value.has(message.id) ? 0 : 1), 0))
const visibleMessages = computed(() => activeFilter.value === 'unread'
  ? messages.value.filter((message) => !readIds.value.has(message.id))
  : messages.value)

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
  <main class="page page--plain page--prototype-grid message-center-page" data-message-workspace="live">
    <PageHeader
      :back="true"
      :eyebrow="t('messageCenter.categoryPlatform')"
      :subtitle="t('messageCenter.summaryUnread', { unread: unreadCount })"
      :title="t('messageCenter.title')"
    >
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('messageCenter.retry')" :disabled="loading" @click="loadMessages">
          <RefreshCw :size="21" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>

    <div class="page-content message-center-content">
      <section class="message-summary">
        <div class="message-summary__signal">
          <span><Bell :size="20" /></span>
          <div>
            <strong>{{ t('messageCenter.summaryTotal', { total: messages.length }) }}</strong>
            <small>{{ t('messageCenter.summaryUnread', { unread: unreadCount }) }}</small>
          </div>
        </div>
        <dl class="message-summary__metrics">
          <div>
            <dt>{{ t('messageCenter.categoryPlatform') }}</dt>
            <dd class="numeric">{{ messages.length }}</dd>
          </div>
          <div>
            <dt>{{ t('messageCenter.filterUnread') }}</dt>
            <dd class="numeric">{{ unreadCount }}</dd>
          </div>
        </dl>
        <button class="mark-all-button" type="button" :disabled="unreadCount === 0" @click="markAllRead">
          <CheckCheck :size="18" />
          <span>{{ unreadCount === 0 ? t('messageCenter.allRead') : t('messageCenter.markAllRead') }}</span>
        </button>
      </section>

      <div class="message-filters" :aria-label="t('messageCenter.title')">
        <button type="button" :aria-pressed="activeFilter === 'all'" @click="activeFilter = 'all'">{{ t('messageCenter.filterAll') }}</button>
        <button type="button" :aria-pressed="activeFilter === 'unread'" @click="activeFilter = 'unread'">{{ t('messageCenter.filterUnread') }}</button>
      </div>

      <p v-if="error" class="error-message message-error" role="alert">{{ error }}</p>
      <button v-if="error" class="button button--secondary button--full" type="button" :disabled="loading" @click="loadMessages">{{ t('messageCenter.retry') }}</button>
      <p v-if="loading && !messages.length" class="empty-state">{{ t('news.loading') }}</p>

      <div v-else-if="visibleMessages.length" class="message-list">
        <button
          v-for="message in visibleMessages"
          :key="message.id"
          class="message-row"
          :class="{ 'message-row--unread': !readIds.has(message.id) }"
          type="button"
          @click="openMessage(message)"
        >
          <span class="message-row__icon"><Megaphone :size="19" /></span>
          <span class="message-row__body">
            <span class="message-row__meta">
              <b>{{ t('messageCenter.categoryPlatform') }}</b>
              <small>{{ t('messageCenter.categoryAnnouncement') }}</small>
            </span>
            <strong>{{ message.title }}</strong>
            <time class="numeric">{{ message.publishedAt ? formatDateTime(message.publishedAt) : t('messageCenter.latest') }}</time>
          </span>
          <span v-if="!readIds.has(message.id)" class="message-row__unread" aria-hidden="true" />
          <ChevronRight :size="18" aria-hidden="true" />
        </button>
      </div>
      <p v-else-if="!loading" class="empty-state">{{ t('messageCenter.empty') }}</p>
    </div>
  </main>
</template>

<style scoped>
.message-center-content { padding-bottom: calc(40px + env(safe-area-inset-bottom)); }
.message-summary {
  align-items: center;
  background:
    linear-gradient(var(--grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid-line) 1px, transparent 1px),
    var(--signal-coral);
  background-size: 32px 32px;
  border-bottom: 1px solid var(--line-strong);
  color: var(--on-accent);
  display: grid;
  gap: 14px;
  grid-template-columns: minmax(0, 1fr) auto;
  margin: 0 -16px;
  min-height: 190px;
  padding: 22px 16px 0;
  position: relative;
}
.message-summary::before { background: var(--dark-surface); content: ''; height: 3px; left: 16px; position: absolute; top: 0; width: 44px; }
.message-summary__signal { align-items: center; display: grid; gap: 11px; grid-template-columns: 42px minmax(0, 1fr); }
.message-summary__signal > span { align-items: center; background: var(--dark-surface); border: 1px solid var(--dark-surface); border-radius: 50%; color: var(--on-dark-surface); display: inline-flex; height: 42px; justify-content: center; width: 42px; }
.message-summary__signal div { display: grid; gap: 4px; min-width: 0; }
.message-summary__signal strong { font-size: 17px; }
.message-summary__signal small { color: color-mix(in srgb, var(--on-accent) 72%, transparent); font-size: 12px; }
.message-summary__metrics { align-self: end; border-top: 1px solid color-mix(in srgb, var(--on-accent) 22%, transparent); display: grid; grid-column: 1 / -1; grid-template-columns: repeat(2, minmax(0, 1fr)); margin: 0 -16px; }
.message-summary__metrics > div { display: grid; gap: 3px; min-height: 62px; padding: 9px 16px; }
.message-summary__metrics > div + div { border-left: 1px solid color-mix(in srgb, var(--on-accent) 22%, transparent); }
.message-summary__metrics > div:first-child { border-top: 3px solid var(--signal-green); }
.message-summary__metrics > div:last-child { border-top: 3px solid var(--signal-blue); }
.message-summary__metrics dt,
.message-summary__metrics dd { margin: 0; }
.message-summary__metrics dt { color: color-mix(in srgb, var(--on-accent) 68%, transparent); font-size: 9px; }
.message-summary__metrics dd { font-size: 17px; font-weight: 800; }
.mark-all-button { align-items: center; background: var(--dark-surface); color: var(--on-dark-surface); display: inline-flex; font-size: 11px; font-weight: 750; gap: 6px; justify-content: center; max-width: 132px; min-height: 44px; padding: 0 10px; text-align: center; }
.message-filters { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin: 16px 0 18px; padding: 3px; }
.message-filters button { background: transparent; border: 1px solid transparent; border-radius: calc(var(--radius) - 3px); color: var(--muted); font-size: 13px; font-weight: 750; min-height: 44px; }
.message-filters button[aria-pressed='true'] { background: var(--surface-elevated); border-color: var(--line); box-shadow: var(--shadow-soft); color: var(--ink); }
.message-error { margin-bottom: 12px; }
.message-list { background: var(--surface); border-top: 1px solid var(--line); display: grid; }
.message-row { align-items: center; background: transparent; border-bottom: 1px solid var(--line); color: var(--ink); display: grid; gap: 11px; grid-template-columns: 40px minmax(0, 1fr) 8px 18px; min-height: 94px; padding: 12px 0; position: relative; text-align: left; width: 100%; }
.message-row::before { background: transparent; bottom: 12px; content: ''; left: -20px; position: absolute; top: 12px; width: 3px; }
.message-row--unread::before { background: var(--accent); }
.message-row__icon { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--muted-strong); display: inline-flex; height: 40px; justify-content: center; width: 40px; }
.message-row--unread .message-row__icon { background: var(--accent-soft); border-color: color-mix(in srgb, var(--accent) 24%, var(--line)); color: var(--accent); }
.message-row__body { display: grid; gap: 5px; min-width: 0; }
.message-row__meta { align-items: center; display: flex; gap: 7px; }
.message-row__meta b { color: var(--accent); font-size: 10px; letter-spacing: 0; text-transform: uppercase; }
.message-row__meta small { color: var(--muted); font-size: 10px; }
.message-row__body > strong { display: -webkit-box; font-size: 14px; line-height: 1.4; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.message-row--unread .message-row__body > strong { font-weight: 780; }
.message-row time { color: var(--muted); font-size: 11px; }
.message-row__unread { background: var(--accent); border-radius: 50%; height: 7px; width: 7px; }
.message-row > svg { color: var(--muted); }
.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 360px) {
  .message-summary { margin-left: -12px; margin-right: -12px; }
}
@media (max-width: 340px) {
  .message-center-content { padding-left: 16px; padding-right: 16px; }
  .message-summary { margin-left: -16px; margin-right: -16px; }
  .mark-all-button { max-width: 112px; }
  .message-row { gap: 8px; grid-template-columns: 36px minmax(0, 1fr) 7px 16px; }
  .message-row::before { left: -16px; }
  .message-row__icon { height: 36px; width: 36px; }
}
</style>
