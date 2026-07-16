<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { LockKeyhole } from 'lucide-vue-next'

const props = defineProps<{ description?: string }>()

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const resolvedDescription = computed(() => props.description || t('common.loginRequiredDescription'))

function goToLogin() {
  void router.push({ name: 'login', query: { redirect: route.fullPath } })
}
</script>

<template>
  <div class="login-required">
    <span class="login-required__icon"><LockKeyhole :size="22" /></span>
    <p>{{ resolvedDescription }}</p>
    <button class="button button--secondary" type="button" @click="goToLogin">{{ t('common.loginNow') }}</button>
  </div>
</template>

<style scoped>
.login-required { align-items: center; color: var(--muted); display: flex; flex-direction: column; gap: 12px; padding: 34px 24px; text-align: center; }
.login-required__icon { align-items: center; background: var(--soft); border-radius: 50%; color: var(--ink); display: inline-flex; height: 44px; justify-content: center; width: 44px; }
.login-required p { font-size: 14px; margin: 0; }
</style>
