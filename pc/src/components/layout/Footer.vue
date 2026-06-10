<template>
  <footer class="h-8 bg-background border-t border-border flex items-center px-4 justify-between text-xs text-muted-foreground select-none">
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-1.5">
        <div class="w-2 h-2 rounded-full bg-up animate-pulse"></div>
        <span>Stable Connection</span>
      </div>
      <span>v0.1.0</span>
      <span class="hover:text-foreground cursor-pointer transition-colors flex items-center gap-1" @click="checkUpdate">
        <Icon icon="mdi:update" class="w-3 h-3" />
        {{ updateStatus }}
      </span>
    </div>

    <div class="flex items-center gap-4">
      <span>24h Vol: ${{ formatNumber(1234567890, 'volume') }}</span>
      <span>UTC {{ currentTime }}</span>
    </div>
  </footer>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { Icon } from '@iconify/vue'
import { formatNumber } from '@/utils/format'

const currentTime = ref(new Date().toISOString().slice(11, 19))
const updateStatus = ref('Check for Updates')
let timer: any

function checkUpdate() {
  updateStatus.value = 'Updates unavailable'
}

onMounted(() => {
  timer = setInterval(() => {
    currentTime.value = new Date().toISOString().slice(11, 19)
  }, 1000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})
</script>
