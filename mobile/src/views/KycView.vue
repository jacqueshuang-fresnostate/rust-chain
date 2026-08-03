<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Building2, Camera, CheckCircle2, IdCard, LockKeyhole, UserRound } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchCountries, type CountryOption } from '@/api/auth'
import { fetchKycStatus, submitKycApplication, type KycCountryDocumentRule, type KycStatus } from '@/api/user'
import { formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'

type SubmissionType = 'personal' | 'enterprise'
type UploadKind = 'front' | 'back' | 'handheld'

const session = useSessionStore()
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const kyc = ref<KycStatus | null>(null)
const countries = ref<CountryOption[]>([])
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')
const submissionType = ref<SubmissionType>('personal')
const form = ref({
  realName: '',
  enterpriseName: '',
  businessRegistrationNumber: '',
  country: '',
  idNumber: '',
  documentType: 'identity_card',
})
const documents = ref<Record<UploadKind, File | null>>({ front: null, back: null, handheld: null })
const previews = ref<Record<UploadKind, string>>({ front: '', back: '', handheld: '' })
const frontInput = ref<HTMLInputElement | null>(null)
const backInput = ref<HTMLInputElement | null>(null)
const handheldInput = ref<HTMLInputElement | null>(null)

const latest = computed(() => kyc.value?.latestSubmission)
const isLocked = computed(() => latest.value?.status === 'pending' || latest.value?.status === 'approved')
const maxDocumentSize = computed(() => kyc.value?.config.maxDocumentSizeBytes || 5 * 1024 * 1024)
const maxDocumentSizeMb = computed(() => Math.max(1, Math.round(maxDocumentSize.value / 1024 / 1024)))
const configuredCountries = computed(() => {
  const rules = kyc.value?.config.countryDocumentTypes.map((rule) => rule.country).filter(Boolean) || []
  return rules.length ? uniqueValues(rules) : uniqueValues(kyc.value?.config.allowedCountries || [])
})
const countryOptions = computed(() => {
  const configured = configuredCountries.value
  if (!configured.length) return countries.value.map((country) => ({ value: country.name || country.code, label: countryLabel(country) }))
  return configured.map((value) => {
    const country = countries.value.find((item) => matchesCountry(value, item))
    return { value, label: country ? countryLabel(country) : value }
  })
})
const selectedRule = computed<KycCountryDocumentRule | undefined>(() => kyc.value?.config.countryDocumentTypes.find((rule) => rule.country.toLowerCase() === form.value.country.toLowerCase()))
const documentTypes = computed(() => {
  const configured = selectedRule.value?.documentTypes || []
  return configured.length ? uniqueValues(configured) : ['identity_card', 'passport', 'driver_license', 'residence_permit']
})
const requiresHandheld = computed(() => selectedRule.value?.handheldDocumentTypes.includes(form.value.documentType) || false)
const uploadItems = computed(() => [
  { kind: 'front' as const, label: t('kyc.front') },
  { kind: 'back' as const, label: t('kyc.back') },
  { kind: 'handheld' as const, label: t('kyc.handheld') },
])

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: route.fullPath } })
}

function uniqueValues(values: string[]): string[] {
  return values.map((value) => value.trim()).filter(Boolean).filter((value, index, source) => source.findIndex((item) => item.toLowerCase() === value.toLowerCase()) === index)
}

function matchesCountry(value: string, country: CountryOption): boolean {
  const normalized = value.trim().toLowerCase()
  return normalized === country.code.toLowerCase() || normalized === country.name.toLowerCase()
}

function countryLabel(country: CountryOption): string {
  return country.name && country.name !== country.code ? `${country.name} (${country.code})` : country.code
}

function documentLabel(value: string): string {
  return {
    identity_card: t('kyc.identityCard'),
    passport: t('kyc.passport'),
    driver_license: t('kyc.driverLicense'),
    residence_permit: t('kyc.residencePermit'),
  }[value] || value
}

function statusLabel(status?: string): string {
  if (status === 'approved') return t('kyc.approved')
  if (status === 'rejected') return t('kyc.rejected')
  return t('kyc.pending')
}

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextKyc, nextCountries] = await Promise.all([fetchKycStatus(), fetchCountries()])
    kyc.value = nextKyc
    countries.value = nextCountries
    if (nextKyc.latestSubmission?.status === 'rejected') {
      form.value.realName = nextKyc.latestSubmission.realName
      form.value.country = nextKyc.latestSubmission.country
      form.value.idNumber = nextKyc.latestSubmission.idNumber
      form.value.documentType = nextKyc.latestSubmission.documentType
      submissionType.value = nextKyc.latestSubmission.submissionType
      form.value.enterpriseName = nextKyc.latestSubmission.enterpriseName || ''
      form.value.businessRegistrationNumber = nextKyc.latestSubmission.businessRegistrationNumber || ''
    }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('kyc.loadFailed'))
  } finally {
    loading.value = false
  }
}

