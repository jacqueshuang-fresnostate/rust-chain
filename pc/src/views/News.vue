<template>
  <div class="h-full overflow-y-auto bg-background text-foreground">
    <div v-if="!isDetailMode" class="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-6 lg:px-6">
      <section class="relative overflow-hidden rounded-2xl border border-border bg-foreground text-background">
        <div class="grid gap-6 p-6 md:grid-cols-[minmax(0,1fr)_320px] md:p-8">
          <div class="flex min-h-[220px] flex-col justify-between gap-8">
            <div class="space-y-4">
              <div class="inline-flex items-center gap-2 rounded-full bg-background/10 px-3 py-1 text-xs font-semibold text-background/80">
                <Icon icon="mdi:radar" class="text-base" />
                {{ t('news.hero_badge') }}
              </div>
              <div class="space-y-3">
                <h1 class="max-w-2xl text-4xl font-black tracking-normal md:text-5xl">
                  {{ t('news.title') }}
                </h1>
                <p class="max-w-2xl text-sm leading-6 text-background/70 md:text-base">
                  {{ t('news.subtitle') }}
                </p>
              </div>
            </div>

            <label class="flex w-full max-w-xl items-center gap-3 rounded-xl bg-background px-4 py-3 text-foreground shadow-lg">
              <Icon icon="mdi:magnify" class="text-xl text-muted-foreground" />
              <input
                v-model="searchText"
                class="w-full bg-transparent text-sm font-medium outline-none placeholder:text-muted-foreground"
                :placeholder="t('news.search_placeholder')"
              />
            </label>
          </div>

          <div class="relative hidden items-end justify-center md:flex">
            <div class="absolute inset-x-4 bottom-0 top-8 rounded-t-full bg-background/10"></div>
            <div class="relative flex h-52 w-52 items-center justify-center rounded-full bg-background text-foreground shadow-2xl">
              <Icon icon="mdi:newspaper-variant-multiple-outline" class="text-7xl text-primary" />
              <div class="absolute -right-4 top-8 flex h-16 w-16 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-lg">
                <Icon icon="mdi:chart-line" class="text-3xl" />
              </div>
              <div class="absolute -left-5 bottom-10 flex h-14 w-14 items-center justify-center rounded-2xl bg-muted text-foreground shadow-lg">
                <Icon icon="mdi:flash" class="text-2xl text-amber-500" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <section class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_360px]">
        <main class="min-w-0 rounded-2xl border border-border bg-card">
          <div class="border-b border-border px-4 pt-4 md:px-5">
            <div class="flex items-center gap-6 overflow-x-auto">
              <button
                v-for="tab in mainTabs"
                :key="tab.id"
                type="button"
                class="shrink-0 border-b-2 px-1 pb-3 text-sm font-bold transition-colors"
                :class="activeSection === tab.id ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
                @click="activeSection = tab.id"
              >
                {{ t(tab.labelKey) }}
              </button>
            </div>

            <div class="flex items-center gap-2 overflow-x-auto py-4">
              <button
                v-for="topic in topics"
                :key="topic.id"
                type="button"
                class="shrink-0 rounded-full px-4 py-2 text-xs font-bold transition-colors"
                :class="activeTopic === topic.id ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground hover:text-foreground'"
                @click="activeTopic = topic.id"
              >
                {{ t(topic.labelKey) }}
              </button>
            </div>
          </div>

          <div v-if="loading" class="flex min-h-[460px] items-center justify-center text-primary">
            <Icon icon="mdi:loading" class="text-4xl animate-spin" />
          </div>

          <div v-else-if="errorMessage" class="flex min-h-[460px] items-center justify-center px-6 text-center text-destructive">
            {{ errorMessage }}
          </div>

          <div v-else-if="filteredNews.length === 0" class="flex min-h-[460px] items-center justify-center px-6 text-center text-muted-foreground">
            {{ t('news.empty') }}
          </div>

          <div v-else class="space-y-0">
            <article
              v-if="featuredNews"
              class="grid cursor-pointer gap-5 border-b border-border p-4 transition-colors hover:bg-muted/40 md:grid-cols-[minmax(0,1fr)_300px] md:p-5"
              @click="openNews(featuredNews)"
            >
              <div class="flex min-w-0 flex-col justify-between gap-5">
                <div class="space-y-3">
                  <div class="flex flex-wrap items-center gap-2 text-xs font-semibold text-muted-foreground">
                    <span class="rounded-full bg-primary/10 px-2.5 py-1 text-primary">
                      {{ t(categoryLabelKey(featuredNews.category)) }}
                    </span>
                    <span>{{ featuredNews.time }}</span>
                    <span>{{ featuredNews.source }}</span>
                  </div>
                  <h2 class="line-clamp-2 text-2xl font-black leading-tight md:text-3xl">
                    {{ featuredNews.title }}
                  </h2>
                  <p class="line-clamp-3 text-sm leading-6 text-muted-foreground">
                    {{ featuredNews.summary || t('news.no_summary') }}
                  </p>
                </div>
                <button type="button" class="inline-flex w-fit items-center gap-2 text-sm font-bold text-primary">
                  {{ t('news.read_more') }}
                  <Icon icon="mdi:arrow-right" />
                </button>
              </div>

              <div class="aspect-[4/3] overflow-hidden rounded-xl bg-muted">
                <img
                  v-if="featuredNews.bannerUrl || featuredNews.smallLogoUrl"
                  :src="featuredNews.bannerUrl || featuredNews.smallLogoUrl"
                  :alt="featuredNews.title"
                  class="h-full w-full object-cover"
                />
                <div v-else class="flex h-full w-full items-center justify-center bg-muted">
                  <Icon :icon="getCategoryIcon(featuredNews.category)" class="text-6xl text-muted-foreground/35" />
                </div>
              </div>
            </article>

            <div class="grid gap-0 md:grid-cols-[260px_minmax(0,1fr)]">
              <div class="border-b border-border p-4 md:border-b-0 md:border-r md:p-5">
                <div class="mb-4 flex items-center justify-between">
                  <h3 class="text-sm font-black">{{ t('news.ranking') }}</h3>
                  <Icon icon="mdi:fire" class="text-lg text-primary" />
                </div>
                <div class="space-y-3">
                  <button
                    v-for="(news, index) in rankedNews"
                    :key="news.id"
                    type="button"
                    class="grid w-full grid-cols-[28px_minmax(0,1fr)] gap-3 text-left"
                    @click="openNews(news)"
                  >
                    <span class="flex h-7 w-7 items-center justify-center rounded-lg bg-muted text-xs font-black text-muted-foreground">
                      {{ index + 1 }}
                    </span>
                    <span class="line-clamp-2 text-sm font-bold leading-5 hover:text-primary">
                      {{ news.title }}
                    </span>
                  </button>
                </div>
              </div>

              <div class="divide-y divide-border">
                <article
                  v-for="news in articleList"
                  :key="news.id"
                  class="grid cursor-pointer gap-4 p-4 transition-colors hover:bg-muted/40 sm:grid-cols-[96px_minmax(0,1fr)] md:p-5"
                  @click="openNews(news)"
                >
                  <div class="h-20 w-24 overflow-hidden rounded-lg bg-muted">
                    <img
                      v-if="news.smallLogoUrl || news.bannerUrl"
                      :src="news.smallLogoUrl || news.bannerUrl"
                      :alt="news.title"
                      class="h-full w-full object-cover"
                    />
                    <div v-else class="flex h-full w-full items-center justify-center">
                      <Icon :icon="getCategoryIcon(news.category)" class="text-3xl text-muted-foreground/35" />
                    </div>
                  </div>
                  <div class="min-w-0 space-y-2">
                    <div class="flex flex-wrap items-center gap-2 text-xs font-semibold text-muted-foreground">
                      <span>{{ t(categoryLabelKey(news.category)) }}</span>
                      <span>{{ news.time }}</span>
                    </div>
                    <h3 class="line-clamp-2 text-base font-black leading-6 hover:text-primary">
                      {{ news.title }}
                    </h3>
                    <p class="line-clamp-2 text-sm leading-6 text-muted-foreground">
                      {{ news.summary || t('news.no_summary') }}
                    </p>
                  </div>
                </article>
              </div>
            </div>
          </div>
        </main>

        <aside class="space-y-6">
          <section class="rounded-2xl border border-border bg-card">
            <div class="flex items-center justify-between border-b border-border px-5 py-4">
              <h2 class="text-base font-black">{{ t('news.quick_news') }}</h2>
              <Icon icon="mdi:flash" class="text-xl text-amber-500" />
            </div>
            <div class="divide-y divide-border">
              <button
                v-for="news in flashFeed"
                :key="news.id"
                type="button"
                class="grid w-full grid-cols-[72px_minmax(0,1fr)] gap-3 px-5 py-4 text-left transition-colors hover:bg-muted/40"
                @click="openNews(news)"
              >
                <span class="text-xs font-bold text-muted-foreground">{{ news.time }}</span>
                <span class="line-clamp-3 text-sm font-bold leading-5">{{ news.title }}</span>
              </button>
            </div>
          </section>

          <section class="rounded-2xl border border-border bg-card">
            <div class="flex items-center justify-between border-b border-border px-5 py-4">
              <h2 class="text-base font-black">{{ t('news.hot_news') }}</h2>
              <Icon icon="mdi:trending-up" class="text-xl text-primary" />
            </div>
            <div class="space-y-3 p-5">
              <button
                v-for="(news, index) in hotNews"
                :key="news.id"
                type="button"
                class="grid w-full grid-cols-[28px_minmax(0,1fr)] gap-3 text-left"
                @click="openNews(news)"
              >
                <span
                  class="flex h-7 w-7 items-center justify-center rounded-lg text-xs font-black"
                  :class="index < 3 ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'"
                >
                  {{ index + 1 }}
                </span>
                <span class="line-clamp-2 text-sm font-bold leading-5 hover:text-primary">
                  {{ news.title }}
                </span>
              </button>
            </div>
          </section>
        </aside>
      </section>
    </div>

    <div v-else class="news-detail-shell bg-background">
      <div class="mx-auto w-full max-w-7xl px-4 py-5 lg:px-6 lg:py-8">
        <div class="mb-6 flex flex-wrap items-center justify-between gap-3 border-b border-border pb-4">
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-full border border-border bg-card px-4 py-2 text-sm font-bold text-muted-foreground transition-colors hover:border-primary hover:text-primary"
            @click="backToNews"
          >
            <Icon icon="mdi:arrow-left" class="text-lg" />
            {{ t('news.back_to_news') }}
          </button>

          <div v-if="selectedNews" class="flex min-w-0 items-center gap-2 text-xs font-semibold text-muted-foreground">
            <span class="inline-flex items-center gap-1.5 rounded-full bg-primary/10 px-3 py-1 text-primary">
              <Icon :icon="getCategoryIcon(selectedNews.category)" class="text-base" />
              {{ t(categoryLabelKey(selectedNews.category)) }}
            </span>
            <span class="hidden sm:inline">{{ t('news.detail_badge') }}</span>
          </div>
        </div>

        <div v-if="detailLoading && !selectedNews" class="flex min-h-[520px] items-center justify-center rounded-2xl border border-border bg-card text-primary">
          <Icon icon="mdi:loading" class="text-4xl animate-spin" />
        </div>

        <div v-else-if="detailErrorMessage" class="flex min-h-[520px] items-center justify-center rounded-2xl border border-border bg-card px-6 text-center text-destructive">
          {{ detailErrorMessage }}
        </div>

        <section v-else-if="selectedNews" class="grid gap-8 lg:grid-cols-[minmax(0,860px)_340px] lg:items-start">
          <article class="min-w-0">
            <header class="border-b border-border pb-7">
              <div class="flex flex-wrap items-center gap-2 text-xs font-semibold text-muted-foreground">
                <span class="inline-flex items-center gap-1.5 rounded-full bg-primary/10 px-3 py-1.5 text-primary">
                  <Icon :icon="getCategoryIcon(selectedNews.category)" class="text-base" />
                  {{ t(categoryLabelKey(selectedNews.category)) }}
                </span>
                <span class="inline-flex items-center gap-1.5">
                  <Icon icon="mdi:clock-outline" class="text-base" />
                  {{ selectedNews.time }}
                </span>
                <span class="inline-flex items-center gap-1.5">
                  <Icon icon="mdi:account-edit-outline" class="text-base" />
                  {{ selectedNews.source || t('news.default_source') }}
                </span>
              </div>

              <h1 class="mt-5 max-w-4xl text-3xl font-black leading-tight tracking-normal text-foreground md:text-5xl">
                {{ selectedNews.title }}
              </h1>

              <p v-if="selectedNews.summary" class="mt-5 max-w-3xl border-l-4 border-primary bg-card px-4 py-3 text-base font-medium leading-8 text-muted-foreground md:text-lg">
                {{ selectedNews.summary }}
              </p>
            </header>

            <figure v-if="selectedNews.bannerUrl" class="mt-8 overflow-hidden rounded-2xl border border-border bg-card">
              <img :src="selectedNews.bannerUrl" :alt="selectedNews.title" class="aspect-[16/7] h-full w-full object-cover" />
            </figure>
            <div v-else class="mt-8 flex aspect-[16/7] items-center justify-center rounded-2xl border border-dashed border-border bg-card">
              <Icon :icon="getCategoryIcon(selectedNews.category)" class="text-7xl text-muted-foreground/25" />
            </div>

            <div class="px-0 py-8 md:py-10">
              <div v-if="detailLoading" class="mb-5 inline-flex items-center gap-2 rounded-full bg-primary/10 px-3 py-1.5 text-xs font-bold text-primary">
                <Icon icon="mdi:loading" class="animate-spin" />
                {{ t('common.loading') }}
              </div>
              <div class="news-detail-prose text-foreground" v-html="detailHtml"></div>

              <footer class="mt-10 flex flex-wrap items-center justify-between gap-4 border-t border-border pt-6">
                <span class="inline-flex items-center gap-2 rounded-full bg-muted px-3 py-1.5 text-xs font-bold text-muted-foreground">
                  <Icon :icon="getCategoryIcon(selectedNews.category)" class="text-base" />
                  {{ t(categoryLabelKey(selectedNews.category)) }}
                </span>
                <button type="button" class="inline-flex items-center gap-2 text-sm font-bold text-primary" @click="backToNews">
                  {{ t('news.more_news') }}
                  <Icon icon="mdi:arrow-right" />
                </button>
              </footer>
            </div>
          </article>

          <aside class="space-y-5 lg:sticky lg:top-6">
            <section class="rounded-2xl border border-border bg-card p-5">
              <div class="flex items-center justify-between">
                <h2 class="text-base font-black">{{ t('news.article_info') }}</h2>
                <Icon icon="mdi:file-document-outline" class="text-xl text-primary" />
              </div>
              <dl class="mt-5 space-y-4 text-sm">
                <div class="flex items-start justify-between gap-4">
                  <dt class="inline-flex items-center gap-2 text-muted-foreground">
                    <Icon icon="mdi:account-edit-outline" />
                    {{ t('news.info_source') }}
                  </dt>
                  <dd class="max-w-[170px] text-right font-bold text-foreground">{{ selectedNews.source || t('news.default_source') }}</dd>
                </div>
                <div class="flex items-start justify-between gap-4">
                  <dt class="inline-flex items-center gap-2 text-muted-foreground">
                    <Icon icon="mdi:clock-outline" />
                    {{ t('news.info_time') }}
                  </dt>
                  <dd class="max-w-[170px] text-right font-bold text-foreground">{{ selectedNews.time }}</dd>
                </div>
                <div class="flex items-start justify-between gap-4">
                  <dt class="inline-flex items-center gap-2 text-muted-foreground">
                    <Icon :icon="getCategoryIcon(selectedNews.category)" />
                    {{ t('news.info_category') }}
                  </dt>
                  <dd class="max-w-[170px] text-right font-bold text-foreground">{{ t(categoryLabelKey(selectedNews.category)) }}</dd>
                </div>
              </dl>
            </section>

            <section class="rounded-2xl border border-border bg-card">
              <div class="flex items-center justify-between border-b border-border px-5 py-4">
                <h2 class="text-base font-black">{{ t('news.related_news') }}</h2>
                <Icon icon="mdi:newspaper-variant-outline" class="text-xl text-primary" />
              </div>
              <div v-if="relatedNews.length > 0" class="divide-y divide-border">
                <button
                  v-for="news in relatedNews"
                  :key="news.id"
                  type="button"
                  class="grid w-full grid-cols-[56px_minmax(0,1fr)] gap-3 px-5 py-4 text-left transition-colors hover:bg-muted/40"
                  @click="openNews(news)"
                >
                  <span class="flex h-14 w-14 items-center justify-center overflow-hidden rounded-lg bg-muted">
                    <img
                      v-if="news.smallLogoUrl || news.bannerUrl"
                      :src="news.smallLogoUrl || news.bannerUrl"
                      :alt="news.title"
                      class="h-full w-full object-cover"
                    />
                    <Icon v-else :icon="getCategoryIcon(news.category)" class="text-2xl text-muted-foreground/35" />
                  </span>
                  <span class="min-w-0">
                    <span class="mb-1 flex items-center gap-2 text-xs font-semibold text-muted-foreground">
                      <span>{{ t(categoryLabelKey(news.category)) }}</span>
                      <span>{{ news.time }}</span>
                    </span>
                    <span class="line-clamp-2 text-sm font-bold leading-5 hover:text-primary">{{ news.title }}</span>
                  </span>
                </button>
              </div>
              <div v-else class="px-5 py-6 text-sm text-muted-foreground">
                {{ t('news.no_related') }}
              </div>
            </section>

            <section class="rounded-2xl border border-border bg-card">
              <div class="flex items-center justify-between border-b border-border px-5 py-4">
                <h2 class="text-base font-black">{{ t('news.latest_updates') }}</h2>
                <Icon icon="mdi:trending-up" class="text-xl text-primary" />
              </div>
              <div class="space-y-3 p-5">
                <button
                  v-for="(news, index) in detailHotNews"
                  :key="news.id"
                  type="button"
                  class="grid w-full grid-cols-[28px_minmax(0,1fr)] gap-3 text-left"
                  @click="openNews(news)"
                >
                  <span
                    class="flex h-7 w-7 items-center justify-center rounded-lg text-xs font-black"
                    :class="index < 3 ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'"
                  >
                    {{ index + 1 }}
                  </span>
                  <span class="min-w-0">
                    <span class="line-clamp-2 text-sm font-bold leading-5 hover:text-primary">
                      {{ news.title }}
                    </span>
                    <span class="mt-1 block text-xs font-semibold text-muted-foreground">{{ news.time }}</span>
                  </span>
                </button>
              </div>
            </section>
          </aside>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Icon } from '@iconify/vue'
