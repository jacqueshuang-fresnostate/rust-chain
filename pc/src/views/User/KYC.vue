<template>
  <div class="max-w-3xl mx-auto space-y-8">
    <div class="flex items-center justify-between mb-8">
      <h2 class="text-2xl font-bold">{{ t('kyc.title') }}</h2>
      <span v-if="securitySetting?.realVerified === 1" class="px-3 py-1 bg-up/20 text-up text-sm font-bold rounded-full border border-up/30">{{ t('kyc.verified') }}</span>
      <span v-else-if="securitySetting?.realAuditing === 1" class="px-3 py-1 bg-primary/20 text-primary text-sm font-bold rounded-full border border-primary/30">{{ t('kyc.auditing') }}</span>
      <span v-else class="px-3 py-1 bg-muted text-muted-foreground text-sm font-bold rounded-full border border-border">{{ t('kyc.unverified') }}</span>
    </div>

    <!-- Step Indicator -->
    <div class="flex items-center mb-10" v-if="!isVerified && !isAuditing">
      <!-- Step 1 -->
      <div class="flex flex-col items-center">
        <div :class="[
          'w-10 h-10 rounded-full flex items-center justify-center font-bold transition-colors',
          currentStep >= 1 ? 'bg-primary text-primary-foreground' : 'bg-muted border-2 border-border text-muted-foreground'
        ]">1</div>
        <span :class="['text-xs mt-2 font-bold', currentStep >= 1 ? 'text-primary' : 'text-muted-foreground']">{{ t('kyc.step1') }}</span>
      </div>
      <div :class="['h-[2px] flex-1 mx-4 transition-colors', currentStep >= 2 ? 'bg-primary' : 'bg-muted']"></div>

      <!-- Step 2 -->
      <div class="flex flex-col items-center">
        <div :class="[
          'w-10 h-10 rounded-full flex items-center justify-center font-bold transition-colors',
          currentStep >= 2 ? 'bg-primary text-primary-foreground' : 'bg-muted border-2 border-border text-muted-foreground'
        ]">2</div>
        <span :class="['text-xs mt-2 font-bold', currentStep >= 2 ? 'text-primary' : 'text-muted-foreground']">{{ t('kyc.step2') }}</span>
      </div>
      <div :class="['h-[2px] flex-1 mx-4 transition-colors', currentStep >= 3 ? 'bg-primary' : 'bg-muted']"></div>

      <!-- Step 3 -->
      <div class="flex flex-col items-center">
        <div :class="[
          'w-10 h-10 rounded-full flex items-center justify-center font-bold transition-colors',
          currentStep >= 3 ? 'bg-primary text-primary-foreground' : 'bg-muted border-2 border-border text-muted-foreground'
        ]">3</div>
        <span :class="['text-xs mt-2 font-bold', currentStep >= 3 ? 'text-primary' : 'text-muted-foreground']">{{ t('kyc.step3') }}</span>
      </div>
    </div>

    <div v-if="initLoading" class="flex justify-center py-10">
      <Icon icon="mdi:loading" class="animate-spin text-4xl text-primary" />
    </div>

    <template v-else>
      <!-- State: Verified -->
      <div v-if="isVerified" class="bg-card border border-border rounded-xl p-8 text-center flex flex-col items-center">
        <div class="w-20 h-20 bg-up/10 rounded-full flex items-center justify-center mb-4 border border-up/30">
            <Icon icon="mdi:check-decagram" class="text-4xl text-up" />
        </div>
        <h3 class="text-xl font-bold mb-2">{{ t('kyc.verify_complete') }}</h3>
        <p class="text-muted-foreground mb-6">{{ t('kyc.complete_desc') }}</p>

        <div class="w-full max-w-sm text-left bg-muted/30 p-4 rounded-lg border border-border">
          <div class="flex justify-between py-2 border-b border-border">
            <span class="text-muted-foreground text-sm">{{ t('kyc.real_name') }}</span>
            <span class="font-bold text-sm">{{ securitySetting?.realName }}</span>
          </div>
          <div class="flex justify-between py-2">
            <span class="text-muted-foreground text-sm">{{ t('kyc.id_number') }}</span>
            <span class="font-bold text-sm">{{ securitySetting?.idCard?.replace(/(.{4}).*(.{4})/, '$1********$2') }}</span>
          </div>
        </div>
      </div>

      <!-- State: Auditing -->
      <div v-else-if="isAuditing" class="bg-card border border-border rounded-xl p-8 text-center flex flex-col items-center">
        <div class="w-20 h-20 bg-primary/10 rounded-full flex items-center justify-center mb-4 border border-primary/30">
            <Icon icon="mdi:clock-outline" class="text-4xl text-primary" />
        </div>
        <h3 class="text-xl font-bold mb-2">{{ t('kyc.auditing') }}</h3>
        <p class="text-muted-foreground">{{ t('kyc.auditing_desc') }}</p>
      </div>

      <!-- State: Form -->
      <div v-else class="bg-card border border-border rounded-xl p-6 md:p-8">
        <!-- Step 1: Basic Info -->
        <form v-if="currentStep === 1" @submit.prevent="nextStep" class="space-y-6">
          <div class="space-y-4">
            <div>
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.certification_type') }}</label>
              <select
                v-model="form.submissionType"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors appearance-none"
              >
                <option value="personal">{{ t('kyc.certification_type_personal') }}</option>
                <option value="enterprise">{{ t('kyc.certification_type_enterprise') }}</option>
              </select>
            </div>

            <div>
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.full_name') }}</label>
              <input
                v-model="form.realName"
                type="text"
                required
                :placeholder="t('kyc.name_placeholder')"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors"
              />
            </div>

            <div v-if="isEnterpriseType">
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.enterprise_name') }}</label>
              <input
                v-model="form.enterpriseName"
                type="text"
                required
                :placeholder="t('kyc.enterprise_name')"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors"
              />
            </div>

            <div>
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.country') }}</label>
              <select
                v-model="form.country"
                required
                :disabled="countriesLoading || availableCountryOptions.length === 0"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors appearance-none"
              >
                <option value="" disabled>{{ countriesLoading ? t('kyc.loading_countries') : t('kyc.select_country') }}</option>
                <option v-for="country in availableCountryOptions" :key="country.value" :value="country.value">
                  {{ countryLabel(country) }}
                </option>
              </select>
            </div>

            <div>
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.document_type') }}</label>
              <select
                v-model="form.documentType"
                required
                :disabled="availableDocumentTypeOptions.length === 0"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors appearance-none"
              >
                <option value="" disabled>{{ t('kyc.select_document_type') }}</option>
                <option v-for="option in availableDocumentTypeOptions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </option>
              </select>
            </div>

            <div>
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.id_number') }}</label>
              <input
                v-model="form.idCard"
                type="text"
                required
                :placeholder="t('kyc.id_placeholder')"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors"
              />
            </div>

            <div v-if="isEnterpriseType">
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.enterprise_business_id') }}</label>
              <input
                v-model="form.businessRegistrationNumber"
                type="text"
                required
                :placeholder="t('kyc.enterprise_business_id')"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors"
              />
            </div>
          </div>
          <div class="flex justify-end pt-4">
            <button
                type="submit"
                class="px-8 py-3 bg-primary text-primary-foreground font-bold rounded hover:bg-primary/90 transition-all box-glow flex items-center gap-2"
            >
              {{ t('kyc.next') }}
              <Icon icon="mdi:arrow-right" />
            </button>
          </div>
        </form>

        <!-- Step 2: Upload Files -->
        <div v-if="currentStep === 2" class="space-y-6">
          <div :class="['grid grid-cols-1 gap-6', requiresHandheldImage ? 'md:grid-cols-3' : 'md:grid-cols-2']">
            <div
              class="border-2 border-dashed border-border rounded-xl p-8 flex flex-col items-center justify-center hover:border-primary/50 transition-colors cursor-pointer group relative overflow-hidden"
              @click="triggerUpload('front')"
            >
               <input type="file" ref="fileFront" class="hidden" @change="(e) => handleFileChange(e, 'front')" accept="image/*" />
               <template v-if="!form.frontImage">
                 <Icon icon="mdi:card-account-details-outline" class="text-6xl text-muted-foreground group-hover:text-primary mb-4 transition-colors" />
                 <span class="font-bold text-center">{{ t('kyc.front_title') }}</span>
                 <span class="text-xs text-muted-foreground mt-2 text-center">{{ t('kyc.front_desc', { size: maxDocumentSizeMb }) }}</span>
               </template>
               <template v-else>
                 <img :src="form.frontImagePreview" class="absolute inset-0 w-full h-full object-cover opacity-80" />
                 <div class="absolute inset-0 bg-black/50 flex flex-col items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                    <Icon icon="mdi:camera-retake" class="text-3xl text-white mb-2" />
                    <span class="text-white text-sm font-bold">{{ t('kyc.replace') }}</span>
                 </div>
               </template>
            </div>

            <div
              class="border-2 border-dashed border-border rounded-xl p-8 flex flex-col items-center justify-center hover:border-primary/50 transition-colors cursor-pointer group relative overflow-hidden"
              @click="triggerUpload('back')"
            >
               <input type="file" ref="fileBack" class="hidden" @change="(e) => handleFileChange(e, 'back')" accept="image/*" />
               <template v-if="!form.backImage">
                 <Icon icon="mdi:card-account-details" class="text-6xl text-muted-foreground group-hover:text-primary mb-4 transition-colors" />
                 <span class="font-bold text-center">{{ t('kyc.back_title') }}</span>
                 <span class="text-xs text-muted-foreground mt-2 text-center">{{ t('kyc.front_desc', { size: maxDocumentSizeMb }) }}</span>
               </template>
               <template v-else>
                 <img :src="form.backImagePreview" class="absolute inset-0 w-full h-full object-cover opacity-80" />
                 <div class="absolute inset-0 bg-black/50 flex flex-col items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                    <Icon icon="mdi:camera-retake" class="text-3xl text-white mb-2" />
                    <span class="text-white text-sm font-bold">{{ t('kyc.replace') }}</span>
                 </div>
               </template>
            </div>

            <div
              v-if="requiresHandheldImage"
              class="border-2 border-dashed border-border rounded-xl p-8 flex flex-col items-center justify-center hover:border-primary/50 transition-colors cursor-pointer group relative overflow-hidden"
              @click="triggerUpload('handheld')"
            >
               <input type="file" ref="fileHandheld" class="hidden" @change="(e) => handleFileChange(e, 'handheld')" accept="image/*" />
               <template v-if="!form.handheldImage">
                 <Icon icon="mdi:account-box-outline" class="text-6xl text-muted-foreground group-hover:text-primary mb-4 transition-colors" />
                 <span class="font-bold text-center">{{ t('kyc.handheld_title') }}</span>
                 <span class="text-xs text-muted-foreground mt-2 text-center">{{ t('kyc.handheld_desc', { size: maxDocumentSizeMb }) }}</span>
               </template>
               <template v-else>
                 <img :src="form.handheldImagePreview" class="absolute inset-0 w-full h-full object-cover opacity-80" />
                 <div class="absolute inset-0 bg-black/50 flex flex-col items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                    <Icon icon="mdi:camera-retake" class="text-3xl text-white mb-2" />
                    <span class="text-white text-sm font-bold">{{ t('kyc.replace') }}</span>
                 </div>
               </template>
            </div>
          </div>

          <div class="flex justify-between pt-4">
            <button
                @click="prevStep"
                class="px-6 py-3 border border-border text-foreground font-bold rounded hover:bg-muted transition-all"
            >
              {{ t('kyc.back') }}
            </button>
            <button
                @click="handleSubmit"
                class="px-8 py-3 bg-primary text-primary-foreground font-bold rounded hover:bg-primary/90 transition-all box-glow flex items-center gap-2"
                :disabled="submitting || !form.documentType || (requiresHandheldImage && !form.handheldImage)"
            >
              <Icon v-if="submitting" icon="mdi:loading" class="animate-spin" />
              {{ submitting ? t('kyc.submitting') : t('kyc.submit') }}
            </button>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { useToast } from 'vue-toastification'