function inputFor(kind: UploadKind): HTMLInputElement | null {
  return kind === 'front' ? frontInput.value : kind === 'back' ? backInput.value : handheldInput.value
}

function chooseFile(kind: UploadKind): void {
  inputFor(kind)?.click()
}

function handleFile(event: Event, kind: UploadKind): void {
  const file = (event.target as HTMLInputElement).files?.[0]
  if (!file) return
  if (file.size > maxDocumentSize.value) {
    error.value = t('kyc.fileTooLarge', { size: maxDocumentSizeMb.value })
    return
  }
  if (!file.type.startsWith('image/')) {
    error.value = t('kyc.imageOnly')
    return
  }
  documents.value[kind] = file
  previews.value[kind] = URL.createObjectURL(file)
  error.value = ''
}

function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result || ''))
    reader.onerror = () => reject(reader.error || new Error(t('kyc.fileReadFailed')))
    reader.readAsDataURL(file)
  })
}

async function submit(): Promise<void> {
  error.value = ''
  success.value = ''
  if (!form.value.realName.trim() || !form.value.country || !form.value.idNumber.trim()) {
    error.value = t('kyc.requiredFields')
    return
  }
  if (submissionType.value === 'enterprise' && (!form.value.enterpriseName.trim() || !form.value.businessRegistrationNumber.trim())) {
    error.value = t('kyc.enterpriseFields')
    return
  }
  if (!documents.value.front || !documents.value.back) {
    error.value = t('kyc.frontBackRequired')
    return
  }
  if (requiresHandheld.value && !documents.value.handheld) {
    error.value = t('kyc.handheldRequired')
    return
  }
  submitting.value = true
  try {
    const [front, back, handheld] = await Promise.all([
      fileToDataUrl(documents.value.front),
      fileToDataUrl(documents.value.back),
      documents.value.handheld ? fileToDataUrl(documents.value.handheld) : Promise.resolve(undefined),
    ])
    await submitKycApplication({
      realName: form.value.realName,
      submissionType: submissionType.value,
      enterpriseName: submissionType.value === 'enterprise' ? form.value.enterpriseName : undefined,
      businessRegistrationNumber: submissionType.value === 'enterprise' ? form.value.businessRegistrationNumber : undefined,
      country: form.value.country,
      idNumber: form.value.idNumber,
      documentType: form.value.documentType,
      documentFrontImage: front,
      documentBackImage: back,
      documentHandheldImage: handheld,
    })
    success.value = t('kyc.submitted')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('kyc.submitFailed'))
  } finally {
    submitting.value = false
  }
}

watch(countryOptions, (options) => {
  if (!options.some((option) => option.value === form.value.country)) form.value.country = options[0]?.value || ''
}, { immediate: true })
watch(documentTypes, (types) => {
  if (!types.includes(form.value.documentType)) form.value.documentType = types[0] || ''
}, { immediate: true })
onMounted(() => { void load() })
</script>