import { fetchPublicNews, fetchPublicNewsDetail } from '@/api/news'
import type { PcNewsCard } from '@/api/backendAdapters'
import { useSettingStore } from '@/stores/setting'
import { useUserStore } from '@/stores/user'
import { useI18n } from 'vue-i18n'

type NewsSection = 'all' | 'general' | 'market' | 'product' | 'system' | 'promotion'
type NewsTopic = 'all' | 'crypto' | 'stocks' | 'forex' | 'macro'

const mainTabs: Array<{ id: NewsSection; labelKey: string; category?: string }> = [
  { id: 'all', labelKey: 'news.tab_all' },
  { id: 'general', labelKey: 'news.category_general', category: 'general' },
  { id: 'market', labelKey: 'news.category_market', category: 'market' },
  { id: 'product', labelKey: 'news.category_product', category: 'product' },
  { id: 'system', labelKey: 'news.category_system', category: 'system' },
  { id: 'promotion', labelKey: 'news.category_promotion', category: 'promotion' },
]

const topics: Array<{ id: NewsTopic; labelKey: string; keywords: string[] }> = [
  { id: 'all', labelKey: 'news.topic_all', keywords: [] },
  { id: 'crypto', labelKey: 'news.topic_crypto', keywords: ['crypto', 'bitcoin', 'ethereum', 'btc', 'eth', '加密', '比特币', '以太坊', '币'] },
  { id: 'stocks', labelKey: 'news.topic_stocks', keywords: ['stock', 'stocks', 'equity', 'nasdaq', 'spx', '股票', '美股', '纳指'] },
  { id: 'forex', labelKey: 'news.topic_forex', keywords: ['forex', 'gold', 'oil', 'commodity', '外汇', '黄金', '原油', '商品'] },
  { id: 'macro', labelKey: 'news.topic_macro', keywords: ['fed', 'cpi', 'rate', 'macro', 'inflation', '央行', '利率', '通胀', '宏观'] },
]

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const settingStore = useSettingStore()
const userStore = useUserStore()
const activeSection = ref<NewsSection>('all')
const activeTopic = ref<NewsTopic>('all')
const searchText = ref('')
const newsItems = ref<PcNewsCard[]>([])
const selectedNews = ref<PcNewsCard | null>(null)
const loading = ref(false)
const detailLoading = ref(false)
const errorMessage = ref('')
const detailErrorMessage = ref('')

