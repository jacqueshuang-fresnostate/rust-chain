<template>
  <div :class="containerClass">
    <img :src="logoSrc" :alt="settingStore.platformName" :class="imageClass" @error="logoFailed = true" />
    <span v-if="showName" :class="nameClass">{{ settingStore.platformName }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import defaultLogo from '@/assets/logo/logo.png'
import { useSettingStore } from '@/stores/setting'

withDefaults(defineProps<{
  containerClass?: string
  imageClass?: string
  nameClass?: string
  showName?: boolean
}>(), {
  containerClass: 'flex items-center gap-2',
  imageClass: 'w-28 h-11 object-contain drop-shadow-neon',
  nameClass: 'text-base font-black tracking-tight text-foreground',
  showName: false,
})

const settingStore = useSettingStore()
const logoFailed = ref(false)
const logoSrc = computed(() => (settingStore.brandLogoUrl && !logoFailed.value ? settingStore.brandLogoUrl : defaultLogo))

watch(() => settingStore.brandLogoUrl, () => {
  logoFailed.value = false
})
</script>
