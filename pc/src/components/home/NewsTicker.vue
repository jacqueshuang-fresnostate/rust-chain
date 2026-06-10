<template>
  <div class="h-80 bg-card/30 border border-border rounded-xl p-4 overflow-hidden flex flex-col backdrop-blur-sm shadow-neon">
    <div class="flex items-center justify-between mb-4 border-b border-border/50 pb-2">
      <div class="flex gap-4">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          @click="activeTab = tab.id"
          class="text-sm font-bold transition-colors relative py-1"
          :class="activeTab === tab.id ? 'text-primary' : 'text-muted-foreground hover:text-foreground'"
        >
          {{ tab.label }}
          <div v-if="activeTab === tab.id" class="absolute bottom-0 left-0 w-full h-0.5 bg-primary shadow-neon"></div>
        </button>
      </div>
      <button class="text-xs text-primary hover:underline flex items-center gap-1">
        More <Icon icon="mdi:arrow-right" class="w-3 h-3" />
      </button>
    </div>

    <div class="flex-1 overflow-y-auto space-y-4 pr-2 custom-scrollbar">
      <div v-for="news in currentNews" :key="news.id" class="group cursor-pointer">
        <div class="flex gap-3">
          <div class="min-w-[4px] bg-border group-hover:bg-primary transition-colors rounded-full mt-1.5 h-auto"></div>
          <div>
            <h4 class="font-bold text-sm group-hover:text-primary transition-colors line-clamp-2 leading-tight mb-1">
              {{ news.title }}
            </h4>
            <div class="flex items-center gap-2 text-xs text-muted-foreground">
              <span class="font-mono">{{ news.time }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { Icon } from '@iconify/vue'

const tabs = [
  { id: 'flash', label: 'Flash' },
  { id: 'depth', label: 'Depth' },
  { id: 'notice', label: 'Notices' }
]

const activeTab = ref('flash')

const newsData = {
  flash: [
    { id: 1, title: 'Bitcoin Breaks $45k Resistance Level as ETF Hype Continues', time: '10:45' },
    { id: 2, title: 'Ethereum Layer 2 TVL Reaches All-Time High of $20B', time: '09:30' },
    { id: 3, title: 'Solana Network Upgrade Promises 10x Throughput Improvement', time: '08:15' },
    { id: 4, title: 'Regulatory Framework for Stablecoins Proposed by EU Commission', time: '07:00' },
    { id: 5, title: 'New NFT Marketplace Launches with Zero Fees for Creators', time: 'Yesterday' },
  ],
  depth: [
    { id: 101, title: 'Analysis: Why the Next Bull Run Will Be Different', time: '2024-01-20' },
    { id: 102, title: 'Deep Dive into Zero-Knowledge Proofs and Privacy', time: '2024-01-19' },
    { id: 103, title: 'The State of DeFi 2.0: Sustainable Yields?', time: '2024-01-18' },
  ],
  notice: [
    { id: 201, title: 'System Maintenance Scheduled for Jan 25th 02:00 UTC', time: '2024-01-22' },
    { id: 202, title: 'New Listing: HIPPO Token (HPO) Coming Soon', time: '2024-01-21' },
    { id: 203, title: 'Update on Withdrawal Limits for Unverified Accounts', time: '2024-01-20' },
  ]
}

const currentNews = computed(() => {
  return newsData[activeTab.value as keyof typeof newsData] || []
})
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: hsl(var(--muted-foreground) / 0.3);
  border-radius: 4px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: hsl(var(--muted-foreground) / 0.5);
}
</style>
