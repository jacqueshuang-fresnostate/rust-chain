<script setup lang="ts">
import { computed } from 'vue'
import { Icon } from '@iconify/vue'
import { useI18n } from 'vue-i18n'
import { useAuthRequired } from '@/composables/useAuthRequired'

const props = withDefaults(defineProps<{
  title?: string
  description?: string
  compact?: boolean
}>(), {
  title: '',
  description: '',
  compact: false,
})

const { t } = useI18n()
const { goToLogin } = useAuthRequired()

const titleText = computed(() => props.title || t('common.login_required_title'))
const descriptionText = computed(() => props.description || t('common.login_required_desc'))
</script>

<template>
  <div
    class="flex flex-col items-center justify-center rounded-xl border border-dashed border-border bg-muted/20 text-center"
    :class="compact ? 'min-h-36 p-4' : 'min-h-[360px] p-8'"
  >
    <div
      class="flex items-center justify-center rounded-full bg-primary/10 text-primary"
      :class="compact ? 'h-10 w-10' : 'h-14 w-14'"
    >
      <Icon icon="mdi:lock-outline" :class="compact ? 'h-5 w-5' : 'h-7 w-7'" />
    </div>
    <div class="mt-4 max-w-md">
      <div class="font-bold text-foreground" :class="compact ? 'text-sm' : 'text-lg'">{{ titleText }}</div>
      <div class="mt-2 text-sm leading-6 text-muted-foreground">{{ descriptionText }}</div>
    </div>
    <button
      type="button"
      class="mt-5 inline-flex h-10 items-center justify-center rounded-lg bg-primary px-5 text-sm font-bold text-primary-foreground transition hover:bg-primary/90"
      @click="goToLogin"
    >
      {{ t('common.login_now') }}
    </button>
  </div>
</template>
