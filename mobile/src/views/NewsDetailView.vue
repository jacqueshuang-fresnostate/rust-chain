<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchNewsDetail, type NewsDetail } from '@/api/news'
import { formatDateTime } from '@/core/format'

const props = defineProps<{ id: string }>()
const { t } = useI18n()
const detail = ref<NewsDetail | null>(null)
const loading = ref(false)
const error = ref('')

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try { detail.value = await fetchNewsDetail(Number(props.id)) } catch (reason) { error.value = apiErrorMessage(reason, t('news.detailLoadFailed')) } finally { loading.value = false }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain news-detail-page"><PageHeader :title="t('news.detailTitle')" /><article class="page-content"><p v-if="error" class="error-message">{{ error }}</p><p v-if="loading" class="empty-state">{{ t('news.detailLoading') }}</p><template v-else-if="detail"><img v-if="detail.bannerUrl" :src="detail.bannerUrl" :alt="detail.title" class="news-banner" /><span class="news-category">{{ detail.category }}</span><h1>{{ detail.title }}</h1><time>{{ formatDateTime(detail.publishedAt) }}</time><div class="news-content">{{ detail.content || t('news.emptyContent') }}</div></template></article></main>
</template>

<style scoped>
.news-detail-page article { padding-bottom: 42px; padding-top: 16px; }.news-banner { border-radius: var(--radius); display: block; margin-bottom: 17px; max-height: 230px; object-fit: cover; width: 100%; }.news-category { color: var(--accent); font-size: 13px; font-weight: 700; }.news-detail-page h1 { font-size: 25px; line-height: 1.35; margin: 10px 0 8px; }.news-detail-page time { color: var(--muted); font-size: 12px; }.news-content { color: var(--muted-strong); font-size: 15px; line-height: 1.85; margin-top: 25px; white-space: pre-wrap; }
</style>
