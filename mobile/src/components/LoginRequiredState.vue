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
  <section
    class="login-required"
    role="group"
    :aria-label="t('common.loginRequiredTitle')"
  >
    <span class="login-required__icon"><LockKeyhole :size="22" /></span>
    <div class="login-required__copy">
      <strong>{{ t('common.loginRequiredTitle') }}</strong>
      <p>{{ resolvedDescription }}</p>
    </div>
    <button class="button button--secondary" type="button" @click="goToLogin">
      {{ t('common.loginNow') }}
    </button>
  </section>
</template>

<style scoped>
.login-required {
  align-items: center;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--accent) 8%, transparent), transparent 48%),
    var(--surface-elevated);
  border: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  color: var(--muted);
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 28px 20px 22px;
  text-align: center;
}

.login-required__icon {
  align-items: center;
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 34%, var(--line));
  border-radius: 50%;
  color: var(--accent);
  display: inline-flex;
  height: 52px;
  justify-content: center;
  width: 52px;
}

.login-required__copy {
  display: grid;
  gap: 6px;
  max-width: 320px;
}

.login-required strong {
  color: var(--ink);
  font-size: 16px;
}

.login-required p {
  font-size: 13px;
  line-height: 1.55;
  margin: 0;
}

.login-required .button {
  min-height: 46px;
  min-width: min(220px, 100%);
}
</style>
