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
    linear-gradient(110deg, color-mix(in srgb, var(--positive) 7%, transparent), transparent 54%),
    var(--surface-elevated);
  border: 1px solid var(--line);
  border-left: 3px solid var(--positive);
  color: var(--muted);
  display: grid;
  gap: 12px;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  min-height: 96px;
  padding: 14px;
  text-align: left;
}

.login-required__icon {
  align-items: center;
  background: var(--positive-soft);
  border: 1px solid color-mix(in srgb, var(--positive) 34%, var(--line));
  border-radius: 50%;
  color: var(--positive);
  display: inline-flex;
  height: 44px;
  justify-content: center;
  width: 44px;
}

.login-required__copy {
  display: grid;
  gap: 6px;
  max-width: 320px;
}

.login-required strong {
  color: var(--ink);
  font-size: 14px;
}

.login-required p {
  font-size: 13px;
  line-height: 1.55;
  margin: 0;
}

.login-required .button {
  min-height: 46px;
  padding-inline: 13px;
  white-space: nowrap;
}

@media (max-width: 340px) {
  .login-required {
    align-items: start;
    grid-template-columns: 44px minmax(0, 1fr);
  }

  .login-required .button {
    grid-column: 2;
    justify-self: start;
  }
}
</style>
