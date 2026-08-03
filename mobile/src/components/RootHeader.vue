<script setup lang="ts">
import { computed } from 'vue'
import { Bell, Moon, Sun } from 'lucide-vue-next'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import landscapeLogo from '@/assets/brand/hippo-logo-landscape.png'
import { useThemeStore } from '@/stores/theme'

const route = useRoute()
const router = useRouter()
const theme = useThemeStore()
const { t } = useI18n()
const isHome = computed(() => route.name === 'home')
</script>

<template>
  <header class="topbar root-header">
    <button
      class="brand-button root-header__brand"
      type="button"
      :aria-label="t('nav.home')"
      :aria-current="isHome ? 'page' : undefined"
      :title="t('nav.home')"
      @click="router.replace({ name: 'home' })"
    >
      <img
        class="brand-logo"
        :src="landscapeLogo"
        alt=""
        aria-hidden="true"
      >
    </button>
    <div class="topbar-actions action-cluster root-header__actions" role="group" :aria-label="t('nav.main')">
      <button
        class="icon-button root-header__control"
        type="button"
        :aria-label="t(theme.isDark ? 'home.switchToLightTheme' : 'home.switchToDarkTheme')"
        :title="t(theme.isDark ? 'home.switchToLightTheme' : 'home.switchToDarkTheme')"
        :aria-pressed="theme.isDark"
        @click="theme.toggleTheme"
      >
        <Sun v-if="theme.isDark" :size="18" aria-hidden="true" />
        <Moon v-else :size="18" aria-hidden="true" />
      </button>
      <button
        class="icon-button has-dot root-header__control root-header__message"
        type="button"
        :aria-label="t('home.openMessageCenter')"
        :title="t('home.openMessageCenter')"
        @click="router.push({ name: 'message-center' })"
      >
        <Bell :size="18" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>
