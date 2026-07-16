<template>
  <div class="space-y-8">
    <h2 class="text-2xl font-bold mb-6">{{ t('security.title') }}</h2>

    <div v-if="initLoading" class="flex justify-center py-10">
      <Icon icon="mdi:loading" class="animate-spin text-4xl text-primary" />
    </div>

    <template v-else>
      <!-- Password Management -->
      <div class="space-y-4">
        <h3 class="text-lg font-bold flex items-center gap-2">
          <Icon icon="mdi:lock-outline" class="text-primary" />
          {{ t('security.pwd_manage') }}
        </h3>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div>
              <div class="font-bold">{{ t('security.login_pwd') }}</div>
              <div class="text-xs text-muted-foreground">{{ t('security.login_desc') }}</div>
            </div>
            <button @click="openPasswordModal('login')" class="text-primary text-sm font-bold hover:underline">{{ t('security.change') }}</button>
          </div>
          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div>
              <div class="font-bold">{{ t('security.trade_pwd') }}</div>
              <div class="text-xs text-muted-foreground">{{ t('security.trade_desc') }}</div>
            </div>
            <div v-if="!isTradePasswordSet">
              <button @click="openPasswordModal('trade-set')" class="text-primary text-sm font-bold hover:underline">{{ t('security.set') }}</button>
            </div>
            <div v-else class="flex gap-3">
              <button @click="openPasswordModal('trade-update')" class="text-primary text-sm font-bold hover:underline">{{ t('security.change') }}</button>
              <button @click="openPasswordModal('trade-reset')" class="text-muted-foreground text-sm font-bold hover:underline">{{ t('security.reset') }}</button>
            </div>
          </div>
        </div>
      </div>

      <!-- Two-Factor Authentication -->
      <div class="space-y-4">
        <h3 class="text-lg font-bold flex items-center gap-2">
          <Icon icon="mdi:shield-key-outline" class="text-primary" />
          {{ t('security.two_factor_title') }}
        </h3>
        <div class="p-4 bg-muted/30 rounded border border-border space-y-4">
          <div class="flex justify-between items-start gap-4">
            <div>
              <div class="font-bold">{{ t('security.authenticator_app') }}</div>
              <div class="text-xs text-muted-foreground">
                {{ twoFactorStatus?.totp_enabled ? t('security.bound') : t('security.not_bound') }}
              </div>
            </div>
            <div class="flex gap-3">
              <button v-if="!twoFactorStatus?.totp_enabled" @click="startTwoFactorSetup" class="text-primary text-sm font-bold hover:underline">{{ t('security.bind') }}</button>
              <button v-else @click="openTwoFactorReset" class="text-muted-foreground text-sm font-bold hover:underline">{{ t('security.reset') }}</button>
            </div>
          </div>
          <div v-if="twoFactorStatus?.totp_enabled" class="flex justify-between items-center border-t border-border pt-4">
            <div>
              <div class="font-bold">{{ t('security.login_2fa') }}</div>
              <div class="text-xs text-muted-foreground">
                {{ twoFactorStatus.can_toggle_login_2fa ? t('security.user_controlled_policy') : t('security.admin_controlled_policy') }}
              </div>
            </div>
            <button
              @click="toggleLoginTwoFactor"
              :disabled="twoFactorLoading || !twoFactorStatus.can_toggle_login_2fa"
              class="px-3 py-1.5 rounded text-xs font-bold border border-border hover:border-primary disabled:opacity-50"
            >
              {{ twoFactorStatus.login_2fa_enabled ? t('security.disable') : t('security.enable') }}
            </button>
          </div>
        </div>
      </div>

      <!-- Account Binding -->
      <div class="space-y-4">
        <h3 class="text-lg font-bold flex items-center gap-2">
          <Icon icon="mdi:link-variant" class="text-primary" />
          {{ t('security.account_bind') }}
        </h3>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div class="flex items-center gap-3">
              <Icon icon="mdi:email-outline" class="text-2xl text-muted-foreground" />
              <div>
                <div class="font-bold">{{ t('security.email') }}</div>
                <div class="text-xs text-muted-foreground">{{ securitySetting?.email || t('security.not_bound') }}</div>
              </div>
            </div>
            <span v-if="securitySetting?.email" class="text-up text-xs font-bold border border-up/30 px-2 py-1 rounded bg-up/10">{{ t('security.verified') }}</span>
            <button v-else class="text-primary text-sm font-bold hover:underline">{{ t('security.bind') }}</button>
          </div>

          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div class="flex items-center gap-3">
              <Icon icon="mdi:account-circle-outline" class="text-2xl text-muted-foreground" />
              <div>
                <div class="font-bold">{{ t('security.username') }}</div>
                <div class="text-xs text-muted-foreground">{{ securitySetting?.username || t('security.username_not_set') }}</div>
              </div>
            </div>
            <button @click="openUsernameModal" class="text-primary text-sm font-bold hover:underline">
              {{ securitySetting?.username ? t('security.change') : t('security.set') }}
            </button>
          </div>

          <div v-if="coinbaseEnabled" class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div class="flex items-center gap-3">
              <Icon icon="simple-icons:coinbase" class="text-2xl text-[#0052FF]" />
              <div>
                <div class="font-bold">{{ t('security.wallet') }}</div>
                <div class="text-xs text-muted-foreground">
                  {{ coinbaseBinding ? thirdPartyBindingLabel(coinbaseBinding) : t('security.not_connected') }}
                </div>
              </div>
            </div>
            <button
                v-if="!coinbaseBinding"
                @click="openThirdPartyModal('coinbase_wallet')"
                class="px-3 py-1.5 bg-[#0052FF] text-white text-xs font-bold rounded hover:opacity-90 transition-all flex items-center gap-1"
                :disabled="thirdPartyLoading"
            >
              <Icon v-if="thirdPartyLoading" icon="mdi:loading" class="animate-spin" />
              <Icon v-else icon="mdi:plus" />
              {{ t('security.connect') }}
            </button>
            <span v-else-if="coinbaseBinding" class="text-up text-xs font-bold border border-up/30 px-2 py-1 rounded bg-up/10">{{ t('security.linked') }}</span>
          </div>

          <div v-if="telegramEnabled" class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div class="flex items-center gap-3">
              <Icon icon="simple-icons:telegram" class="text-2xl text-[#26A5E4]" />
              <div>
                <div class="font-bold">{{ t('security.telegram') }}</div>
                <div class="text-xs text-muted-foreground">
                  {{ telegramBinding ? thirdPartyBindingLabel(telegramBinding) : t('security.not_connected') }}
                </div>
              </div>
            </div>
            <button
                v-if="!telegramBinding"
                @click="openThirdPartyModal('telegram_account')"
                class="px-3 py-1.5 bg-primary text-primary-foreground text-xs font-bold rounded hover:bg-primary/90 transition-all flex items-center gap-1"
                :disabled="thirdPartyLoading"
            >
              <Icon v-if="thirdPartyLoading" icon="mdi:loading" class="animate-spin" />
              <Icon v-else icon="mdi:plus" />
              {{ t('security.bind') }}
            </button>
            <span v-else-if="telegramBinding" class="text-up text-xs font-bold border border-up/30 px-2 py-1 rounded bg-up/10">{{ t('security.linked') }}</span>
          </div>
        </div>
      </div>
    </template>
  </div>

  <div v-if="showUsernameModal" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div class="bg-card border border-border p-6 rounded-lg w-full max-w-md shadow-neon animate-in fade-in zoom-in duration-200">
          <h3 class="text-xl font-bold mb-4">{{ t('security.update_username') }}</h3>
          <form @submit.prevent="saveUsername" class="space-y-4">
              <div>
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.username') }}</label>
                  <input
                    v-model="usernameForm.username"
                    type="text"
                    required
                    minlength="3"
                    maxlength="32"
                    autocomplete="username"
                    :placeholder="t('security.username_placeholder')"
                    class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors"
                  />
                  <p class="mt-2 text-xs text-muted-foreground">{{ t('security.username_rule') }}</p>
              </div>
              <div class="flex justify-end gap-2 mt-6">
                  <button type="button" @click="closeUsernameModal" :disabled="usernameLoading" class="px-4 py-2 text-sm font-bold text-muted-foreground hover:text-foreground">{{ t('security.cancel') }}</button>
                  <button type="submit" :disabled="usernameLoading" class="px-4 py-2 text-sm font-bold bg-primary text-primary-foreground rounded hover:bg-primary/90 flex items-center gap-2">
                    <Icon v-if="usernameLoading" icon="mdi:loading" class="animate-spin" />
                    {{ t('security.save') }}
                  </button>
              </div>
          </form>
      </div>
  </div>

  <!-- Dynamic Modal for Password Management -->
  <div v-if="showModal" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div class="bg-card border border-border p-6 rounded-lg w-full max-w-md shadow-neon animate-in fade-in zoom-in duration-200">
          <h3 class="text-xl font-bold mb-4">{{ modalTitle }}</h3>

          <form @submit.prevent="handleSave" class="space-y-4">
              <!-- Old Password (Only for update) -->
              <div v-if="modalType === 'trade-update'">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.current_trade') }}</label>
                  <input v-model="form.oldPassword" type="password" required class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- Login Password (Required by backend when setting trade password) -->
              <div v-if="modalType === 'trade-set'">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.current_pwd') }}</label>
                  <input v-model="form.loginPassword" type="password" required :placeholder="t('auth.password_placeholder')" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- New Password (set, update, reset) -->
              <div v-if="['trade-set', 'trade-update', 'trade-reset'].includes(modalType)">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.new_trade') }}</label>
                  <input v-model="form.newPassword" type="password" required :placeholder="t('auth.password_placeholder')" minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- Confirm New Password -->
              <div v-if="['trade-set', 'trade-update', 'trade-reset'].includes(modalType)">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.confirm_trade') }}</label>
                  <input v-model="form.confirmPassword" type="password" required :placeholder="t('auth.confirm_placeholder')" minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- Verification Code (Only for reset) -->
              <div v-if="modalType === 'trade-reset'">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.code') }}</label>
                  <div class="flex gap-2">
                      <input v-model="form.code" type="text" required class="flex-1 bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" placeholder="XXXXXX" />
                      <button type="button" @click="sendCode" :disabled="codeLoading || countdown > 0 || !securitySetting?.email" class="px-3 py-2 bg-secondary text-secondary-foreground text-xs font-bold rounded hover:bg-secondary/80 disabled:opacity-50 min-w-[100px]">
                          {{ countdown > 0 ? `${countdown}s` : (codeLoading ? '...' : t('security.send_code')) }}
                      </button>
                  </div>
              </div>

              <div v-if="modalType === 'login'">
                  <div>
                      <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.current_pwd') }}</label>
                      <input v-model="form.oldPassword" type="password" required class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
                  </div>
                  <div class="mt-4">
                      <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('auth.new_password') }}</label>
                      <input v-model="form.newPassword" type="password" required minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
                  </div>
                  <div class="mt-4">
                      <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.confirm_trade') }}</label>
                      <input v-model="form.confirmPassword" type="password" required minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
                  </div>
              </div>

              <div class="flex justify-end gap-2 mt-6">
                  <button type="button" @click="showModal = false" :disabled="submitLoading" class="px-4 py-2 text-sm font-bold text-muted-foreground hover:text-foreground">{{ t('security.cancel') }}</button>
                  <button type="submit" :disabled="submitLoading" class="px-4 py-2 text-sm font-bold bg-primary text-primary-foreground rounded hover:bg-primary/90 flex items-center gap-2">
                    <Icon v-if="submitLoading" icon="mdi:loading" class="animate-spin" />
                    {{ t('security.save') }}
                  </button>
              </div>
          </form>
      </div>
  </div>

  <div v-if="showTwoFactorModal" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div class="bg-card border border-border p-6 rounded-lg w-full max-w-md shadow-neon animate-in fade-in zoom-in duration-200">
          <h3 class="text-xl font-bold mb-4">{{ twoFactorModalTitle }}</h3>
          <div v-if="twoFactorModalType === 'setup'" class="space-y-4">
              <div v-if="twoFactorSetup" class="p-4 bg-muted/30 rounded-xl border border-border text-sm">
                  <div class="flex flex-col sm:flex-row items-center sm:items-start gap-4">
                      <div class="w-48 h-48 shrink-0 rounded-xl bg-white p-3 border border-border flex items-center justify-center">
                          <img
                              v-if="twoFactorQrCodeUrl"
                              :src="twoFactorQrCodeUrl"
                              :alt="t('security.two_factor_qr_alt')"
                              class="w-full h-full object-contain"
                          />
                          <div v-else class="text-slate-500 text-xs text-center space-y-2">
                              <Icon icon="mdi:qrcode-scan" class="text-4xl mx-auto" />
                              <div>{{ twoFactorQrCodeError || t('security.two_factor_qr_failed') }}</div>
                          </div>
                      </div>
                      <div class="min-w-0 flex-1 space-y-3">
                          <div>
                              <div class="font-bold">{{ t('security.two_factor_scan_title') }}</div>
                              <p class="text-xs text-muted-foreground mt-1 leading-relaxed">{{ t('security.two_factor_scan_desc') }}</p>
                          </div>
                          <div class="rounded-lg border border-border bg-background/50 p-3">
                              <div class="text-xs text-muted-foreground mb-1">{{ t('security.two_factor_manual_key') }}</div>
                              <code class="block break-all text-primary font-mono">{{ twoFactorSetup.secret }}</code>
                          </div>
                      </div>
                  </div>
              </div>
              <label class="text-xs font-bold text-muted-foreground block">{{ t('security.two_factor_code') }}</label>
              <input v-model="twoFactorForm.totpCode" inputmode="numeric" pattern="[0-9]*" autocomplete="one-time-code" maxlength="6" placeholder="000000" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
          </div>
          <div v-else class="space-y-4">
              <p class="text-sm text-muted-foreground">{{ t('security.reset_email_required') }}</p>
              <div class="flex gap-2">
                  <input v-model="twoFactorForm.emailCode" class="flex-1 bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" :placeholder="t('security.email_code_placeholder')" />
                  <button type="button" @click="sendTwoFactorCode" :disabled="twoFactorLoading" class="px-3 py-2 bg-secondary text-secondary-foreground text-xs font-bold rounded hover:bg-secondary/80 disabled:opacity-50">
                      {{ t('security.send_code') }}
                  </button>
              </div>
          </div>
          <div class="flex justify-end gap-2 mt-6">
              <button type="button" @click="closeTwoFactorModal" :disabled="twoFactorLoading" class="px-4 py-2 text-sm font-bold text-muted-foreground hover:text-foreground">{{ t('security.cancel') }}</button>
              <button type="button" @click="saveTwoFactorModal" :disabled="isTwoFactorSubmitDisabled" class="px-4 py-2 text-sm font-bold bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50 flex items-center gap-2">
                  <Icon v-if="twoFactorLoading" icon="mdi:loading" class="animate-spin" />
                  {{ t('security.save') }}
              </button>
          </div>
      </div>
  </div>

  <div v-if="showThirdPartyModal" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div class="bg-card border border-border p-6 rounded-lg w-full max-w-md shadow-neon animate-in fade-in zoom-in duration-200">
          <h3 class="text-xl font-bold mb-4">{{ thirdPartyModalTitle }}</h3>
          <form @submit.prevent="saveThirdPartyBinding" class="space-y-4">
              <div>
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ thirdPartyIdentifierLabel }}</label>
                  <input
                      v-model="thirdPartyForm.accountIdentifier"
                      required
                      :placeholder="thirdPartyIdentifierPlaceholder"
                      class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors"
                  />
              </div>
              <div>
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.third_party_display_name') }}</label>
                  <input
                      v-model="thirdPartyForm.displayName"
                      :placeholder="t('security.third_party_display_name_placeholder')"
                      class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors"
                  />
              </div>
              <div class="flex justify-end gap-2 mt-6">
                  <button type="button" @click="closeThirdPartyModal" :disabled="thirdPartyLoading" class="px-4 py-2 text-sm font-bold text-muted-foreground hover:text-foreground">{{ t('security.cancel') }}</button>
                  <button type="submit" :disabled="thirdPartyLoading || !thirdPartyForm.accountIdentifier.trim()" class="px-4 py-2 text-sm font-bold bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50 flex items-center gap-2">
                    <Icon v-if="thirdPartyLoading" icon="mdi:loading" class="animate-spin" />
                    {{ t('security.save') }}
                  </button>
              </div>
          </form>
      </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { Icon } from '@iconify/vue'
