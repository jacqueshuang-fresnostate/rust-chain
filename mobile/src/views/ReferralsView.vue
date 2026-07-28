<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Check, Copy, Link2, UsersRound } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { bindReferralCode, fetchReferralCode, fetchReferralInvites, type InviteRecord, type ReferralCode } from '@/api/user'
import { formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'

const session = useSessionStore()
const { t } = useI18n()
const code = ref<ReferralCode | null>(null)
const invites = ref<InviteRecord[]>([])
const loading = ref(false)
const error = ref('')
const copied = ref(false)
const binding = ref(false)
const bindCode = ref('')
const success = ref('')

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextCode, nextInvites] = await Promise.all([fetchReferralCode(), fetchReferralInvites()])
    code.value = nextCode
    invites.value = nextInvites
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('referrals.loadFailed'))
  } finally {
    loading.value = false
  }
}

async function copyCode(): Promise<void> {
  if (!code.value?.code) return
  try { await navigator.clipboard.writeText(code.value.code) } catch {
    const field = document.createElement('textarea')
    field.value = code.value.code
    document.body.appendChild(field)
    field.select()
    document.execCommand('copy')
    field.remove()
  }
  copied.value = true
  window.setTimeout(() => { copied.value = false }, 1_600)
}

async function bindCodeToAccount(): Promise<void> {
  if (!bindCode.value.trim()) {
    error.value = t('referrals.codeRequired')
    return
  }
  binding.value = true
  error.value = ''
  try {
    await bindReferralCode(bindCode.value)
    bindCode.value = ''
    success.value = t('referrals.bound')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('referrals.bindFailed'))
  } finally {
    binding.value = false
  }
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain referrals-page">
    <PageHeader :title="t('referrals.title')" />
    <div class="page-content referrals-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('referrals.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message referrals-feedback" role="alert">{{ error }}</p>
        <p v-if="success" class="success-message referrals-feedback" role="status">{{ success }}</p>
        <p v-if="loading" class="empty-state referrals-loading" role="status">{{ t('referrals.loading') }}</p>
        <template v-else>
          <section v-if="code" class="referral-code">
            <UsersRound :size="24" />
            <span>{{ t('referrals.myCode') }}</span>
            <strong class="numeric">{{ code.code || '--' }}</strong>
            <p>{{ code.usageLimit ? t('referrals.usedWithLimit', { count: code.usedCount, limit: code.usageLimit }) : t('referrals.usedCount', { count: code.usedCount }) }}</p>
            <button class="button button--primary button--full" type="button" aria-live="polite" @click="copyCode">
              <Check v-if="copied" :size="17" /><Copy v-else :size="17" />{{ copied ? t('referrals.copied') : t('referrals.copyCode') }}
            </button>
          </section>

          <section class="bind-code">
            <header>
              <Link2 :size="19" />
              <div><strong>{{ t('referrals.bindCode') }}</strong><p>{{ t('referrals.bindDescription') }}</p></div>
            </header>
            <form :aria-busy="binding" @submit.prevent="bindCodeToAccount">
              <input v-model="bindCode" class="input" maxlength="64" :placeholder="t('referrals.inputPlaceholder')" />
              <button class="button button--secondary" type="submit" :disabled="binding">
                {{ binding ? t('referrals.binding') : t('referrals.bind') }}
              </button>
            </form>
          </section>

          <section class="invite-history">
            <div class="section-heading"><span>{{ t('referrals.history') }}</span></div>
            <article v-for="invite in invites" :key="invite.userId" class="invite-row">
              <div>
                <strong>{{ invite.email || invite.phone || t('referrals.userNumber', { id: invite.userId }) }}</strong>
                <small>{{ formatDateTime(invite.createdAt) }}</small>
              </div>
              <span>{{ invite.status }}</span>
            </article>
            <p v-if="!invites.length" class="empty-state">{{ t('referrals.empty') }}</p>
          </section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.referrals-page { background: var(--background); }
.referrals-content { display: grid; gap: 22px; margin: 0 auto; max-width: 520px; padding-bottom: calc(44px + env(safe-area-inset-bottom)); padding-top: 18px; width: 100%; }
.referrals-feedback { border: 1px solid currentColor; border-radius: var(--radius); line-height: 1.45; margin: 0; padding: 11px 13px; }
.error-message.referrals-feedback { background: var(--negative-soft); }
.success-message { background: var(--positive-soft); color: var(--positive); font-size: 13px; font-weight: 680; }
.referrals-loading { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); margin: 0; padding: 24px 12px; }
.referral-code { align-items: center; border-bottom: 1px solid var(--line); display: grid; justify-items: center; padding: 8px 0 24px; text-align: center; }
.referral-code > svg { color: var(--accent); }
.referral-code > span { color: var(--muted); font-size: 13px; margin-top: 9px; }
.referral-code strong { font-size: 32px; letter-spacing: 0; margin-top: 4px; overflow-wrap: anywhere; }
.referral-code p { color: var(--muted-strong); font-size: 12px; line-height: 1.45; margin: 6px 0 18px; }
.referral-code .button { min-height: 52px; }
.bind-code { border-top: 1px solid var(--line); display: grid; gap: 13px; padding-top: 20px; }
.bind-code header { align-items: flex-start; display: flex; gap: 9px; }
.bind-code header > svg { color: var(--accent); flex: 0 0 auto; margin-top: 2px; }
.bind-code header div { display: grid; gap: 4px; min-width: 0; }
.bind-code strong { font-size: 15px; }
.bind-code p { color: var(--muted); font-size: 11px; line-height: 1.45; margin: 0; }
.bind-code form { display: grid; gap: 9px; grid-template-columns: minmax(0, 1fr) 88px; }
.bind-code .input { min-height: 52px; }
.bind-code .button { font-size: 12px; min-height: 52px; padding: 0 7px; }
.invite-history { border-top: 1px solid var(--line); }
.invite-history .section-heading { font-size: 19px; margin: 20px 0 8px; }
.invite-row { align-items: center; border-bottom: 1px solid var(--line); display: flex; gap: 12px; justify-content: space-between; min-height: 68px; padding: 9px 0; }
.invite-row div { display: grid; gap: 5px; min-width: 0; }
.invite-row strong { font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.invite-row small { color: var(--muted); font-size: 11px; }
.invite-row > span { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--muted-strong); flex: 0 0 auto; font-size: 11px; max-width: 38%; overflow: hidden; padding: 5px 8px; text-overflow: ellipsis; white-space: nowrap; }
@media (max-width: 340px) {
  .referrals-content { padding-left: 16px; padding-right: 16px; }
  .bind-code form { grid-template-columns: 1fr; }
}
</style>