import { fetchPublicCountries } from '@/api/countries'
import type { PcCountryOption } from '@/api/backendAdapters'
import { getKycStatus, getSecuritySetting, submitKycApplication, type KycConfig, type MemberSecurity } from '@/api/user'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const toast = useToast()
const initLoading = ref(true)
const countriesLoading = ref(false)
const submitting = ref(false)
const currentStep = ref(1)

const securitySetting = ref<MemberSecurity | null>(null)
const kycConfig = ref<KycConfig | null>(null)
const countryOptions = ref<PcCountryOption[]>([])
const isVerified = computed(() => securitySetting.value?.realVerified === 1)
const isAuditing = computed(() => securitySetting.value?.realAuditing === 1)

const form = ref({
    realName: '',
    submissionType: 'personal' as 'personal' | 'enterprise',
    enterpriseName: '',
    businessRegistrationNumber: '',
    idCard: '',
    country: '',
    documentType: 'identity_card',
    frontImage: null as File | null,
    frontImagePreview: '',
    backImage: null as File | null,
    backImagePreview: '',
    handheldImage: null as File | null,
    handheldImagePreview: '',
})

const fileFront = ref<HTMLInputElement | null>(null)
const fileBack = ref<HTMLInputElement | null>(null)
const fileHandheld = ref<HTMLInputElement | null>(null)