import { toDataURL } from 'qrcode'
import { useToast } from 'vue-toastification'
import {
    bindThirdPartyAccount,
    changeLoginPassword,
    confirmTwoFactor,
    getSecuritySetting,
    getThirdPartyBindings,
    getTwoFactorStatus,
    resetTransactionPassword,
    resetTwoFactor,
    sendTransactionPasswordResetCode,
    sendTwoFactorResetCode,
    setTransactionPassword,
    setupTwoFactor,
    updateUsername,
    updateLoginTwoFactor,
    updateTransactionPassword,
    type MemberSecurity,
    type ThirdPartyBinding,
    type ThirdPartyBindingStatus,
    type ThirdPartyProvider,
    type TwoFactorSetup,
    type TwoFactorStatus,
} from '@/api/user'
import { useUserStore } from '@/stores/user'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const toast = useToast()
const userStore = useUserStore()
const showModal = ref(false)
const showUsernameModal = ref(false)
const showTwoFactorModal = ref(false)
const showThirdPartyModal = ref(false)
const modalTitle = ref('')
const submitLoading = ref(false)
const codeLoading = ref(false)
const twoFactorLoading = ref(false)
const usernameLoading = ref(false)
const thirdPartyLoading = ref(false)
const countdown = ref(0)
const initLoading = ref(true)