<template>
  <main
    class="page page--plain pencil-page kyc-page"
    data-pencil-source="Raoes wJT9Y"
  >
    <PageHeader
      :back="true"
      :pencil="true"
      :title="t('kyc.title')"
    />
    <div class="pencil-content kyc-content">
      <section v-if="!session.isAuthenticated" class="account-login-state">
        <span class="account-login-state__icon"><LockKeyhole :size="20" /></span>
        <div><strong>{{ t('common.loginRequiredTitle') }}</strong><p>{{ t('kyc.loginDescription') }}</p></div>
        <button class="pencil-primary" type="button" @click="openLogin">{{ t('common.loginNow') }}</button>
      </section>
      <template v-else>
        <p v-if="error" class="pencil-message pencil-message--error kyc-feedback" role="alert">{{ error }}</p>
        <p v-else-if="success" class="pencil-message pencil-message--success kyc-feedback" role="status">{{ success }}</p>
        <p v-if="loading" class="kyc-loading" role="status">{{ t('kyc.loading') }}</p>
        <template v-else-if="kyc">
          <section class="kyc-status" :class="latest ? `kyc-status--${latest.status}` : ''" role="status">
            <span><CheckCircle2 v-if="latest" :size="20" /><IdCard v-else :size="20" /></span>
            <div>
              <strong>{{ latest ? statusLabel(latest.status) : t('kyc.subjectInfo') }}</strong>
              <p>{{ latest ? t('kyc.submittedAt', { type: latest.submissionType === 'enterprise' ? t('kyc.enterprise') : t('kyc.personal'), time: formatDateTime(latest.submittedAt) }) : t('kyc.fileHint', { size: maxDocumentSizeMb }) }}</p>
              <small v-if="latest?.reviewReason">{{ latest.reviewReason }}</small>
            </div>
          </section>
          <p v-if="!kyc.config.enabled" class="kyc-note" role="status">{{ t('kyc.disabled') }}</p>
          <form v-else-if="!isLocked" class="kyc-form" :aria-busy="submitting" @submit.prevent="submit">
            <div class="kyc-type" role="group" :aria-label="t('kyc.subjectInfo')">
              <button type="button" :aria-pressed="submissionType === 'personal'" :class="{ 'is-active': submissionType === 'personal' }" @click="submissionType = 'personal'">
                <UserRound :size="16" />{{ t('kyc.personal') }}
              </button>
              <button type="button" :aria-pressed="submissionType === 'enterprise'" :class="{ 'is-active': submissionType === 'enterprise' }" @click="submissionType = 'enterprise'">
                <Building2 :size="16" />{{ t('kyc.enterprise') }}
              </button>
            </div>

            <section class="kyc-fields">
              <label class="kyc-field">
                <span>{{ t('kyc.country') }}</span>
                <select v-model="form.country">
                  <option v-for="country in countryOptions" :key="country.value" :value="country.value">{{ country.label }}</option>
                </select>
              </label>
              <label class="kyc-field">
                <span>{{ t('kyc.documentType') }}</span>
                <select v-model="form.documentType">
                  <option v-for="type in documentTypes" :key="type" :value="type">{{ documentLabel(type) }}</option>
                </select>
              </label>
              <label class="kyc-field">
                <span>{{ t('kyc.legalName') }}</span>
                <input v-model="form.realName" :placeholder="t('kyc.legalNamePlaceholder')" />
              </label>
              <template v-if="submissionType === 'enterprise'">
                <label class="kyc-field">
                  <span>{{ t('kyc.enterpriseName') }}</span>
                  <input v-model="form.enterpriseName" :placeholder="t('kyc.enterpriseNamePlaceholder')" />
                </label>
                <label class="kyc-field">
                  <span>{{ t('kyc.registrationNumber') }}</span>
                  <input v-model="form.businessRegistrationNumber" :placeholder="t('kyc.registrationNumberPlaceholder')" />
                </label>
              </template>
              <label class="kyc-field">
                <span>{{ t('kyc.documentNumber') }}</span>
                <input v-model="form.idNumber" :placeholder="t('kyc.documentNumberPlaceholder')" />
              </label>
            </section>

            <section class="document-section">
              <div class="document-grid">
                <button
                  v-for="item in uploadItems"
                  :key="item.kind"
                  class="upload-tile"
                  :class="{ 'upload-tile--selected': documents[item.kind] }"
                  type="button"
                  @click="chooseFile(item.kind)"
                >
                  <img v-if="previews[item.kind]" :src="previews[item.kind]" :alt="item.label" />
                  <template v-else><Camera :size="18" /><span>{{ item.label }}</span></template>
                  <span v-if="previews[item.kind]" class="upload-tile__status">{{ item.label }}</span>
                </button>
              </div>
              <input ref="frontInput" class="hidden-input" type="file" accept="image/*" @change="handleFile($event, 'front')" />
              <input ref="backInput" class="hidden-input" type="file" accept="image/*" @change="handleFile($event, 'back')" />
              <input ref="handheldInput" class="hidden-input" type="file" accept="image/*" @change="handleFile($event, 'handheld')" />
            </section>

            <p class="kyc-note">{{ t('kyc.fileHint', { size: maxDocumentSizeMb }) }}</p>
            <button class="pencil-primary kyc-submit" type="submit" :disabled="submitting">
              {{ submitting ? t('common.submitting') : t('kyc.submit') }}
            </button>
          </form>
          <p v-else class="kyc-note">{{ latest?.status === 'approved' ? t('kyc.completedLevel', { level: latest.targetKycLevel }) : t('kyc.reviewPending') }}</p>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.page.pencil-page.kyc-page { background: var(--page); background-image: none; min-height: 100dvh; }
