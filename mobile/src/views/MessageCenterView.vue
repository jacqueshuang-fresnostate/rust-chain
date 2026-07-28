<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { CheckCircle2, CheckCheck, ChevronRight, ListFilter, Megaphone, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNews, type NewsItem } from '@/api/news'
import { formatDateTime } from '@/core/format'

type MessageCategory = 'all' | 'account' | 'funds' | 'trade' | 'announcement'

const MESSAGE_CATEGORIES: Array<{ value: MessageCategory; labelKey: string }> = [
  { value: 'all', labelKey: 'messageCenter.filterAll' },
  { value: 'account', labelKey: 'messageCenter.categoryAccount' },
  { value: 'funds', labelKey: 'messageCenter.categoryFunds' },
  { value: 'trade', labelKey: 'messageCenter.categoryTrade' },
  { value: 'announcement', labelKey: 'messageCenter.categoryAnnouncement' },
]

const READ_IDS_STORAGE_KEY = 'hippo_mobile_message_read_ids'
const router = useRouter()
const { t } = useI18n()
const messages = ref<NewsItem[]>([])
const readIds = ref(readStoredIds())
const activeCategory = ref<MessageCategory>('all')
const unreadOnly = ref(false)
const loading = ref(false)
const error = ref('')

const unreadCount = computed(() => messages.value.reduce((count, message) => count + (readIds.value.has(message.id) ? 0 : 1), 0))
const categoryHasNewsSource = computed(() => activeCategory.value === 'all' || activeCategory.value === 'announcement')
const categoryMessages = computed(() => categoryHasNewsSource.value ? messages.value : [])
const visibleMessages = computed(() => unreadOnly.value
  ? categoryMessages.value.filter((message) => !readIds.value.has(message.id))
  : categoryMessages.value)
const messageGroups = computed(() => visibleMessages.value.length
  ? [{ label: t('messageCenter.latest'), messages: visibleMessages.value }]
  : [])