const securitySetting = ref<MemberSecurity | null>(null)
const twoFactorStatus = ref<TwoFactorStatus | null>(null)
const thirdPartyStatus = ref<ThirdPartyBindingStatus | null>(null)
const twoFactorSetup = ref<TwoFactorSetup | null>(null)
const twoFactorQrCodeUrl = ref('')
const twoFactorQrCodeError = ref('')
const modalType = ref<'trade-set' | 'trade-update' | 'trade-reset' | 'login'>('trade-set')
const twoFactorModalType = ref<'setup' | 'reset'>('setup')
const thirdPartyProvider = ref<ThirdPartyProvider>('coinbase_wallet')

const form = ref({
    oldPassword: '',
    loginPassword: '',
    newPassword: '',
    confirmPassword: '',
    code: ''
})

const twoFactorForm = ref({
    totpCode: '',
    emailCode: '',
})

const thirdPartyForm = ref({
    accountIdentifier: '',
    displayName: '',
})

const usernameForm = ref({
    username: '',
})

const twoFactorModalTitle = computed(() => twoFactorModalType.value === 'setup' ? t('security.two_factor_bind') : t('security.two_factor_reset'))
const thirdPartyModalTitle = computed(() => thirdPartyProvider.value === 'coinbase_wallet' ? t('security.bind_coinbase_wallet') : t('security.bind_telegram_account'))
const thirdPartyIdentifierLabel = computed(() => thirdPartyProvider.value === 'coinbase_wallet' ? t('security.coinbase_identifier') : t('security.telegram_identifier'))
const thirdPartyIdentifierPlaceholder = computed(() => thirdPartyProvider.value === 'coinbase_wallet' ? t('security.coinbase_identifier_placeholder') : t('security.telegram_identifier_placeholder'))
const isTwoFactorSubmitDisabled = computed(() => {
    if (twoFactorLoading.value) return true
    if (twoFactorModalType.value === 'setup') {
        return twoFactorForm.value.totpCode.trim().length !== 6
    }
    return !twoFactorForm.value.emailCode.trim()
})