const isDetailMode = computed(() => route.name === 'NewsDetail')
const routeDetailId = computed(() => (isDetailMode.value ? String(route.params.id || '') : ''))
const apiCategory = computed(() => mainTabs.find(tab => tab.id === activeSection.value)?.category)

const filteredNews = computed(() => {
  const keyword = normalizeText(searchText.value)
  const topic = topics.find(item => item.id === activeTopic.value)

  return newsItems.value.filter(news => {
    const source = normalizeText(`${news.title} ${news.summary} ${news.content}`)
    const matchesSearch = !keyword || source.includes(keyword)
    const matchesTopic = !topic || topic.id === 'all' || topic.keywords.some(item => source.includes(normalizeText(item)))
    return matchesSearch && matchesTopic
  })
})

const featuredNews = computed(() => filteredNews.value[0])
const rankedNews = computed(() => filteredNews.value.slice(0, 5))
const articleList = computed(() => filteredNews.value.slice(1))
const flashFeed = computed(() => newsItems.value.slice(0, 6))
const hotNews = computed(() => newsItems.value.slice(0, 6))
const detailHotNews = computed(() => newsItems.value.filter(news => news.id !== selectedNews.value?.id).slice(0, 6))
const relatedNews = computed(() => {
  const current = selectedNews.value
  if (!current) return []
  return newsItems.value
    .filter(news => news.id !== current.id)
    .sort((left, right) => Number(right.category === current.category) - Number(left.category === current.category))
    .slice(0, 5)
})
const detailHtml = computed(() => selectedNews.value?.content || selectedNews.value?.summary || t('news.no_summary'))

