<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Check, Copy, LockKeyhole } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { bindReferralCode, fetchReferralCode, fetchReferralInvites, type InviteRecord, type ReferralCode } from '@/api/user'
import { formatDateTime } from '@/core/format'
import { referralStatusPresentation } from '@/core/financialEnumPresentation'
import { useSessionStore } from '@/stores/session'

const session = useSessionStore()
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const code = ref<ReferralCode | null>(null)
const invites = ref<InviteRecord[]>([])
const loading = ref(false)
const error = ref('')
const copied = ref(false)
const binding = ref(false)
const bindCode = ref('')
const success = ref('')
const inviteCount = computed(() => invites.value.length)
const remainingUses = computed(() => code.value?.usageLimit === undefined
  ? null
  : Math.max(0, code.value.usageLimit - code.value.usedCount))

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: route.fullPath } })
}

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

function inviteStatusLabel(status: string): string {
  const presentation = referralStatusPresentation(status)
  return t(presentation.translationKey, { source: presentation.source || '--' })
}

onMounted(() => { void load() })
</script>

<template>
  <main
    class="page page--plain pencil-page referrals-page"
    data-pencil-source="c80gd Bmt4u e4bPj Qy31s"
  >
    <PageHeader
      :back="true"
      :pencil="true"
      :title="t('referrals.title')"
    />
    <div class="pencil-content referrals-content">
      <section v-if="!session.isAuthenticated" class="account-login-state">
        <span class="account-login-state__icon"><LockKeyhole :size="20" /></span>
        <div><strong>{{ t('common.loginRequiredTitle') }}</strong><p>{{ t('referrals.loginDescription') }}</p></div>
        <button class="pencil-primary" type="button" @click="openLogin">{{ t('common.loginNow') }}</button>
      </section>
      <template v-else>
        <p v-if="error" class="pencil-message pencil-message--error referrals-feedback" role="alert">{{ error }}</p>
        <p v-else-if="success" class="pencil-message pencil-message--success referrals-feedback" role="status">{{ success }}</p>
        <p v-if="loading" class="referrals-loading" role="status">{{ t('referrals.loading') }}</p>
        <template v-else>
          <h1 class="referrals-intro">{{ t('referrals.myCode') }}</h1>
          <p class="referrals-intro-sub">{{ code?.usageLimit ? t('referrals.usedWithLimit', { count: code.usedCount, limit: code.usageLimit }) : t('referrals.bindDescription') }}</p>

          <section v-if="code" class="referral-code">
            <span><small>{{ t('referrals.myCode') }}</small><strong class="pencil-numeric">{{ code.code || t('common.serviceUnavailable') }}</strong></span>
            <button type="button" :aria-label="t('referrals.copyCode')" :disabled="!code.code" @click="copyCode">
              <Check v-if="copied" :size="16" /><Copy v-else :size="16" />
            </button>
          </section>

          <button class="pencil-primary referral-copy-action" type="button" :disabled="!code?.code" aria-live="polite" @click="copyCode">
            {{ copied ? t('referrals.copied') : t('referrals.copyCode') }}
          </button>

          <section class="bind-code">
            <form :aria-busy="binding" @submit.prevent="bindCodeToAccount">
              <label class="bind-field"><input v-model="bindCode" maxlength="64" :aria-label="t('referrals.bindCode')" :placeholder="t('referrals.inputPlaceholder')" /></label>
              <button type="submit" :disabled="binding">
                {{ binding ? t('referrals.binding') : t('referrals.bind') }}
              </button>
            </form>
          </section>

          <section class="referral-stats" :aria-label="t('referrals.history')">
            <div><strong class="pencil-numeric">{{ code?.usedCount ?? 0 }}</strong><small>{{ t('referrals.usedCount', { count: code?.usedCount ?? 0 }) }}</small></div>
            <div><strong class="pencil-numeric">{{ inviteCount }}</strong><small>{{ t('referrals.history') }}</small></div>
            <div><strong class="pencil-numeric">{{ remainingUses ?? '--' }}</strong><small>{{ code?.usageLimit ? t('referrals.usedWithLimit', { count: code.usedCount, limit: code.usageLimit }) : t('referrals.myCode') }}</small></div>
          </section>

          <section class="invite-history">
            <h2>{{ t('referrals.history') }}</h2>
            <article v-for="invite in invites" :key="invite.userId" class="invite-row">
              <div>
                <strong>{{ invite.email || invite.phone || t('referrals.userNumber', { id: invite.userId }) }}</strong>
                <small>{{ formatDateTime(invite.createdAt) }}</small>
              </div>
              <span>{{ inviteStatusLabel(invite.status) }}</span>
            </article>
            <p v-if="!invites.length" class="referrals-empty">{{ t('referrals.empty') }}</p>
          </section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.page.pencil-page.referrals-page { background: var(--page); background-image: none; min-height: 100dvh; }
