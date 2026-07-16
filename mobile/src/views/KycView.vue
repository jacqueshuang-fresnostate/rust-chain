<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Building2, Camera, CheckCircle2, FileBadge, UserRound } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchCountries, type CountryOption } from '@/api/auth'
import { fetchKycStatus, submitKycApplication, type KycCountryDocumentRule, type KycStatus } from '@/api/user'
import { formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'

type SubmissionType = 'personal' | 'enterprise'
type UploadKind = 'front' | 'back' | 'handheld'

const session = useSessionStore()
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
  ...(requiresHandheld.value ? [{ kind: 'handheld' as const, label: t('kyc.handheld') }] : []),
])

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
      requiresHandheld.value && documents.value.handheld ? fileToDataUrl(documents.value.handheld) : Promise.resolve(undefined),
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
  <main class="page page--plain kyc-page">
    <PageHeader :title="t('kyc.title')" />
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('kyc.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="loading" class="empty-state">{{ t('kyc.loading') }}</p>
        <template v-else-if="kyc">
          <section v-if="latest" class="kyc-status" :class="`kyc-status--${latest.status}`"><CheckCircle2 :size="22" /><div><strong>{{ statusLabel(latest.status) }}</strong><p>{{ t('kyc.submittedAt', { type: latest.submissionType === 'enterprise' ? t('kyc.enterprise') : t('kyc.personal'), time: formatDateTime(latest.submittedAt) }) }}</p><small v-if="latest.reviewReason">{{ latest.reviewReason }}</small></div></section>
          <p v-if="!kyc.config.enabled" class="surface-note">{{ t('kyc.disabled') }}</p>
          <form v-else-if="!isLocked" class="kyc-form" @submit.prevent="submit">
            <div class="kyc-type"><button type="button" :class="{ 'is-active': submissionType === 'personal' }" @click="submissionType = 'personal'"><UserRound :size="19" />{{ t('kyc.personal') }}</button><button type="button" :class="{ 'is-active': submissionType === 'enterprise' }" @click="submissionType = 'enterprise'"><Building2 :size="19" />{{ t('kyc.enterprise') }}</button></div>
            <section class="form-section"><h2>{{ t('kyc.subjectInfo') }}</h2><label><span>{{ t('kyc.legalName') }}</span><input v-model="form.realName" class="input" :placeholder="t('kyc.legalNamePlaceholder')" /></label><template v-if="submissionType === 'enterprise'"><label><span>{{ t('kyc.enterpriseName') }}</span><input v-model="form.enterpriseName" class="input" :placeholder="t('kyc.enterpriseNamePlaceholder')" /></label><label><span>{{ t('kyc.registrationNumber') }}</span><input v-model="form.businessRegistrationNumber" class="input" :placeholder="t('kyc.registrationNumberPlaceholder')" /></label></template><label><span>{{ t('kyc.country') }}</span><select v-model="form.country"><option v-for="country in countryOptions" :key="country.value" :value="country.value">{{ country.label }}</option></select></label><label><span>{{ t('kyc.documentType') }}</span><select v-model="form.documentType"><option v-for="type in documentTypes" :key="type" :value="type">{{ documentLabel(type) }}</option></select></label><label><span>{{ t('kyc.documentNumber') }}</span><input v-model="form.idNumber" class="input" :placeholder="t('kyc.documentNumberPlaceholder')" /></label></section>
            <section class="form-section"><h2>{{ t('kyc.documents') }}</h2><p>{{ t('kyc.fileHint', { size: maxDocumentSizeMb }) }}</p><div class="document-grid"><button v-for="item in uploadItems" :key="item.kind" class="upload-tile" type="button" @click="chooseFile(item.kind)"><img v-if="previews[item.kind]" :src="previews[item.kind]" :alt="item.label" /><template v-else><Camera :size="23" /><span>{{ item.label }}</span></template></button></div><input ref="frontInput" class="hidden-input" type="file" accept="image/*" @change="handleFile($event, 'front')" /><input ref="backInput" class="hidden-input" type="file" accept="image/*" @change="handleFile($event, 'back')" /><input ref="handheldInput" class="hidden-input" type="file" accept="image/*" @change="handleFile($event, 'handheld')" /></section>
            <p v-if="success" class="success-message">{{ success }}</p><button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('common.submitting') : t('kyc.submit') }}</button>
          </form>
          <p v-else class="surface-note">{{ latest?.status === 'approved' ? t('kyc.completedLevel', { level: latest.targetKycLevel }) : t('kyc.reviewPending') }}</p>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.kyc-page .page-content { display: grid; gap: 18px; padding-bottom: 42px; padding-top: 16px; }.kyc-status { align-items: flex-start; border: 1px solid var(--line); border-radius: var(--radius); display: flex; gap: 11px; padding: 14px; }.kyc-status--approved { background: var(--positive-soft); border-color: #caeddb; color: var(--positive); }.kyc-status--pending { background: #fff8e6; border-color: #f2dfaf; color: #8a5a00; }.kyc-status--rejected { background: var(--negative-soft); border-color: #f5cbd5; color: var(--negative); }.kyc-status div { display: grid; gap: 4px; }.kyc-status strong { color: var(--ink); font-size: 16px; }.kyc-status p,.kyc-status small { color: var(--muted-strong); font-size: 12px; line-height: 1.4; margin: 0; }.kyc-form { display: grid; gap: 22px; }.kyc-type { background: var(--soft); border-radius: 25px; display: grid; grid-template-columns: 1fr 1fr; padding: 4px; }.kyc-type button { align-items: center; background: transparent; border-radius: 20px; color: var(--muted); display: flex; font-size: 14px; font-weight: 700; gap: 6px; justify-content: center; min-height: 39px; }.kyc-type .is-active { background: white; box-shadow: 0 1px 4px rgb(15 23 42 / 10%); color: var(--ink); }.form-section { border-top: 1px solid var(--line); display: grid; gap: 13px; padding-top: 18px; }.form-section h2 { font-size: 18px; margin: 0; }.form-section > p { color: var(--muted); font-size: 12px; margin: -5px 0 0; }.form-section label { display: grid; gap: 7px; }.form-section label > span { color: var(--muted); font-size: 13px; }.form-section select { appearance: none; background: var(--soft); border: 1px solid transparent; border-radius: var(--radius); color: var(--ink); font: inherit; min-height: 48px; outline: 0; padding: 0 13px; }.form-section select:focus { background: white; border-color: var(--accent); box-shadow: 0 0 0 3px rgb(22 124 103 / 9%); }.document-grid { display: grid; gap: 10px; grid-template-columns: repeat(3, 1fr); }.upload-tile { align-items: center; aspect-ratio: .9; background: var(--soft); border: 1px dashed #cbd2d8; border-radius: var(--radius); color: var(--muted); display: flex; flex-direction: column; font-size: 12px; gap: 8px; justify-content: center; overflow: hidden; padding: 6px; }.upload-tile img { height: 100%; object-fit: cover; width: 100%; }.hidden-input { display: none; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: -8px 0 0; }
</style>