async function loadNews() {
  loading.value = true
  errorMessage.value = ''
  try {
    const res = await fetchPublicNews({
      category: apiCategory.value,
      countryCode: userStore.user?.countryCode,
      locale: settingStore.locale,
      limit: 100,
    })
    if (res.data.code === 0) {
      newsItems.value = res.data.data
    } else {
      errorMessage.value = res.data.message || t('news.load_failed')
    }
  } catch (error: any) {
    errorMessage.value = error?.response?.data?.message || t('news.load_failed')
  } finally {
    loading.value = false
  }
}

async function loadNewsDetail(id: number | string) {
  if (!id) return
  detailLoading.value = true
  detailErrorMessage.value = ''
  try {
    const res = await fetchPublicNewsDetail(id, settingStore.locale)
    if (res.data.code === 0 && res.data.data) {
      selectedNews.value = res.data.data
    } else {
      detailErrorMessage.value = res.data.message || t('news.load_failed')
    }
  } catch (error: any) {
    detailErrorMessage.value = error?.response?.data?.message || t('news.load_failed')
  } finally {
    detailLoading.value = false
  }
}

function openNews(news: PcNewsCard) {
  selectedNews.value = news
  router.push({ name: 'NewsDetail', params: { id: String(news.id) } })
}

function backToNews() {
  router.push({ name: 'News' })
}

