<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { BookOpen, ChevronRight, CircleAlert, LoaderCircle, Newspaper, Share2 } from 'lucide-vue-next'
import NewsRichText from '@/components/NewsRichText.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNews, fetchNewsDetail, type NewsDetail, type NewsItem } from '@/api/news'
import { formatDateTime } from '@/core/format'

const props = defineProps<{ id: string }>()
const router = useRouter()
const { t } = useI18n()
const detail = ref<NewsDetail | null>(null)
const loading = ref(false)
const error = ref('')
const related = ref<NewsItem[]>([])
const shareFeedback = ref('')
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
    const [result, nextRelated] = await Promise.all([
      fetchNewsDetail(id),
      fetchNews(6).catch(() => []),
    ])
    if (version === requestVersion) {
      detail.value = result
      related.value = nextRelated.filter((item) => item.id !== id).slice(0, 3)
    }
  } catch (reason) {
    if (version === requestVersion) error.value = apiErrorMessage(reason, t('news.detailLoadFailed'))
  } finally {
    if (version === requestVersion) loading.value = false
  }
}

async function shareArticle(): Promise<void> {
  if (!detail.value) return
  const data = { title: detail.value.title, url: window.location.href }
  try {
    if (navigator.share) await navigator.share(data)
    else await navigator.clipboard.writeText(data.url)
    shareFeedback.value = t('news.shared')
  } catch {
    shareFeedback.value = ''
  }
}

watch(() => props.id, () => {
  void load()
}, { immediate: true })
</script>

<template>
  <main class="page page--plain pencil-page news-detail-pencil" data-pencil-source="Q50Rgr ASvmq">
    <PageHeader class="news-detail-pencil__header" :back="true" compact :pencil="true" :title="t('news.detailTitle')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('news.share')" :disabled="!detail" @click="shareArticle">
          <Share2 :size="18" />
        </button>
      </template>
    </PageHeader>

    <article class="pencil-content news-article-pencil" :aria-busy="loading">
      <div v-if="loading" class="pencil-state" role="status">
        <LoaderCircle :size="22" class="spin" /><span>{{ t('news.detailLoading') }}</span>
      </div>
      <div v-else-if="error" class="pencil-state news-detail-state--error" role="alert">
        <CircleAlert :size="22" /><span>{{ error }}</span>
        <button class="pencil-secondary" type="button" @click="load">{{ t('common.retry') }}</button>
      </div>

      <template v-else-if="detail">
        <div class="news-article-pencil__body">
          <header class="news-article-pencil__copy">
            <span class="news-article-pencil__kicker">{{ detail.category }}</span>
            <h1>{{ detail.title }}</h1>
            <time v-if="detail.publishedAt">{{ formatDateTime(detail.publishedAt) }}</time>
            <span v-if="shareFeedback" class="news-share-feedback" role="status">{{ shareFeedback }}</span>
          </header>

          <div
            v-if="detail.bannerUrl"
            class="news-detail-visual"
            :style="{ backgroundImage: `url(${detail.bannerUrl})` }"
            role="img"
            :aria-label="detail.title"
          />
          <div v-else class="news-detail-visual news-detail-visual--empty" aria-hidden="true" />

          <NewsRichText :blocks="detail.content" :empty-text="t('news.emptyContent')" />
        </div>

        <section v-if="related.length" class="news-related">
          <button
            v-for="item in related.slice(0, 1)"
            :key="item.id"
            class="news-related__link"
            type="button"
            :aria-label="`${t('news.related')}: ${item.title}`"
            @click="router.push({ name: 'news-detail', params: { id: item.id } })"
          >
            <BookOpen :size="16" aria-hidden="true" />
            <strong>{{ t('news.related') }}: {{ item.title }}</strong>
            <ChevronRight :size="16" aria-hidden="true" />
          </button>
        </section>
      </template>

      <div v-else class="pencil-state"><Newspaper :size="23" /><span>{{ t('news.empty') }}</span></div>
    </article>
  </main>
</template>

<style scoped>
.news-detail-pencil {
  background: var(--page);
  color: var(--ink);
}

.news-detail-pencil :deep(.news-detail-pencil__header.pencil-page-header) {
  --pencil-header-inline: 14px;
  --pencil-header-inline-compact: 14px;

  height: 58px;
  min-height: 58px;
  padding: 7px var(--pencil-header-inline);
}

.news-detail-pencil :deep(.news-detail-pencil__header .page-header__back svg) {
  height: 22px;
  width: 22px;
}

.news-article-pencil {
  padding-top: 0;
}

.news-article-pencil__body {
  box-sizing: border-box;
  min-height: 528px;
  padding: 8px 0 16px;
}

.news-article-pencil__copy {
  display: grid;
  gap: 14px;
  position: relative;
}

.news-article-pencil__kicker {
  color: var(--positive);
  font-size: 11px;
  font-weight: 600;
  line-height: 14px;
}

.news-article-pencil h1 {
  color: var(--ink);
  font-size: 24px;
  font-weight: 750;
  letter-spacing: 0;
  line-height: 1.25;
  margin: 0;
  overflow-wrap: anywhere;
}

.news-article-pencil__copy > time {
  color: var(--muted);
  font-family: var(--font-geist-sans), sans-serif;
  font-size: 11px;
  font-weight: 450;
  line-height: 14px;
}

.news-share-feedback {
  color: var(--positive);
  font-size: 10px;
  position: absolute;
  right: 0;
  top: 0;
}

.news-detail-visual {
  background-color: var(--surface-2);
  background-position: center;
  background-size: cover;
  border-radius: 12px;
  height: 160px;
  margin-top: 23px;
  overflow: hidden;
  width: 100%;
}

.news-detail-visual--empty {
  background: linear-gradient(
    45deg,
    var(--accent),
    color-mix(in srgb, var(--accent) 36%, var(--on-accent)) 50%,
    var(--on-accent)
  );
}

.news-article-pencil :deep(.news-rich-text) {
  color: var(--muted);
  font-size: 13px;
  gap: 13px;
  line-height: 1.55;
  margin-top: 14px;
}

.news-article-pencil :deep(.news-rich-text > p:first-child) {
  color: var(--ink);
  font-size: 14px;
  font-weight: 500;
}

.news-article-pencil :deep(.news-rich-text :is(h1, h2, h3)) {
  color: var(--ink);
  font-size: 14px;
  font-weight: 700;
  line-height: 1.45;
}

.news-article-pencil :deep(.news-rich-text__image img) {
  border: 0;
  border-radius: 12px;
}

.news-related {
  min-height: 45px;
}

.news-related__link {
  align-items: center;
  background: transparent;
  border: 0;
  box-sizing: border-box;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 16px minmax(0, 1fr) 16px;
  height: 45px;
  min-height: 45px;
  padding: 8px 0 20px;
  text-align: left;
  width: 100%;
}

.news-related__link > svg:first-child {
  color: var(--positive);
}

.news-related__link > svg:last-child {
  color: var(--muted);
}

.news-related__link strong {
  font-size: 12px;
  font-weight: 600;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.news-related__link:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.news-article-pencil > .pencil-state {
  min-height: 528px;
}

.news-detail-state--error {
  color: var(--negative);
}

.spin {
  animation: news-detail-spin .8s linear infinite;
}

@keyframes news-detail-spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .news-article-pencil h1 {
    font-size: 22px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