// Determining if trading password is set using fundsVerified (assumed)
const isTradePasswordSet = computed(() => {
    return securitySetting.value?.fundsVerified === 1 || securitySetting.value?.transactionStatus === 1
})

const thirdPartyPolicy = computed(() => thirdPartyStatus.value?.policy ?? twoFactorStatus.value?.third_party_bindings ?? {
    coinbase_wallet_enabled: false,
    telegram_account_enabled: false,
})

const coinbaseEnabled = computed(() => thirdPartyPolicy.value.coinbase_wallet_enabled)
const telegramEnabled = computed(() => thirdPartyPolicy.value.telegram_account_enabled)
const coinbaseBinding = computed(() => findThirdPartyBinding('coinbase_wallet'))
const telegramBinding = computed(() => findThirdPartyBinding('telegram_account'))

const fetchSecuritySetting = async () => {
    try {
        const [profileRes, twoFactorRes, thirdPartyRes]: any[] = await Promise.all([
            getSecuritySetting(),
            getTwoFactorStatus(),
            getThirdPartyBindings(),
        ])
        if (profileRes.code === 0 || profileRes.code === 200) {
            securitySetting.value = profileRes.data
        }
        if (twoFactorRes.code === 0 || twoFactorRes.code === 200) {
            twoFactorStatus.value = twoFactorRes.data
        }
        if (thirdPartyRes.code === 0 || thirdPartyRes.code === 200) {
            thirdPartyStatus.value = thirdPartyRes.data
        }
    } catch (e) {
        console.error('Failed to load security settings', e)
    } finally {
        initLoading.value = false
    }
}

