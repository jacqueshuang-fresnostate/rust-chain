<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Building2, Camera, Check, CheckCircle2, ChevronDown, IdCard, LockKeyhole, Search, UserRound, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchCountries, type CountryOption } from '@/api/auth'
import { fetchKycStatus, submitKycApplication, type KycCountryDocumentRule, type KycStatus } from '@/api/user'
import { formatDateTime } from '@/core/format'
import { filterCountryOptions, matchesCountryIdentity } from '@/core/countrySearch'
import { filterDocumentTypeOptions, type KycDocumentTypeSearchOption } from '@/core/kycDocumentSearch'
import { useModalDialog } from '@/core/modalDialog'
import { useSessionStore } from '@/stores/session'

type SubmissionType = 'personal' | 'enterprise'
type UploadKind = 'front' | 'back' | 'handheld'
type KycCountryOption = CountryOption & {
  value: string
  label: string
  localizedLabel: string
  searchAliases: string[]
}

const session = useSessionStore()
const route = useRoute()
const router = useRouter()
const { locale, t } = useI18n()
const kyc = ref<KycStatus | null>(null)
const countries = ref<CountryOption[]>([])
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const countryDirectoryError = ref('')
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
const countryPickerOpen = ref(false)
const countrySearch = ref('')
const countryPickerDialog = ref<HTMLElement | null>(null)
const countryPickerTrigger = ref<HTMLButtonElement | null>(null)
const documentTypePickerOpen = ref(false)
const documentTypeSearch = ref('')
const documentTypePickerDialog = ref<HTMLElement | null>(null)
const documentTypePickerTrigger = ref<HTMLButtonElement | null>(null)
const {
  trapFocus: trapCountryPickerFocus,
  setReturnFocus: setCountryPickerReturnFocus,
} = useModalDialog(countryPickerOpen, countryPickerDialog, '[data-country-search]')
const {
  trapFocus: trapDocumentTypePickerFocus,
  setReturnFocus: setDocumentTypePickerReturnFocus,
} = useModalDialog(documentTypePickerOpen, documentTypePickerDialog, '[data-document-type-search]')

const latest = computed(() => kyc.value?.latestSubmission)
const isLocked = computed(() => latest.value?.status === 'pending' || latest.value?.status === 'approved')
const maxDocumentSize = computed(() => kyc.value?.config.maxDocumentSizeBytes || 5 * 1024 * 1024)
const maxDocumentSizeMb = computed(() => Math.max(1, Math.round(maxDocumentSize.value / 1024 / 1024)))
const configuredCountries = computed(() => {
  const rules = kyc.value?.config.countryDocumentTypes.map((rule) => rule.country).filter(Boolean) || []
  return rules.length ? uniqueValues(rules) : uniqueValues(kyc.value?.config.allowedCountries || [])
})
const regionNameResolvers = computed<Intl.DisplayNames[]>(() => uniqueValues([
  locale.value,
  'en',
  'zh-CN',
]).flatMap((localeName) => {
  try {
    return [new Intl.DisplayNames([localeName], { type: 'region' })]
  } catch {
    return []
  }
}))
const countryOptions = computed<KycCountryOption[]>(() => {
  const configured = configuredCountries.value
  if (!configured.length) {
    return countries.value.map((country) => kycCountryOption(country.name || country.code, country))
  }
  return configured.map((value) => {
    const country = countries.value.find((item) => matchesCountry(value, item))
    return kycCountryOption(value, country)
  })
})
const filteredCountryOptions = computed(() => filterCountryOptions(
  countryOptions.value,
  countrySearch.value,
  (country) => country.localizedLabel,
))
const selectedCountryOption = computed(() => countryOptions.value.find(
  (country) => country.value.toLowerCase() === form.value.country.toLowerCase(),
))
const selectedCountryLabel = computed(() => selectedCountryOption.value?.label || t('kyc.selectCountry'))
const selectedRule = computed<KycCountryDocumentRule | undefined>(() => kyc.value?.config.countryDocumentTypes.find((rule) => rule.country.toLowerCase() === form.value.country.toLowerCase()))
const documentTypes = computed(() => {
  const configured = selectedRule.value?.documentTypes || []
  return configured.length ? uniqueValues(configured) : ['identity_card', 'passport', 'driver_license', 'residence_permit']
})
const documentTypeOptions = computed<KycDocumentTypeSearchOption[]>(() => documentTypes.value.map((value) => ({
  value,
  label: documentLabel(value),
})))
const filteredDocumentTypeOptions = computed(() => filterDocumentTypeOptions(
  documentTypeOptions.value,
  documentTypeSearch.value,
))
const selectedDocumentTypeLabel = computed(() => documentTypeOptions.value.find(
  (option) => option.value.toLowerCase() === form.value.documentType.toLowerCase(),
)?.label || t('kyc.selectDocumentType'))
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
  return matchesCountryIdentity(country, value, localizedCountryNames(country))
}