.kyc-content { display: flex; flex-direction: column; gap: 12px; padding-bottom: calc(20px + env(safe-area-inset-bottom)); padding-top: 6px; }
.account-login-state { align-items: center; display: grid; gap: 12px; grid-template-columns: 44px minmax(0, 1fr) auto; min-height: 76px; }
.account-login-state__icon { align-items: center; background: var(--accent-soft); border-radius: 50%; color: var(--positive); display: inline-flex; height: 44px; justify-content: center; width: 44px; }
.account-login-state div { display: grid; gap: 3px; min-width: 0; }
.account-login-state strong { color: var(--ink); font-size: 14px; }
.account-login-state p { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
.account-login-state .pencil-primary { min-height: 44px; padding-inline: 16px; }
.kyc-feedback { margin: 0; }
.kyc-loading { color: var(--muted); font-size: 11px; margin: 0; min-height: 44px; padding-block: 14px; }
.kyc-status { align-items: center; display: flex; gap: 12px; min-height: 56px; padding: 4px 0 8px; }
.kyc-status > span { align-items: center; background: var(--accent-soft); border-radius: 50%; color: var(--positive); display: inline-flex; flex: 0 0 44px; height: 44px; justify-content: center; width: 44px; }
.kyc-status--rejected > span { background: var(--negative-soft); color: var(--negative); }
.kyc-status div { display: grid; gap: 4px; min-width: 0; }
.kyc-status strong { color: var(--ink); font-size: 15px; font-weight: 700; line-height: 21px; }
.kyc-status p,.kyc-status small { color: var(--muted); font-size: 11px; line-height: 15px; margin: 0; overflow-wrap: anywhere; }
.kyc-form { display: flex; flex-direction: column; gap: 12px; }
.kyc-type { display: flex; gap: 8px; min-height: 34px; }
.kyc-type button { align-items: center; background: var(--surface-2); border-radius: 17px; color: var(--muted); display: flex; font-size: 11px; gap: 5px; justify-content: center; min-height: 34px; padding: 0 12px; }
.kyc-type button.is-active { background: var(--accent-soft); color: var(--positive); }
.kyc-fields { display: grid; gap: 12px; }
.kyc-field { border: 1px solid transparent; border-radius: 8px; display: grid; gap: 3px; min-height: 48px; padding: 4px 0; }
.kyc-field:focus-within { border-color: var(--positive); box-shadow: 0 0 0 2px var(--focus-ring); }
.kyc-field > span { color: var(--muted); font-size: 11px; font-weight: 500; line-height: 15px; }
.kyc-field input,
.kyc-field select { appearance: none; background: transparent; border: 0; color: var(--ink); font-size: 14px; font-weight: 600; line-height: 20px; min-height: 25px; outline: 0; padding: 0; width: 100%; }
.document-grid { display: grid; gap: 8px; grid-template-columns: repeat(3, minmax(0, 1fr)); }
.upload-tile { align-items: center; background: transparent; border: 1px solid var(--line); border-radius: 10px; color: var(--muted); display: flex; flex-direction: column; font-size: 11px; gap: 6px; height: 72px; justify-content: center; min-height: 72px; overflow: hidden; padding: 5px; position: relative; }
.upload-tile:focus-visible { box-shadow: 0 0 0 2px var(--focus-ring); outline: 0; }
.upload-tile--selected { border-color: var(--positive); color: var(--positive); }
.upload-tile img { height: 100%; inset: 0; object-fit: cover; position: absolute; width: 100%; }
.upload-tile__status { background: var(--surface-elevated); bottom: 0; color: var(--positive); font-size: 9px; font-weight: 600; inset-inline: 0; overflow: hidden; padding: 3px; position: absolute; text-align: center; text-overflow: ellipsis; white-space: nowrap; z-index: 1; }
.hidden-input { display: none; }
.kyc-note { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
.kyc-submit { font-size: 15px; height: 48px; min-height: 48px; width: 100%; }
.kyc-page button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
@media (max-width: 340px) {
  .kyc-content { padding-inline: 16px; }
  .account-login-state { align-items: start; grid-template-columns: 44px minmax(0, 1fr); }
  .account-login-state .pencil-primary { grid-column: 2; justify-self: start; }
  .document-grid { gap: 6px; }
}
</style>
