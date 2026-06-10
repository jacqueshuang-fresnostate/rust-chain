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
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.full_name') }}</label>
              <input
                v-model="form.realName"
                type="text"
                required
                :placeholder="t('kyc.name_placeholder')"
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors"
              />
            </div>

            <div>
              <label class="text-sm font-bold text-muted-foreground mb-1 block">{{ t('kyc.country') }}</label>
              <select
                v-model="form.country"
                required
                class="w-full bg-muted/50 border border-border rounded-lg p-3 text-sm focus:border-primary focus:outline-none transition-colors appearance-none"
              >
                <option value="" disabled>{{ t('kyc.select_country') }}</option>
                <option value="China">China</option>
                <option value="United States">United States</option>
                <option value="United Kingdom">United Kingdom</option>
                <option value="Japan">Japan</option>
                <option value="South Korea">South Korea</option>
                <!-- Add more as needed -->
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
          <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div
              class="border-2 border-dashed border-border rounded-xl p-8 flex flex-col items-center justify-center hover:border-primary/50 transition-colors cursor-pointer group relative overflow-hidden"
              @click="triggerUpload('front')"
            >
               <input type="file" ref="fileFront" class="hidden" @change="(e) => handleFileChange(e, 'front')" accept="image/*" />
               <template v-if="!form.frontImage">
                 <Icon icon="mdi:card-account-details-outline" class="text-6xl text-muted-foreground group-hover:text-primary mb-4 transition-colors" />
                 <span class="font-bold text-center">{{ t('kyc.front_title') }}</span>
                 <span class="text-xs text-muted-foreground mt-2 text-center">{{ t('kyc.front_desc') }}</span>
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
                 <span class="text-xs text-muted-foreground mt-2 text-center">{{ t('kyc.front_desc') }}</span>
               </template>
               <template v-else>
                 <img :src="form.backImagePreview" class="absolute inset-0 w-full h-full object-cover opacity-80" />
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
                :disabled="submitting"
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
import { ref, onMounted, computed } from 'vue'
import { Icon } from '@iconify/vue'
import { useToast } from 'vue-toastification'
import { getSecuritySetting, type MemberSecurity } from '@/api/user'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const toast = useToast()
const initLoading = ref(true)
const submitting = ref(false)
const currentStep = ref(1)

const securitySetting = ref<MemberSecurity | null>(null)
const isVerified = computed(() => securitySetting.value?.realVerified === 1)
const isAuditing = computed(() => securitySetting.value?.realAuditing === 1)

const form = ref({
    realName: '',
    idCard: '',
    country: '',
    frontImage: null as File | null,
    frontImagePreview: '',
    backImage: null as File | null,
    backImagePreview: '',
})

const fileFront = ref<HTMLInputElement | null>(null)
const fileBack = ref<HTMLInputElement | null>(null)

const fetchSecuritySetting = async () => {
    try {
        const res: any = await getSecuritySetting()
        if (res.code === 0 || res.code === 200) {
            securitySetting.value = res.data
        }
    } catch (e) {
        console.error('Failed to load KYC settings', e)
    } finally {
        initLoading.value = false
    }
}

onMounted(() => {
    fetchSecuritySetting()
})

function nextStep() {
    if (!form.value.realName || !form.value.idCard || !form.value.country) {
        toast.error(t('kyc.fill_basic'))
        return
    }
    currentStep.value = 2
}

function prevStep() {
    currentStep.value = 1
}

function triggerUpload(side: 'front' | 'back') {
    if (side === 'front' && fileFront.value) {
        fileFront.value.click()
    } else if (side === 'back' && fileBack.value) {
        fileBack.value.click()
    }
}

function handleFileChange(event: Event, side: 'front' | 'back') {
    const target = event.target as HTMLInputElement
    if (target.files && target.files.length > 0) {
        const file = target.files[0]
        // Check size (5MB)
        if (file.size > 5 * 1024 * 1024) {
             toast.error(t('kyc.size_err'))
             return
        }

        const previewUrl = URL.createObjectURL(file)
        if (side === 'front') {
            form.value.frontImage = file
            form.value.frontImagePreview = previewUrl
        } else {
            form.value.backImage = file
            form.value.backImagePreview = previewUrl
        }
    }
}

async function handleSubmit() {
    if (!form.value.frontImage || !form.value.backImage) {
        toast.error(t('kyc.upload_both'))
        return
    }

    submitting.value = true
    toast.error('当前后端暂未开放实名认证提交接口')
    submitting.value = false
}
</script>