function localizedCountryNames(country: CountryOption): string[] {
  return uniqueValues(regionNameResolvers.value.flatMap((resolver) => {
    try {
      const name = resolver.of(country.code)
      return name ? [name] : []
    } catch {
      return []
    }
  }))
}

function kycCountryOption(value: string, country?: CountryOption): KycCountryOption {
  const code = country?.code || value
  const name = country?.name || value
  const localizedNames = country ? localizedCountryNames(country) : []
  const localizedLabel = localizedNames[0] || name || code
  return {
    value,
    code,
    name,
    localizedLabel,
    searchAliases: uniqueValues([value, ...localizedNames]),
    label: code && localizedLabel !== code ? `${localizedLabel} (${code})` : localizedLabel,
  }
}

function countrySecondaryLabel(country: KycCountryOption): string {
  return country.name.toLowerCase() !== country.localizedLabel.toLowerCase() ? country.name : ''
}

function isCountrySelected(value: string): boolean {
  return value.trim().toLowerCase() === form.value.country.trim().toLowerCase()
}

function openCountryPicker(): void {
  countrySearch.value = ''
  setCountryPickerReturnFocus(countryPickerTrigger.value)
  countryPickerOpen.value = true
}

function closeCountryPicker(): void {
  countryPickerOpen.value = false
}

function selectCountry(value: string): void {
  const country = countryOptions.value.find((option) => option.value === value)
  if (!country) return
  form.value.country = country.value
  closeCountryPicker()
}

function handleCountryPickerKeydown(event: KeyboardEvent): void {
  trapCountryPickerFocus(event, closeCountryPicker)
}

function isDocumentTypeSelected(value: string): boolean {
  return value.trim().toLowerCase() === form.value.documentType.trim().toLowerCase()
}

function openDocumentTypePicker(): void {
  documentTypeSearch.value = ''
  setDocumentTypePickerReturnFocus(documentTypePickerTrigger.value)
  documentTypePickerOpen.value = true
}

function closeDocumentTypePicker(): void {
  documentTypePickerOpen.value = false
}

function selectDocumentType(value: string): void {
  const option = documentTypeOptions.value.find((item) => item.value === value)
  if (!option) return
  form.value.documentType = option.value
  closeDocumentTypePicker()
}