onMounted(() => {
    fetchSecuritySetting()
})

const startCountdown = () => {
    countdown.value = 60
    const timer = setInterval(() => {
        countdown.value--
        if (countdown.value <= 0) {
            clearInterval(timer)
        }
    }, 1000)
}

const sendCode = async () => {
    if (!securitySetting.value?.email) {
        toast.error(t('security.no_email'))
        return
    }
    codeLoading.value = true
    try {
        await sendTransactionPasswordResetCode()
        toast.success(t('security.code_sent') + ' ' + securitySetting.value.email)
        startCountdown()
    } catch (e: any) {
        console.error(e)
    } finally {
        codeLoading.value = false
    }
}

function openPasswordModal(type: 'trade-set' | 'trade-update' | 'trade-reset' | 'login') {
    modalType.value = type
    modalTitle.value = type === 'trade-set' ? t('security.set_trade') :
                       type === 'trade-update' ? t('security.update_trade') :
                       type === 'trade-reset' ? t('security.reset_trade') : t('security.change_login')
    form.value = { oldPassword: '', loginPassword: '', newPassword: '', confirmPassword: '', code: '' }
    showModal.value = true
}

function openUsernameModal() {
    usernameForm.value.username = securitySetting.value?.username || ''
    showUsernameModal.value = true
}

