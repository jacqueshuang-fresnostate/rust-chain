<template>
  <div class="space-y-8">
    <h2 class="text-2xl font-bold mb-6">{{ t('invite.title') }}</h2>

    <div v-if="initLoading" class="flex justify-center py-10">
      <Icon icon="mdi:loading" class="animate-spin text-4xl text-primary" />
    </div>

    <template v-else>
      <!-- Invitation Info -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
        <div class="p-6 bg-card border border-border rounded-xl space-y-4">
          <div class="flex items-center gap-3 text-muted-foreground mb-4">
            <Icon icon="mdi:ticket-percent-outline" class="text-2xl text-primary" />
            <h3 class="font-bold text-foreground">{{ t('invite.code_title') }}</h3>
          </div>

          <div class="flex items-center gap-4">
            <div class="text-3xl font-black tracking-widest text-primary font-mono">
              {{ securitySetting?.promotionCode || '------' }}
            </div>
            <button
              @click="copyToClipboard(securitySetting?.promotionCode || '')"
              class="p-2 hover:bg-muted rounded-full transition-colors text-muted-foreground hover:text-foreground"
              :title="t('invite.copy_code')"
            >
              <Icon icon="mdi:content-copy" class="text-xl" />
            </button>
          </div>
          <p class="text-xs text-muted-foreground">{{ t('invite.code_desc') }}</p>
        </div>

        <div class="p-6 bg-card border border-border rounded-xl space-y-4">
          <div class="flex items-center gap-3 text-muted-foreground mb-4">
            <Icon icon="mdi:link-variant" class="text-2xl text-primary" />
            <h3 class="font-bold text-foreground">{{ t('invite.link_title') }}</h3>
          </div>

          <div class="flex flex-col gap-2">
            <div class="flex items-center gap-2">
              <input
                type="text"
                readonly
                :value="inviteLink"
                class="flex-1 bg-muted/50 border border-border rounded-lg p-3 text-sm focus:outline-none text-foreground"
              />
              <button
                @click="copyToClipboard(inviteLink)"
                class="px-4 py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-colors flex items-center justify-center min-w-[100px]"
              >
                {{ t('invite.copy_link') }}
              </button>
            </div>
          </div>
          <p class="text-xs text-muted-foreground">{{ t('invite.link_desc') }}</p>
        </div>
      </div>

      <!-- Invitation Records -->
      <div class="mt-8 space-y-4">
        <h3 class="text-lg font-bold flex items-center gap-2">
          <Icon icon="mdi:format-list-bulleted" class="text-primary" />
          {{ t('invite.records_title') }}
        </h3>

        <div class="bg-card border border-border rounded-xl overflow-hidden">
          <div class="overflow-x-auto">
            <table class="w-full text-sm text-left">
              <thead class="text-xs uppercase bg-muted/50 border-b border-border">
                <tr>
                  <th scope="col" class="px-6 py-4 text-muted-foreground">{{ t('invite.date') }}</th>
                  <th scope="col" class="px-6 py-4 text-muted-foreground">{{ t('invite.invitee') }}</th>
                  <th scope="col" class="px-6 py-4 text-muted-foreground">{{ t('invite.status') }}</th>
                  <th scope="col" class="px-6 py-4 text-right text-muted-foreground">{{ t('invite.reward') }}</th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="inviteRecords.length === 0" class="border-b border-border hover:bg-muted/20 transition-colors">
                  <td colspan="4" class="px-6 py-8 text-center text-muted-foreground">{{ t('invite.no_records') }}</td>
                </tr>
                <tr v-for="(record, index) in inviteRecords" :key="index" class="border-b border-border hover:bg-muted/20 transition-colors">
                  <td class="px-6 py-4 text-foreground">{{ record.date }}</td>
                  <td class="px-6 py-4 text-foreground font-medium">{{ record.invitee }}</td>
                  <td class="px-6 py-4">
                    <span :class="['px-2 py-1 text-xs font-bold rounded', record.status === 'Completed' ? 'bg-up/20 text-up border border-up/30' : 'bg-primary/20 text-primary border border-primary/30']">
                      {{ record.status }}
                    </span>
                  </td>
                  <td class="px-6 py-4 text-right font-bold text-up">{{ record.reward }}</td>
                </tr>
              </tbody>
            </table>
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
import { getReferralCode, getReferralInvites, type MemberSecurity } from '@/api/user'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const toast = useToast()
const initLoading = ref(true)
const securitySetting = ref<MemberSecurity | null>(null)
const inviteRecords = ref<Array<{ date: string; invitee: string; status: string; reward: string }>>([])

const inviteLink = computed(() => {
    if (!securitySetting.value?.promotionCode) return t('invite.generating')
    const domain = window.location.origin
    return `${domain}/register?promotion=${securitySetting.value.promotionCode}`
})

const fetchInvitationInfo = async () => {
    try {
        const [codeRes, invitesRes]: any[] = await Promise.all([
            getReferralCode(),
            getReferralInvites(),
        ])
        if (codeRes.code === 0 || codeRes.code === 200) {
            securitySetting.value = { id: codeRes.data.owner_id, createTime: '', promotionCode: codeRes.data.code }
        }
        if (invitesRes.code === 0 || invitesRes.code === 200) {
            inviteRecords.value = invitesRes.data
        }
    } catch (e) {
        console.error('Failed to load invitation info', e)
    } finally {
        initLoading.value = false
    }
}

const copyToClipboard = async (text: string) => {
    if (!text || text === '------' || text === t('invite.generating')) {
        toast.warning(t('invite.not_available'))
        return
    }

    try {
        await navigator.clipboard.writeText(text)
        toast.success(t('invite.copied'))
    } catch (err) {
        console.error('Failed to copy text: ', err)
        toast.error(t('invite.copy_failed'))
    }
}

onMounted(() => {
    fetchInvitationInfo()
})
</script>