function handleDocumentTypePickerKeydown(event: KeyboardEvent): void {
  trapDocumentTypePickerFocus(event, closeDocumentTypePicker)
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
  countryDirectoryError.value = ''
  try {
    const [kycResult, countriesResult] = await Promise.allSettled([
      fetchKycStatus(),
      fetchCountries(),
    ])
    if (countriesResult.status === 'fulfilled') {
      countries.value = countriesResult.value
    } else {
      countries.value = []
      countryDirectoryError.value = apiErrorMessage(countriesResult.reason, t('kyc.countryLoadFailed'))
    }
    if (kycResult.status === 'rejected') {
      kyc.value = null
      throw kycResult.reason
    }
    const nextKyc = kycResult.value
    kyc.value = nextKyc
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
  if (!options.some((option) => option.value.toLowerCase() === form.value.country.toLowerCase())) {
    form.value.country = options[0]?.value || ''
  }
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
        <p v-if="countryDirectoryError" class="kyc-note kyc-country-directory-warning" role="status">{{ countryDirectoryError }}</p>
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
              <div class="kyc-field">
                <span>{{ t('kyc.country') }}</span>
                <button
                  id="kyc-country-picker-trigger"
                  ref="countryPickerTrigger"
                  class="kyc-picker-trigger kyc-country-trigger"
                  type="button"
                  aria-haspopup="dialog"
                  :aria-controls="countryPickerOpen ? 'kyc-country-picker' : undefined"
                  :aria-expanded="countryPickerOpen"
                  @click="openCountryPicker"
                >
                  <span>{{ selectedCountryLabel }}</span>
                  <ChevronDown :size="17" aria-hidden="true" />
                </button>
              </div>
              <div class="kyc-field">
                <span>{{ t('kyc.documentType') }}</span>
                <button
                  id="kyc-document-type-picker-trigger"
                  ref="documentTypePickerTrigger"
                  class="kyc-picker-trigger kyc-document-trigger"
                  type="button"
                  aria-haspopup="dialog"
                  :aria-controls="documentTypePickerOpen ? 'kyc-document-type-picker' : undefined"
                  :aria-expanded="documentTypePickerOpen"
                  @click="openDocumentTypePicker"
                >
                  <span>{{ selectedDocumentTypeLabel }}</span>
                  <ChevronDown :size="17" aria-hidden="true" />
                </button>
              </div>
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

    <Teleport to="body">
      <div v-if="countryPickerOpen" class="kyc-picker-mask kyc-country-picker-mask" @click.self="closeCountryPicker">
        <section
          id="kyc-country-picker"
          ref="countryPickerDialog"
          class="kyc-picker-sheet kyc-country-picker-sheet"
          role="dialog"
          aria-modal="true"
          aria-labelledby="kyc-country-picker-title"
          @keydown="handleCountryPickerKeydown"
        >
          <div class="kyc-picker-handle" aria-hidden="true" />
          <header class="kyc-picker-header">
            <h2 id="kyc-country-picker-title">{{ t('kyc.countryPickerTitle') }}</h2>
            <button type="button" :aria-label="t('kyc.countryPickerClose')" @click="closeCountryPicker">
              <X :size="20" aria-hidden="true" />
            </button>
          </header>
          <label class="kyc-picker-search">
            <Search :size="18" aria-hidden="true" />
            <input
              v-model="countrySearch"
              data-country-search
              type="search"
              autocomplete="off"
              autocapitalize="none"
              :aria-label="t('kyc.countrySearchLabel')"
              :placeholder="t('kyc.countrySearchPlaceholder')"
              :spellcheck="false"
            />
          </label>
          <div class="kyc-picker-list">
            <button
              v-for="country in filteredCountryOptions"
              :key="country.value"
              class="kyc-picker-option kyc-country-picker-option"
              :class="{ 'is-selected': isCountrySelected(country.value) }"
              type="button"
              :aria-pressed="isCountrySelected(country.value)"
              @click="selectCountry(country.value)"
            >
              <span>
                <strong>{{ country.localizedLabel }}</strong>
                <small v-if="countrySecondaryLabel(country)">{{ countrySecondaryLabel(country) }}</small>
              </span>
              <code>{{ country.code }}</code>
              <Check v-if="isCountrySelected(country.value)" :size="18" aria-hidden="true" />
            </button>
            <div v-if="!filteredCountryOptions.length" class="kyc-picker-empty" role="status">
              <Search :size="24" aria-hidden="true" />
              <strong>{{ t('kyc.countryNoResults') }}</strong>
            </div>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="documentTypePickerOpen" class="kyc-picker-mask kyc-document-picker-mask" @click.self="closeDocumentTypePicker">
        <section
          id="kyc-document-type-picker"
          ref="documentTypePickerDialog"
          class="kyc-picker-sheet kyc-document-picker-sheet"
          role="dialog"
          aria-modal="true"
          aria-labelledby="kyc-document-type-picker-title"
          @keydown="handleDocumentTypePickerKeydown"
        >
          <div class="kyc-picker-handle" aria-hidden="true" />
          <header class="kyc-picker-header">
            <h2 id="kyc-document-type-picker-title">{{ t('kyc.documentTypePickerTitle') }}</h2>
            <button type="button" :aria-label="t('kyc.documentTypePickerClose')" @click="closeDocumentTypePicker">
              <X :size="20" aria-hidden="true" />
            </button>
          </header>
          <label class="kyc-picker-search">
            <Search :size="18" aria-hidden="true" />
            <input
              v-model="documentTypeSearch"
              data-document-type-search
              type="search"
              autocomplete="off"
              autocapitalize="none"
              :aria-label="t('kyc.documentTypeSearchLabel')"
              :placeholder="t('kyc.documentTypeSearchPlaceholder')"
              :spellcheck="false"
            />
          </label>
          <div class="kyc-picker-list">
            <button
              v-for="option in filteredDocumentTypeOptions"
              :key="option.value"
              class="kyc-picker-option kyc-document-picker-option"
              :class="{ 'is-selected': isDocumentTypeSelected(option.value) }"
              type="button"
              :aria-pressed="isDocumentTypeSelected(option.value)"
              @click="selectDocumentType(option.value)"
            >
              <span>
                <strong>{{ option.label }}</strong>
              </span>
              <Check v-if="isDocumentTypeSelected(option.value)" :size="18" aria-hidden="true" />
            </button>
            <div v-if="!filteredDocumentTypeOptions.length" class="kyc-picker-empty" role="status">
              <Search :size="24" aria-hidden="true" />
              <strong>{{ t('kyc.documentTypeNoResults') }}</strong>
            </div>
          </div>
        </section>
      </div>
    </Teleport>
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
.kyc-field input { appearance: none; background: transparent; border: 0; color: var(--ink); font-size: 14px; font-weight: 600; line-height: 20px; min-height: 25px; outline: 0; padding: 0; width: 100%; }
.kyc-picker-trigger { align-items: center; background: transparent; color: var(--ink); display: flex; font-size: 14px; font-weight: 600; gap: 8px; justify-content: space-between; min-height: 44px; padding: 0; text-align: left; width: 100%; }
.kyc-picker-trigger span { min-width: 0; overflow-wrap: anywhere; }
.kyc-picker-trigger svg { flex: 0 0 auto; }
.document-grid { display: grid; gap: 8px; grid-template-columns: repeat(3, minmax(0, 1fr)); }
.upload-tile { align-items: center; background: transparent; border: 1px solid var(--line); border-radius: 10px; color: var(--muted); display: flex; flex-direction: column; font-size: 11px; gap: 6px; height: 72px; justify-content: center; min-height: 72px; overflow: hidden; padding: 5px; position: relative; }
.upload-tile:focus-visible { box-shadow: 0 0 0 2px var(--focus-ring); outline: 0; }
.upload-tile--selected { border-color: var(--positive); color: var(--positive); }
.upload-tile img { height: 100%; inset: 0; object-fit: cover; position: absolute; width: 100%; }
.upload-tile__status { background: var(--surface-elevated); bottom: 0; color: var(--positive); font-size: 9px; font-weight: 600; inset-inline: 0; overflow: hidden; padding: 3px; position: absolute; text-align: center; text-overflow: ellipsis; white-space: nowrap; z-index: 1; }
.hidden-input { display: none; }
.kyc-note { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
.kyc-country-directory-warning { color: var(--warning); }
.kyc-submit { font-size: 15px; height: 48px; min-height: 48px; width: 100%; }
.kyc-page button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.kyc-page .kyc-picker-trigger:focus-visible { outline: 0; }
.kyc-picker-mask { align-items: end; backdrop-filter: blur(10px); background: var(--overlay); display: grid; inset: 0; justify-items: center; position: fixed; z-index: var(--layer-overlay); }
.kyc-picker-sheet { background: var(--surface-elevated); border: 1px solid var(--line); border-bottom: 0; border-radius: 24px 24px 0 0; display: grid; grid-template-rows: auto auto auto minmax(0, 1fr); height: min(680px, 82dvh); max-height: calc(100dvh - 20px); min-height: 360px; overflow: hidden; padding: 10px 20px calc(14px + env(safe-area-inset-bottom)); width: min(100%, 448px); }
.kyc-picker-handle { background: var(--line-strong); border-radius: 999px; height: 4px; justify-self: center; margin-bottom: 6px; width: 38px; }
.kyc-picker-header { align-items: center; display: flex; justify-content: space-between; min-height: 58px; }
.kyc-picker-header h2 { color: var(--ink); font-size: 19px; line-height: 25px; margin: 0; }
.kyc-picker-header button { align-items: center; background: color-mix(in srgb, var(--surface-elevated) 90%, var(--ink)); border: 1px solid var(--line); border-radius: 50%; color: var(--ink); display: inline-flex; height: 44px; justify-content: center; min-height: 44px; padding: 0; width: 44px; }
.kyc-picker-header button:focus-visible,
.kyc-picker-option:focus-visible { box-shadow: 0 0 0 3px var(--focus-ring); outline: 2px solid var(--focus); outline-offset: -2px; }
.kyc-picker-search { align-items: center; background: color-mix(in srgb, var(--surface-elevated) 90%, var(--ink)); border: 1px solid transparent; border-radius: 14px; color: var(--muted); display: flex; gap: 10px; height: 52px; margin: 6px 0 10px; padding: 0 14px; }
.kyc-picker-search:focus-within { border-color: var(--focus); box-shadow: 0 0 0 3px var(--focus-ring); }
.kyc-picker-search input { appearance: none; background: transparent; border: 0; color: var(--ink); font-size: 14px; height: 50px; min-width: 0; outline: 0; padding: 0; width: 100%; }
.kyc-picker-list { min-height: 0; overscroll-behavior: contain; overflow-y: auto; scrollbar-width: none; }
.kyc-picker-list::-webkit-scrollbar { display: none; }
.kyc-picker-option { align-items: center; background: transparent; border-bottom: 1px solid var(--line); color: var(--ink); display: grid; gap: 10px; grid-template-columns: minmax(0, 1fr) 20px; min-height: 58px; padding: 8px 2px; text-align: left; width: 100%; }
.kyc-country-picker-option { grid-template-columns: minmax(0, 1fr) auto 20px; }
.kyc-picker-option.is-selected { color: var(--positive); }
.kyc-picker-option > span { display: grid; gap: 2px; min-width: 0; }
.kyc-picker-option strong { font-size: 14px; overflow-wrap: anywhere; }
.kyc-picker-option small { color: var(--muted); font-size: 11px; overflow-wrap: anywhere; }
.kyc-picker-option code { color: var(--muted-strong); font-family: var(--data-font); font-size: 12px; }
.kyc-picker-empty { align-items: center; color: var(--muted); display: flex; flex-direction: column; gap: 10px; justify-content: center; min-height: 180px; text-align: center; }
:global(html[data-performance-tier='constrained'] .kyc-picker-mask) { backdrop-filter: none; }
@media (prefers-reduced-motion: reduce) {
  .kyc-picker-mask { backdrop-filter: none; }
}
@media (max-width: 340px) {
  .kyc-content { padding-inline: 16px; }
  .account-login-state { align-items: start; grid-template-columns: 44px minmax(0, 1fr); }
  .account-login-state .pencil-primary { grid-column: 2; justify-self: start; }
  .document-grid { gap: 6px; }
  .kyc-picker-sheet { padding-inline: 16px; }
}
</style>