function closeUsernameModal() {
    showUsernameModal.value = false
    usernameForm.value.username = ''
}

async function saveUsername() {
    const username = usernameForm.value.username.trim()
    if (!username) return
    usernameLoading.value = true
    try {
        const res = await updateUsername(username)
        if (res.code === 0 || res.code === 200) {
            securitySetting.value = {
                ...(securitySetting.value || {}),
                username: res.data.username,
            }
            await userStore.loadProfile().catch((error) => console.error('Failed to refresh user profile', error))
            closeUsernameModal()
            toast.success(t('security.username_update_success'))
        } else {
            toast.error(res.message || t('security.username_update_failed'))
        }
    } catch (e: any) {
        console.error(e)
        if (!e.response) {
            toast.error(e.message || t('security.username_update_failed'))
        }
    } finally {
        usernameLoading.value = false
    }
}

async function handleSave() {
    if (form.value.newPassword !== form.value.confirmPassword) {
        toast.error(t('security.no_match'))
        return
    }

    submitLoading.value = true
    try {
        let res: any

        if (modalType.value === 'login') {
            res = await changeLoginPassword(form.value.oldPassword, form.value.newPassword)
        } else if (modalType.value === 'trade-set') {
            res = await setTransactionPassword(form.value.newPassword, form.value.loginPassword)
        } else if (modalType.value === 'trade-update') {
            res = await updateTransactionPassword(form.value.oldPassword, form.value.newPassword)
        } else if (modalType.value === 'trade-reset') {
            res = await resetTransactionPassword(form.value.newPassword, form.value.code)
        }

        if (res?.code === 0 || res?.code === 200) {
            toast.success(res.message || t('security.success'))
            showModal.value = false
            fetchSecuritySetting() // Refresh settings
        } else {
            toast.error(res?.message || t('security.failed'))
        }
    } catch (e: any) {
        console.error(e)
        if (!e.response) {
            toast.error(e.message || t('security.failed'))
        }
    } finally {
        submitLoading.value = false
    }
}

async function startTwoFactorSetup() {
    twoFactorLoading.value = true
    twoFactorModalType.value = 'setup'
    twoFactorForm.value = { totpCode: '', emailCode: '' }
    resetTwoFactorSetupState()
    try {
        const res = await setupTwoFactor()
        if (res.code === 0 || res.code === 200) {
            twoFactorSetup.value = res.data
            await renderTwoFactorQrCode(res.data)
            showTwoFactorModal.value = true
        }
    } catch (e: any) {
        console.error(e)
        if (!e.response) toast.error(e.message || t('security.two_factor_start_failed'))
    } finally {
        twoFactorLoading.value = false
    }
}

function openTwoFactorReset() {
    twoFactorModalType.value = 'reset'
    twoFactorForm.value = { totpCode: '', emailCode: '' }
    resetTwoFactorSetupState()
    showTwoFactorModal.value = true
}

