<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleAlert, LoaderCircle, Newspaper } from 'lucide-vue-next'
import NewsRichText from '@/components/NewsRichText.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNewsDetail, type NewsDetail } from '@/api/news'
import { formatDateTime } from '@/core/format'

const props = defineProps<{ id: string }>()
const { t } = useI18n()
const detail = ref<NewsDetail | null>(null)
const loading = ref(false)
const error = ref('')
let requestVersion = 0

async function load(): Promise<void> {
  const version = ++requestVersion
  const id = Number(props.id)
  loading.value = true
  error.value = ''
  detail.value = null
  if (!Number.isSafeInteger(id) || id <= 0) {
    error.value = t('news.detailLoadFailed')
    loading.value = false
    return
  }

  try {
    const result = await fetchNewsDetail(id)
    if (version === requestVersion) detail.value = result
  } catch (reason) {
    if (version === requestVersion) error.value = apiErrorMessage(reason, t('news.detailLoadFailed'))
  } finally {
    if (version === requestVersion) loading.value = false
  }
}

watch(() => props.id, () => {
  void load()
}, { immediate: true })
</script>

<template>
  <main class="page page--plain news-detail-page">
    <PageHeader
      :back="true"
      :eyebrow="t('messageCenter.categoryAnnouncement')"
      :subtitle="t('messageCenter.categoryPlatform')"
      :title="t('news.detailTitle')"
    />

    <article class="page-content news-article" :aria-busy="loading">
      <div v-if="loading" class="news-detail-state" role="status">
        <LoaderCircle :size="22" class="spin" />
        <span>{{ t('news.detailLoading') }}</span>
      </div>

      <div v-else-if="error" class="news-detail-state news-detail-state--error" role="alert">
        <CircleAlert :size="22" />
        <span>{{ error }}</span>
        <button class="button button--secondary" type="button" @click="load">
          {{ t('common.retry') }}
        </button>
      </div>

      <template v-else-if="detail">
        <img
          v-if="detail.bannerUrl"
          :src="detail.bannerUrl"
          :alt="detail.title"
          class="news-banner"
          decoding="async"
        />
        <header class="news-article__header">
          <span class="news-category">
            <Newspaper :size="15" />
            {{ detail.category }}
          </span>
          <h1>{{ detail.title }}</h1>
          <time v-if="detail.publishedAt">{{ formatDateTime(detail.publishedAt) }}</time>
        </header>
        <NewsRichText :blocks="detail.content" :empty-text="t('news.emptyContent')" />
      </template>

      <div v-else class="news-detail-state">
        <Newspaper :size="23" />
        <span>{{ t('news.empty') }}</span>
      </div>
    </article>
  </main>
</template>

<style scoped>
.news-detail-page {
  background: var(--surface);
}

.news-article {
  min-width: 0;
  padding-bottom: calc(42px + env(safe-area-inset-bottom));
  padding-top: 16px;
}

.news-banner {
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  display: block;
  margin-bottom: 18px;
  max-height: 240px;
  min-height: 150px;
  object-fit: cover;
  width: 100%;
}

.news-article__header {
  border-bottom: 1px solid var(--line);
  padding-bottom: 18px;
}

.news-category {
  align-items: center;
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 34%, var(--line));
  color: var(--accent);
  display: inline-flex;
  font-size: 11px;
  font-weight: 750;
  gap: 6px;
  min-height: 32px;
  padding: 0 9px;
}

.news-detail-page h1 {
  color: var(--ink);
  font-size: 26px;
  letter-spacing: 0;
  line-height: 1.3;
  margin: 14px 0 10px;
  overflow-wrap: anywhere;
}

.news-detail-page time {
  color: var(--muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}

.news-detail-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 300px;
  padding: 32px 16px;
  text-align: center;
}

.news-detail-state--error {
  background: var(--negative-soft);
  border: 1px solid color-mix(in srgb, var(--negative) 28%, var(--line));
  color: var(--negative);
  min-height: 220px;
}

.news-detail-state .button {
  min-height: 44px;
  min-width: 132px;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .news-article {
    padding-left: 14px;
    padding-right: 14px;
  }

  .news-detail-page h1 {
    font-size: 22px;
  }

}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
