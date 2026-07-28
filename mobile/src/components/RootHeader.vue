<script setup lang="ts">
import { Bell, Moon, Sun } from 'lucide-vue-next'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import compactLogo from '@/assets/brand/hippo-logo-compact.png'
import { useThemeStore } from '@/stores/theme'

const router = useRouter()
const theme = useThemeStore()
const { t } = useI18n()
</script>

<template>
  <header class="topbar">
    <button
      class="brand-button"
      type="button"
      :aria-label="t('nav.profile')"
      @click="router.replace({ name: 'profile' })"
    >
      <span
        class="brand-logo"
        :style="{ backgroundImage: `url(${compactLogo})` }"
        aria-hidden="true"
      />
    </button>
    <div class="topbar-actions action-cluster" role="group" :aria-label="t('nav.main')">
      <button
        class="icon-button"
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
        class="icon-button has-dot"
        type="button"
        :aria-label="t('home.openMessageCenter')"
        @click="router.push({ name: 'message-center' })"
      >
        <Bell :size="18" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>