async function saveTwoFactorModal() {
    if (isTwoFactorSubmitDisabled.value) return
    twoFactorLoading.value = true
    try {
        if (twoFactorModalType.value === 'setup') {
            await confirmTwoFactor(twoFactorForm.value.totpCode)
            toast.success(t('security.two_factor_bound_success'))
        } else {
            await resetTwoFactor(twoFactorForm.value.emailCode)
            toast.success(t('security.two_factor_reset_success'))
        }
        closeTwoFactorModal()
        await fetchSecuritySetting()
    } catch (e: any) {
        console.error(e)
        if (!e.response) toast.error(e.message || t('security.two_factor_operation_failed'))
    } finally {
        twoFactorLoading.value = false
    }
}

async function renderTwoFactorQrCode(setup: TwoFactorSetup) {
    twoFactorQrCodeUrl.value = ''
    twoFactorQrCodeError.value = ''
    try {
        twoFactorQrCodeUrl.value = await toDataURL(setup.otpauth_uri, {
            errorCorrectionLevel: 'M',
            margin: 1,
            width: 192,
            color: {
                dark: '#020617',
                light: '#ffffff',
            },
        })
    } catch (error) {
        console.error('Failed to render 2FA QR code', error)
        twoFactorQrCodeError.value = t('security.two_factor_qr_failed')
    }
}

function resetTwoFactorSetupState() {
    twoFactorSetup.value = null
    twoFactorQrCodeUrl.value = ''
    twoFactorQrCodeError.value = ''
}

function closeTwoFactorModal() {
    showTwoFactorModal.value = false
    twoFactorForm.value = { totpCode: '', emailCode: '' }
    resetTwoFactorSetupState()
}

async function sendTwoFactorCode() {
    twoFactorLoading.value = true
    try {
        await sendTwoFactorResetCode()
        toast.success(t('security.code_sent'))
    } catch (e: any) {
        console.error(e)
        if (!e.response) toast.error(e.message || t('security.send_code_failed'))
    } finally {
        twoFactorLoading.value = false
    }
}

async function toggleLoginTwoFactor() {
    if (!twoFactorStatus.value?.can_toggle_login_2fa) return
    twoFactorLoading.value = true
    try {
        await updateLoginTwoFactor(!twoFactorStatus.value.login_2fa_enabled)
        await fetchSecuritySetting()
        toast.success(t('security.success'))
    } catch (e: any) {
        console.error(e)
        if (!e.response) toast.error(e.message || t('security.login_2fa_update_failed'))
    } finally {
        twoFactorLoading.value = false
    }
}

function findThirdPartyBinding(provider: ThirdPartyProvider) {
    return thirdPartyStatus.value?.bindings.find((binding) => binding.provider === provider && binding.status === 'bound') ?? null
}

function thirdPartyBindingLabel(binding: ThirdPartyBinding) {
    return binding.display_name || binding.account_identifier
}

function isThirdPartyProviderEnabled(provider: ThirdPartyProvider) {
    return provider === 'coinbase_wallet' ? coinbaseEnabled.value : telegramEnabled.value
}

function openThirdPartyModal(provider: ThirdPartyProvider) {
    if (!isThirdPartyProviderEnabled(provider)) {
        toast.error(t('security.third_party_disabled'))
        return
    }
    thirdPartyProvider.value = provider
    thirdPartyForm.value = { accountIdentifier: '', displayName: '' }
    showThirdPartyModal.value = true
}

function closeThirdPartyModal() {
    showThirdPartyModal.value = false
    thirdPartyForm.value = { accountIdentifier: '', displayName: '' }
}

async function saveThirdPartyBinding() {
    const accountIdentifier = thirdPartyForm.value.accountIdentifier.trim()
    if (!accountIdentifier) return
    thirdPartyLoading.value = true
    try {
        const res = await bindThirdPartyAccount(
            thirdPartyProvider.value,
            accountIdentifier,
            thirdPartyForm.value.displayName.trim() || undefined,
        )
        if (res.code === 0 || res.code === 200) {
            thirdPartyStatus.value = res.data
            closeThirdPartyModal()
            toast.success(t('security.third_party_bind_success'))
        }
    } catch (e: any) {
        console.error(e)
        if (!e.response) toast.error(e.message || t('security.failed'))
    } finally {
        thirdPartyLoading.value = false
    }
}
</script>