type KycCountrySelectOption = PcCountryOption & {
    value: string
}

const defaultDocumentTypes = ['identity_card', 'passport', 'driver_license', 'residence_permit']
const fallbackCountryOptions: PcCountryOption[] = [
    { code: 'CN', name: 'China', defaultLocale: 'zh', supportedLocales: ['zh', 'en'] },
    { code: 'US', name: 'United States', defaultLocale: 'en', supportedLocales: ['en'] },
    { code: 'GB', name: 'United Kingdom', defaultLocale: 'en', supportedLocales: ['en'] },
    { code: 'JP', name: 'Japan', defaultLocale: 'en', supportedLocales: ['en'] },
    { code: 'KR', name: 'South Korea', defaultLocale: 'en', supportedLocales: ['en'] },
]

const maxDocumentSizeBytes = computed(() => kycConfig.value?.max_document_size_bytes ?? 5 * 1024 * 1024)
const maxDocumentSizeMb = computed(() => Math.max(1, Math.round(maxDocumentSizeBytes.value / 1024 / 1024)))
const isEnterpriseType = computed(() => form.value.submissionType === 'enterprise')

const configuredCountries = computed(() => {
    const rules = kycConfig.value?.country_document_types?.map((rule) => rule.country).filter(Boolean) ?? []
    if (rules.length > 0) {
        return uniqueValues(rules)
    }
    return uniqueValues(kycConfig.value?.allowed_countries ?? [])
})

