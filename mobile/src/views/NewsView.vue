<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ChevronRight, RefreshCw } from 'lucide-vue-next'
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
    <PageHeader :title="t('news.title')"><template #actions><button class="icon-button" type="button" :aria-label="t('news.refresh')" :disabled="loading" @click="load"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template></PageHeader>
    <div class="page-content"><p v-if="error" class="error-message">{{ error }}</p><p v-if="loading" class="empty-state">{{ t('news.loading') }}</p><div v-else class="news-list"><button v-for="notice in rows" :key="notice.id" type="button" @click="router.push({ name: 'news-detail', params: { id: notice.id } })"><span><strong>{{ notice.title }}</strong><small>{{ formatDateTime(notice.publishedAt) }}</small></span><ChevronRight :size="19" /></button></div><p v-if="!loading && !rows.length" class="empty-state">{{ t('news.empty') }}</p></div>
  </main>
</template>

<style scoped>
.news-page .page-content { padding-bottom: 42px; }.news-list { display: grid; }.news-list button { align-items: center; background: transparent; border-bottom: 1px solid var(--line); display: flex; gap: 12px; justify-content: space-between; min-height: 76px; padding: 12px 0; text-align: left; width: 100%; }.news-list span { display: grid; gap: 6px; min-width: 0; }.news-list strong { font-size: 15px; line-height: 1.4; }.news-list small { color: var(--muted); font-size: 12px; }.news-list svg { color: var(--muted); flex: 0 0 auto; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
