<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ArrowUpRight, CircleAlert, LoaderCircle, Newspaper, RefreshCw } from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNews, type NewsItem } from '@/api/news'
import { formatDateTime } from '@/core/format'

const router = useRouter()
const { t } = useI18n()
const rows = ref<NewsItem[]>([])
const loading = ref(false)
const error = ref('')

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try { rows.value = await fetchNews(50) } catch (reason) { error.value = apiErrorMessage(reason, t('news.loadFailed')) } finally { loading.value = false }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain news-page">
    <PageHeader
      :back="true"
      :eyebrow="t('messageCenter.categoryAnnouncement')"
      :subtitle="t('messageCenter.categoryPlatform')"
      :title="t('news.title')"
    >
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="t('news.refresh')"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw :size="20" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>

    <div class="page-content news-page__content" :aria-busy="loading">
      <section v-if="!loading && !error && rows.length" class="news-summary">
        <span class="news-summary__icon"><Newspaper :size="20" /></span>
        <span>{{ t('common.liveData') }}</span>
        <strong class="numeric">{{ rows.length }}</strong>
      </section>

      <div v-if="loading" class="news-state" role="status">
        <LoaderCircle :size="22" class="spin" />
        <span>{{ t('news.loading') }}</span>
      </div>

      <div v-else-if="error" class="news-state news-state--error" role="alert">
        <CircleAlert :size="22" />
        <span>{{ error }}</span>
        <button class="button button--secondary" type="button" @click="load">
          {{ t('common.retry') }}
        </button>
      </div>

      <div v-else-if="rows.length" class="news-list">
        <button
          v-for="notice in rows"
          :key="notice.id"
          type="button"
          :aria-label="notice.title"
          @click="router.push({ name: 'news-detail', params: { id: notice.id } })"
        >
          <span class="news-list__signal"><Newspaper :size="18" /></span>
          <span class="news-list__copy">
            <strong>{{ notice.title }}</strong>
            <small>
              <span>{{ t('news.title') }}</span>
              <time v-if="notice.publishedAt">{{ formatDateTime(notice.publishedAt) }}</time>
            </small>
          </span>
          <ArrowUpRight :size="18" />
        </button>
      </div>

      <div v-else class="news-state">
        <Newspaper :size="23" />
        <span>{{ t('news.empty') }}</span>
      </div>
    </div>
  </main>
</template>

<style scoped>
.news-page {
  background: var(--surface);
}

.news-page__content {
  min-width: 0;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
}

.news-summary {
  align-items: center;
  background:
    linear-gradient(100deg, color-mix(in srgb, var(--accent) 13%, transparent), transparent 64%),
    var(--surface-elevated);
  border-bottom: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  color: var(--muted-strong);
  display: grid;
  font-size: 11px;
  gap: 9px;
  grid-template-columns: 36px 1fr auto;
  margin: 0 -20px;
  min-height: 62px;
  padding: 8px 20px;
}

.news-summary__icon {
  align-items: center;
  background: var(--accent-soft);
  color: var(--accent);
  display: inline-flex;
  height: 36px;
  justify-content: center;
  width: 36px;
}

.news-summary strong {
  color: var(--ink);
  font-size: 20px;
}

.news-list {
  display: grid;
  min-width: 0;
}

.news-list button {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--line);
  color: var(--ink);
  display: grid;
  gap: 11px;
  grid-template-columns: 38px minmax(0, 1fr) 24px;
  min-height: 82px;
  min-width: 0;
  padding: 11px 0;
  text-align: left;
  width: 100%;
}

.news-list button:hover,
.news-list button:focus-visible {
  background: color-mix(in srgb, var(--soft) 72%, transparent);
}

.news-list__signal {
  align-items: center;
  background: var(--soft);
  border: 1px solid var(--line);
  color: var(--accent);
  display: inline-flex;
  height: 38px;
  justify-content: center;
  width: 38px;
}

.news-list__copy {
  display: grid;
  gap: 7px;
  min-width: 0;
}

.news-list strong {
  display: -webkit-box;
  font-size: 14px;
  line-height: 1.45;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.news-list small {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 10px;
  gap: 7px;
  min-width: 0;
}

.news-list small span {
  color: var(--accent);
  font-weight: 750;
}

.news-list time {
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.news-list button > svg {
  color: var(--muted);
  justify-self: end;
}

.news-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 280px;
  padding: 32px 16px;
  text-align: center;
}

.news-state--error {
  background: var(--negative-soft);
  border: 1px solid color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
  margin-top: 16px;
  min-height: 220px;
}

.news-state .button {
  min-height: 44px;
  min-width: 132px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 360px) {
  .news-summary {
    margin-left: -16px;
    margin-right: -16px;
    padding-left: 16px;
    padding-right: 16px;
  }
}

@media (max-width: 340px) {
  .news-page__content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .news-summary {
    margin-left: -14px;
    margin-right: -14px;
    padding-left: 14px;
    padding-right: 14px;
  }

  .news-list button {
    gap: 8px;
    grid-template-columns: 34px minmax(0, 1fr) 20px;
  }

  .news-list__signal {
    height: 34px;
    width: 34px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