function normalizeText(value: string) {
  return value.trim().toLowerCase()
}

function categoryLabelKey(category: string) {
  switch (category) {
    case 'market': return 'news.category_market'
    case 'product': return 'news.category_product'
    case 'system': return 'news.category_system'
    case 'promotion': return 'news.category_promotion'
    default: return 'news.category_general'
  }
}

function getCategoryIcon(category: string) {
  switch (category) {
    case 'market': return 'mdi:chart-line'
    case 'product': return 'mdi:cube-outline'
    case 'system': return 'mdi:bullhorn'
    case 'promotion': return 'mdi:gift-outline'
    default: return 'mdi:newspaper'
  }
}

watch(() => [settingStore.locale, userStore.user?.countryCode, activeSection.value], loadNews)
watch(routeDetailId, (id) => {
  if (id) {
    void loadNewsDetail(id)
  } else {
    selectedNews.value = null
    detailErrorMessage.value = ''
  }
}, { immediate: true })
watch(() => settingStore.locale, () => {
  if (routeDetailId.value) {
    void loadNewsDetail(routeDetailId.value)
  }
})

onMounted(loadNews)
</script>

<style scoped>
.news-detail-prose {
  max-width: 780px;
}

.news-detail-prose :deep(*) {
  letter-spacing: 0;
}