const activeCategoryLabel = computed(() => {
  const category = MESSAGE_CATEGORIES.find((item) => item.value === activeCategory.value)
  return category ? t(category.labelKey) : ''
})
const emptyTitle = computed(() => {
  if (!categoryHasNewsSource.value) {
    return t('messageCenter.categoryEmpty', { category: activeCategoryLabel.value })
  }
  return unreadOnly.value ? t('messageCenter.allRead') : t('messageCenter.empty')
})
const emptyDescription = computed(() => {
  if (!categoryHasNewsSource.value) return t('messageCenter.categoryEmptyDescription')
  if (unreadOnly.value) return t('messageCenter.unreadEmptyDescription')
  return t('messageCenter.announcementEmptyDescription')
})

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
  <main class="secondary-view page page--plain page--prototype-grid message-center-page" data-message-workspace="live">
    <PageHeader
      :back="true"
      :eyebrow="t('messageCenter.scene')"
      :subtitle="t('messageCenter.context')"
      :title="t('messageCenter.title')"
    >
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('messageCenter.retry')" :disabled="loading" @click="loadMessages">
          <RefreshCw :size="21" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>

    <div class="secondary-content page-content message-center-content">
      <section class="message-center" data-message-source="live">
        <div class="inbox-summary" :aria-label="t('messageCenter.title')">
          <div>
            <span>{{ t('messageCenter.summaryTotalLabel') }}</span>
            <strong class="numeric">{{ messages.length }}</strong>
          </div>
          <div>
            <span>{{ t('messageCenter.summaryUnreadLabel') }}</span>
            <strong class="numeric">{{ unreadCount }}</strong>
          </div>
          <p>{{ t('messageCenter.summarySource') }}</p>
        </div>

        <div class="message-filter-bar" role="group" :aria-label="t('messageCenter.categoryLabel')">
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
        </div>

        <div class="message-tools">
          <button
            type="button"
            :aria-pressed="unreadOnly"
            @click="unreadOnly = !unreadOnly"
          >
            <ListFilter :size="16" />
            {{ t('messageCenter.filterUnread') }}
          </button>
          <button type="button" :disabled="unreadCount === 0" @click="markAllRead">
            <CheckCheck :size="16" />
            {{ unreadCount === 0 ? t('messageCenter.allRead') : t('messageCenter.markAllRead') }}
          </button>
        </div>

        <div v-if="unreadCount === 0 && messages.length && !unreadOnly" class="inbox-all-read" role="status">
          <CheckCircle2 :size="18" />
          <span>
            <strong>{{ t('messageCenter.allRead') }}</strong>
            {{ t('messageCenter.summaryTotal', { total: messages.length }) }}
          </span>
        </div>

        <div v-if="error && !messages.length" class="message-empty" role="alert">
          <Megaphone :size="24" />
          <strong>{{ error }}</strong>
          <span>{{ t('messageCenter.empty') }}</span>
          <button type="button" :disabled="loading" @click="loadMessages">
            {{ t('messageCenter.retry') }}
          </button>
        </div>

        <div v-else-if="loading && !messages.length" class="message-empty" aria-live="polite">
          <RefreshCw :size="24" class="spin" />
          <strong>{{ t('news.loading') }}</strong>
          <span>{{ t('messageCenter.summaryUnread', { unread: unreadCount }) }}</span>
        </div>

        <div v-else-if="messageGroups.length" class="message-timeline">
          <section v-for="group in messageGroups" :key="group.label" class="message-time-group">
            <h2>{{ group.label }}</h2>
            <div class="message-list">
              <button
                v-for="message in group.messages"
                :key="message.id"
                class="message-row"
                :class="{ unread: !readIds.has(message.id), 'message-row--unread': !readIds.has(message.id) }"
                type="button"
                @click="openMessage(message)"
              >
                <span class="message-icon"><Megaphone :size="19" /></span>
                <span class="message-row-copy">
                  <span>
                    <b>{{ t('messageCenter.categoryAnnouncement') }}</b>
                    <time class="numeric">
                      {{ message.publishedAt ? formatDateTime(message.publishedAt) : t('messageCenter.latest') }}
                    </time>
                  </span>
                  <strong>{{ message.title }}</strong>
                  <small>{{ t('messageCenter.categoryPlatform') }}</small>
                </span>
                <span class="unread-dot" :class="{ show: !readIds.has(message.id) }" aria-hidden="true" />
                <ChevronRight :size="16" aria-hidden="true" />
              </button>
            </div>
          </section>
        </div>

        <div v-else-if="!loading" class="message-empty" role="status">
          <CheckCircle2 :size="24" />
          <strong>{{ emptyTitle }}</strong>
          <span>{{ emptyDescription }}</span>
          <button v-if="unreadOnly && categoryHasNewsSource" type="button" @click="unreadOnly = false">
            {{ t('messageCenter.filterAll') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.message-center {
  display: grid;
  gap: 16px;
  min-width: 0;
}

.inbox-summary {
  --overview-accent: var(--signal-blue);
  background:
    linear-gradient(132deg, color-mix(in srgb, var(--overview-accent) 8%, transparent), transparent 62%),
    var(--surface);
  border-bottom: 1px solid var(--line-strong);
  border-top: 3px solid var(--overview-accent);
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  min-width: 0;
  overflow: hidden;
}

.inbox-summary > div {
  display: grid;
  gap: 3px;
  min-width: 0;
  padding: 16px 12px;
}

.inbox-summary > div + div { border-left: 1px solid var(--line); }

.inbox-summary span,
.inbox-summary p {
  color: var(--muted);
  font-size: 10px;
}

.inbox-summary strong { font-size: 27px; }

.inbox-summary p {
  border-top: 1px solid var(--line);
  grid-column: 1 / -1;
  line-height: 1.5;
  margin: 0;
  padding: 10px 12px;
}

.message-filter-bar {
  display: grid;
  gap: 3px;
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.message-filter-bar button,
.message-tools button {
  background: transparent;
  border: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
  min-height: 44px;
  min-width: 0;
}

.message-filter-bar button.active {
  background: var(--ink);
  border-color: var(--ink);
  color: var(--surface);
  font-weight: 700;
}

.message-tools {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.message-tools button {
  align-items: center;
  background: var(--soft);
  color: var(--muted-strong);
  display: flex;
  gap: 7px;
  justify-content: center;
  padding: 4px 8px;
}

.message-tools button[aria-pressed='true'] {
  background: color-mix(in srgb, var(--accent) 7%, var(--soft));
  color: var(--accent);
}

.inbox-all-read {
  align-items: center;
  border-block: 1px solid var(--line);
  color: var(--positive);
  display: grid;
  gap: 9px;
  grid-template-columns: 22px minmax(0, 1fr);
  min-width: 0;
  padding: 11px 2px;
}

.inbox-all-read span {
  color: var(--muted);
  display: grid;
  font-size: 10px;
  gap: 3px;
}

.inbox-all-read strong {
  color: var(--ink);
  font-size: 11px;
}

.message-timeline,
.message-time-group,
.message-list { display: grid; }

.message-timeline { gap: 18px; }

.message-time-group h2 {
  border-bottom: 1px solid var(--line-strong);
  color: var(--muted);
  font-size: 10px;
  margin: 0;
  padding: 0 0 8px;
}

.message-list > button {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  border-left: 3px solid transparent;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 42px minmax(0, 1fr) 8px 18px;
  min-height: 88px;
  min-width: 0;
  padding: 12px 2px;
  text-align: left;
}

.message-list > button.unread {
  background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 7%, transparent), transparent 44%);
  border-left-color: var(--accent);
  box-shadow: inset 3px 0 0 color-mix(in srgb, var(--accent) 18%, transparent);
}

.message-icon {
  border: 1px solid var(--line);
  color: var(--accent);
  display: grid;
  height: 40px;
  place-items: center;
  width: 40px;
}

.message-row-copy {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.message-row-copy > span {
  align-items: center;
  display: flex;
  gap: 8px;
  justify-content: space-between;
  min-width: 0;
}

.message-row-copy b,
.message-row-copy time {
  color: var(--muted);
  font-size: 9px;
  font-weight: 650;
}

.message-row-copy strong,
.message-row-copy small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.message-row-copy strong {
  font-size: 12px;
  white-space: nowrap;
}

.message-row-copy small {
  color: var(--muted);
  display: -webkit-box;
  font-size: 10px;
  line-height: 1.45;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.unread-dot {
  background: transparent;
  border-radius: 50%;
  height: 7px;
  width: 7px;
}

.unread-dot.show { background: var(--accent); }

.message-empty {
  align-content: center;
  border-block: 1px solid var(--line-strong);
  color: var(--positive);
  display: grid;
  gap: 9px;
  min-height: 220px;
  padding: 24px;
  place-items: center;
  text-align: center;
}

.message-empty strong {
  color: var(--ink);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.message-empty span {
  color: var(--muted);
  font-size: 11px;
}

.message-empty button {
  background: var(--soft);
  border: 1px solid var(--line);
  color: var(--ink);
  min-height: 44px;
  padding-inline: 16px;
}

.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 340px) {
  .message-list > button {
    gap: 8px;
    grid-template-columns: 36px minmax(0, 1fr) 7px 16px;
  }

  .message-icon {
    height: 36px;
    width: 36px;
  }
}
</style>