const availableCountryOptions = computed<KycCountrySelectOption[]>(() => {
    const source = countryOptions.value.length > 0 ? countryOptions.value : fallbackCountryOptions
    const configured = configuredCountries.value
    if (configured.length === 0) {
        return source.map((country) => ({ ...country, value: country.name }))
    }
    return configured.map((value) => {
        const matched = source.find((country) => countryMatches(value, country))
        return {
            code: matched?.code ?? value,
            name: matched?.name ?? value,
            defaultLocale: matched?.defaultLocale ?? 'en',
            supportedLocales: matched?.supportedLocales ?? ['en'],
            value,
        }
    })
})

const selectedCountryRule = computed(() => {
    return kycConfig.value?.country_document_types?.find((item) => item.country.toLowerCase() === form.value.country.toLowerCase())
})

const availableDocumentTypeOptions = computed(() => {
    const rule = selectedCountryRule.value
    const documentTypes = rule?.document_types?.length ? rule.document_types : defaultDocumentTypes
    return uniqueValues(documentTypes).map((value) => ({
        value,
        label: documentTypeLabel(value),
    }))
})

const requiresHandheldImage = computed(() => {
    return Boolean(selectedCountryRule.value?.handheld_document_types?.includes(form.value.documentType))
})

const fetchSecuritySetting = async () => {
    countriesLoading.value = true
    try {
        const [securityResult, kycResult, countriesResult] = await Promise.allSettled([
            getSecuritySetting(),
            getKycStatus(),
            fetchPublicCountries(),
        ])
        if (securityResult.status === 'fulfilled' && (securityResult.value.code === 0 || securityResult.value.code === 200)) {
            securitySetting.value = securityResult.value.data
        }
        if (kycResult.status === 'fulfilled' && (kycResult.value.code === 0 || kycResult.value.code === 200)) {
            kycConfig.value = kycResult.value.data.config
        }
        if (countriesResult.status === 'fulfilled' && (countriesResult.value.code === 0 || countriesResult.value.code === 200)) {
            countryOptions.value = countriesResult.value.data
        }
    } catch (e) {
        console.error('Failed to load KYC settings', e)
    } finally {
        countriesLoading.value = false
        initLoading.value = false
    }
}

onMounted(() => {
    fetchSecuritySetting()
})

watch(availableCountryOptions, (options) => {
    if (form.value.country && options.some((country) => country.value === form.value.country)) {
        return
    }
    form.value.country = options[0]?.value ?? ''
})

