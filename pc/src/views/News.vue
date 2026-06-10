<template>
  <div class="min-h-full flex flex-col items-center relative overflow-y-auto bg-background/50">
    <div class="relative z-10 w-full max-w-7xl px-4 pt-10 pb-8">
      <div class="flex flex-col md:flex-row justify-between items-center mb-8 gap-4">
        <h1 class="text-3xl font-bold flex items-center gap-2">
          <Icon icon="mdi:newspaper-variant-outline" class="text-primary" />
          Crypto News
        </h1>

        <div class="flex bg-muted/50 p-1 rounded-lg">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            @click="activeTab = tab.id"
            class="px-4 py-1.5 rounded-md text-sm font-bold transition-all"
            :class="activeTab === tab.id ? 'bg-background text-primary shadow-sm' : 'text-muted-foreground hover:text-foreground'"
          >
            {{ tab.label }}
          </button>
        </div>
      </div>

      <div v-if="loading" class="py-20 flex justify-center text-primary">
        <span class="i-mdi-loading animate-spin text-4xl"></span>
      </div>

      <div v-else-if="errorMessage" class="py-16 text-center text-destructive">
        {{ errorMessage }}
      </div>

      <div v-else-if="filteredNews.length === 0" class="py-16 text-center text-muted-foreground">
        No published news available.
      </div>

      <div v-else class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        <div v-for="news in filteredNews" :key="news.id" class="bg-card/40 backdrop-blur border border-border rounded-xl overflow-hidden hover:border-primary/50 transition-all group flex flex-col">
          <div class="h-48 bg-muted relative">
            <div class="absolute inset-0 flex items-center justify-center text-muted-foreground bg-gradient-to-br from-muted to-background">
              <Icon :icon="getCategoryIcon(news.category)" class="text-4xl opacity-20" />
            </div>
            <div class="absolute top-2 left-2 px-2 py-1 bg-black/60 backdrop-blur rounded text-xs font-bold text-white uppercase">
              {{ news.category }}
            </div>
          </div>
          <div class="p-4 flex-1 flex flex-col">
            <div class="flex items-center gap-2 text-xs text-muted-foreground mb-2">
              <Icon icon="mdi:clock-outline" />
              <span>{{ news.time }}</span>
              <span>•</span>
              <span>{{ news.source }}</span>
            </div>
            <h3 class="font-bold text-lg mb-2 line-clamp-2 group-hover:text-primary transition-colors">
              {{ news.title }}
            </h3>
            <p class="text-sm text-muted-foreground line-clamp-3 mb-4 flex-1">
              {{ news.summary }}
            </p>
            <div class="flex justify-between items-center mt-auto pt-4 border-t border-border/50">
              <div class="flex gap-2">
                <span class="px-2 py-0.5 bg-muted rounded text-[10px] text-muted-foreground font-medium">#{{ news.category }}</span>
              </div>
              <Icon icon="mdi:arrow-right" class="text-primary" />
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Icon } from '@iconify/vue'
import { fetchPublicNews } from '@/api/news'
import type { PcNewsCard } from '@/api/backendAdapters'

const tabs = [
  { id: 'all', label: 'All' },
  { id: 'flash', label: 'Flash' },
  { id: 'deep', label: 'Deep Dive' },
  { id: 'announcement', label: 'Announcements' },
]

const activeTab = ref('all')
const newsItems = ref<PcNewsCard[]>([])
const loading = ref(false)
const errorMessage = ref('')

const filteredNews = computed(() => {
  if (activeTab.value === 'all') return newsItems.value
  return newsItems.value.filter(news => news.category === activeTab.value)
})

async function loadNews() {
  loading.value = true
  errorMessage.value = ''
  try {
    const res = await fetchPublicNews({ limit: 100 })
    if (res.data.code === 0) {
      newsItems.value = res.data.data
    } else {
      errorMessage.value = res.data.message || 'Failed to load news.'
    }
  } catch (error: any) {
    errorMessage.value = error?.response?.data?.message || 'Failed to load news.'
  } finally {
    loading.value = false
  }
}

function getCategoryIcon(category: string) {
  switch (category) {
    case 'flash': return 'mdi:flash'
    case 'deep': return 'mdi:book-open-page-variant'
    case 'announcement': return 'mdi:bullhorn'
    default: return 'mdi:newspaper'
  }
}

onMounted(loadNews)
</script>
