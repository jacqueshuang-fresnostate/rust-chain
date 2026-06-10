<template>
  <div class="flex flex-col h-screen overflow-hidden bg-background text-foreground">
    <Header />
    <main class="flex-1 overflow-auto relative">
      <router-view v-slot="{ Component }">
        <component :is="Component" :key="$route.fullPath" />
      </router-view>
    </main>
    <Footer />
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { stompService } from '@/api/stomp'
import Header from './Header.vue'
import Footer from './Footer.vue'

onMounted(() => {
    stompService.connect()
})
</script>

<style>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