watch(availableDocumentTypeOptions, (options) => {
    if (options.some((option) => option.value === form.value.documentType)) {
        return
    }
    form.value.documentType = options[0]?.value ?? ''
})

function uniqueValues(values: string[]) {
    return values.map((value) => value.trim()).filter(Boolean).filter((value, index, items) => items.findIndex((item) => item.toLowerCase() === value.toLowerCase()) === index)
}

function countryMatches(value: string, country: PcCountryOption) {
    const normalized = value.toLowerCase()
    return country.name.toLowerCase() === normalized || country.code.toLowerCase() === normalized
}

function countryLabel(country: KycCountrySelectOption) {
    return country.code && country.code !== country.name ? `${country.name} (${country.code})` : country.name
}

function documentTypeLabel(value: string) {
    const labelKey = `kyc.document_type_${value}`
    const label = t(labelKey)
    return label === labelKey ? value : label
}

function nextStep() {
    if (!form.value.realName || !form.value.country || !form.value.documentType || !form.value.idCard) {
        toast.error(t('kyc.fill_basic'))
        return
    }
    if (form.value.submissionType === 'enterprise' && (!form.value.enterpriseName || !form.value.businessRegistrationNumber)) {
        toast.error(t('kyc.fill_basic'))
        return
    }
    currentStep.value = 2
}

function prevStep() {
    currentStep.value = 1
}

function triggerUpload(side: 'front' | 'back' | 'handheld') {
    if (side === 'front' && fileFront.value) {
        fileFront.value.click()
    } else if (side === 'back' && fileBack.value) {
        fileBack.value.click()
    } else if (side === 'handheld' && fileHandheld.value) {
        fileHandheld.value.click()
    }
}

function handleFileChange(event: Event, side: 'front' | 'back' | 'handheld') {
    const target = event.target as HTMLInputElement
    if (target.files && target.files.length > 0) {
        const file = target.files[0]
        if (file.size > maxDocumentSizeBytes.value) {
             toast.error(t('kyc.size_err', { size: maxDocumentSizeMb.value }))
             return
        }

        const previewUrl = URL.createObjectURL(file)
        if (side === 'front') {
            form.value.frontImage = file
            form.value.frontImagePreview = previewUrl
        } else if (side === 'back') {
            form.value.backImage = file
            form.value.backImagePreview = previewUrl
        } else {
            form.value.handheldImage = file
            form.value.handheldImagePreview = previewUrl
        }
    }
}

function fileToDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(String(reader.result))
        reader.onerror = () => reject(reader.error ?? new Error('Failed to read file'))
        reader.readAsDataURL(file)
    })
}

function getErrorMessage(error: unknown) {
    if (error instanceof Error) {
        return error.message
    }
    return t('kyc.submit_failed')
}

async function handleSubmit() {
    if (!form.value.frontImage || !form.value.backImage) {
        toast.error(t('kyc.upload_both'))
        return
    }
    if (requiresHandheldImage.value && !form.value.handheldImage) {
        toast.error(t('kyc.upload_handheld'))
        return
    }

    submitting.value = true
    try {
        const [documentFrontImage, documentBackImage, documentHandheldImage] = await Promise.all([
            fileToDataUrl(form.value.frontImage),
            fileToDataUrl(form.value.backImage),
            requiresHandheldImage.value && form.value.handheldImage ? fileToDataUrl(form.value.handheldImage) : Promise.resolve(undefined),
        ])
        await submitKycApplication({
            submission_type: form.value.submissionType,
            real_name: form.value.realName,
            enterprise_name: isEnterpriseType.value ? form.value.enterpriseName : undefined,
            business_registration_number: isEnterpriseType.value
                ? form.value.businessRegistrationNumber
                : undefined,
            country: form.value.country,
            id_number: form.value.idCard,
            document_type: form.value.documentType,
            document_front_image: documentFrontImage,
            document_back_image: documentBackImage,
            ...(documentHandheldImage ? { document_handheld_image: documentHandheldImage } : {}),
        })
        toast.success(t('kyc.submit_success'))
        securitySetting.value = {
            ...(securitySetting.value ?? {}),
            realAuditing: 1,
            realVerified: 0,
            realName: form.value.realName,
            idCard: form.value.idCard,
        }
        currentStep.value = 3
        await fetchSecuritySetting()
    } catch (error) {
        toast.error(getErrorMessage(error))
    } finally {
        submitting.value = false
    }
}
</script>