.referrals-content { display: flex; flex-direction: column; gap: 14px; padding-bottom: calc(20px + env(safe-area-inset-bottom)); padding-top: 6px; }
.account-login-state { align-items: center; display: grid; gap: 12px; grid-template-columns: 44px minmax(0, 1fr) auto; min-height: 76px; }
.account-login-state__icon { align-items: center; background: var(--accent-soft); border-radius: 50%; color: var(--positive); display: inline-flex; height: 44px; justify-content: center; width: 44px; }
.account-login-state div { display: grid; gap: 3px; min-width: 0; }
.account-login-state strong { color: var(--ink); font-size: 14px; }
.account-login-state p { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
.account-login-state .pencil-primary { min-height: 44px; padding-inline: 16px; }
.referrals-feedback { margin: 0; }
.referrals-loading { color: var(--muted); font-size: 11px; margin: 0; min-height: 44px; padding-block: 14px; }
.referrals-intro { color: var(--ink); font-size: 20px; font-weight: 400; line-height: 28px; margin: 0; }
.referrals-intro-sub { color: var(--muted-strong); font-size: 12px; line-height: 17px; margin: 0; }
.referral-code { align-items: center; background: var(--accent-soft); border-radius: 12px; display: flex; height: 56px; justify-content: space-between; padding: 0 16px; }
.referral-code > span { display: grid; gap: 3px; min-width: 0; }
.referral-code small { color: var(--muted-strong); font-size: 10px; font-weight: 500; }
.referral-code strong { color: var(--ink); font-size: 16px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.referral-code > button { background: transparent; color: var(--ink); display: grid; min-height: 44px; min-width: 44px; place-items: center; }
.referral-copy-action { font-size: 15px; height: 48px; min-height: 48px; width: 100%; }
.bind-code form { display: grid; gap: 10px; grid-template-columns: minmax(0, 1fr) 84px; }
.bind-field { align-items: center; background: var(--surface); border: 1px solid var(--line); border-radius: 22px; display: flex; height: 44px; min-width: 0; padding: 0 16px; }
.bind-field:focus-within { border-color: var(--positive); box-shadow: 0 0 0 2px var(--focus-ring); }
.bind-field input { background: transparent; border: 0; color: var(--ink); min-height: 42px; min-width: 0; outline: 0; padding: 0; width: 100%; }
.bind-code form > button { background: var(--ink); border-radius: 22px; color: var(--surface-elevated); font-size: 13px; font-weight: 700; height: 44px; min-height: 44px; padding: 0 10px; }
.referral-stats { display: grid; gap: 10px; grid-template-columns: repeat(3, minmax(0, 1fr)); padding-top: 6px; }
.referral-stats > div { align-items: center; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.referral-stats strong { color: var(--ink); font-size: 18px; font-weight: 700; line-height: 25px; }
.referral-stats small { color: var(--muted); font-size: 10px; font-weight: 500; line-height: 14px; max-width: 100%; overflow: hidden; text-align: center; text-overflow: ellipsis; white-space: nowrap; }
.invite-history { display: grid; gap: 4px; padding-top: 8px; }
.invite-history h2 { color: var(--ink); font-size: 14px; font-weight: 700; line-height: 20px; margin: 0; }
.invite-row { align-items: center; display: flex; gap: 10px; height: 44px; justify-content: space-between; min-height: 44px; }
.invite-row div { display: grid; gap: 5px; min-width: 0; }
.invite-row strong { color: var(--ink); font-size: 12px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.invite-row small { color: var(--muted); font-size: 10px; }
.invite-row > span { color: var(--positive); flex: 0 0 auto; font-size: 12px; max-width: 38%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.referrals-empty { color: var(--muted); font-size: 11px; margin: 0; min-height: 44px; padding-block: 14px; text-align: center; }
.referrals-page button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
@media (max-width: 340px) {
  .referrals-content { padding-inline: 16px; }
  .account-login-state { align-items: start; grid-template-columns: 44px minmax(0, 1fr); }
  .account-login-state .pencil-primary { grid-column: 2; justify-self: start; }
  .bind-code form { gap: 8px; grid-template-columns: minmax(0, 1fr) 76px; }
}
</style>