.news-detail-prose :deep(p) {
  margin: 0 0 18px;
  color: hsl(var(--muted-foreground));
  font-size: 16px;
  line-height: 1.9;
}

.news-detail-prose :deep(h1),
.news-detail-prose :deep(h2),
.news-detail-prose :deep(h3) {
  margin: 32px 0 14px;
  color: hsl(var(--foreground));
  font-weight: 900;
  line-height: 1.28;
}

.news-detail-prose :deep(h1) {
  font-size: 30px;
}

.news-detail-prose :deep(h2) {
  font-size: 24px;
}

.news-detail-prose :deep(h3) {
  font-size: 20px;
}

.news-detail-prose :deep(strong) {
  color: hsl(var(--foreground));
  font-weight: 800;
}

.news-detail-prose :deep(a) {
  color: hsl(var(--primary));
  font-weight: 700;
  text-decoration: none;
}

.news-detail-prose :deep(a:hover) {
  text-decoration: underline;
}

.news-detail-prose :deep(blockquote) {
  margin: 24px 0;
  border-left: 4px solid hsl(var(--primary));
  background: hsl(var(--card));
  padding: 14px 18px;
  color: hsl(var(--foreground));
  font-weight: 700;
  line-height: 1.8;
}

.news-detail-prose :deep(img) {
  display: block;
  width: 100%;
  max-height: 520px;
  margin: 28px 0;
  border: 1px solid hsl(var(--border));
  border-radius: 16px;
  object-fit: cover;
}

.news-detail-prose :deep(ul),
.news-detail-prose :deep(ol) {
  margin: 18px 0 24px;
  padding-left: 24px;
  color: hsl(var(--muted-foreground));
  line-height: 1.85;
}

.news-detail-prose :deep(li + li) {
  margin-top: 8px;
}

@media (max-width: 768px) {
  .news-detail-prose :deep(p) {
    font-size: 15px;
    line-height: 1.85;
  }

  .news-detail-prose :deep(h1) {
    font-size: 24px;
  }

  .news-detail-prose :deep(h2) {
    font-size: 21px;
  }
}
</style>
