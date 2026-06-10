<template>
  <div class="min-h-full p-4 md:p-8 max-w-7xl mx-auto space-y-8">
    <!-- Header Section -->
    <div class="flex flex-col md:flex-row justify-between items-end gap-4 border-b border-border pb-6">
      <div>
        <h1 class="text-4xl font-black text-transparent bg-clip-text bg-gradient-to-r from-primary to-neon-pink mb-2 tracking-tighter">
          {{ t('launchpad.title') }}
        </h1>
        <p class="text-muted-foreground text-lg">
          {{ t('launchpad.subtitle') }}
        </p>
      </div>
      <!-- Tabs -->
      <div class="flex gap-2 overflow-x-auto pb-2 md:pb-0">
        <button
            v-for="tab in tabs"
            :key="tab.value"
            @click="switchTab(tab.value)"
            class="px-4 py-2 text-sm font-bold border rounded transition-all whitespace-nowrap"
            :class="activeStep === tab.value
              ? 'bg-primary/10 text-primary border-primary/20'
              : 'bg-muted text-muted-foreground border-transparent hover:text-foreground'"
        >
          {{ t(tab.labelKey) }}
        </button>
      </div>
    </div>

    <!-- Active Projects Grid -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
      <div v-for="project in projects" :key="project.id" class="group relative bg-card border border-border rounded-xl overflow-hidden hover:border-primary/50 transition-all duration-300 shadow-lg hover:shadow-primary/10 flex flex-col">

        <!-- Top Banner & Logo -->
        <div class="relative h-40 overflow-hidden">
            <div class="absolute inset-0 bg-gradient-to-t from-card via-transparent to-transparent z-10"></div>
            <!-- Banner Image -->
            <img v-if="project.banner" :src="project.banner" class="w-full h-full object-cover opacity-80 group-hover:scale-105 transition-transform duration-700" />
            <div v-else class="w-full h-full bg-muted/20 flex items-center justify-center text-muted-foreground">
                <span class="i-mdi-image-off w-12 h-12 opacity-20"></span>
            </div>

            <!-- Status Badge -->
            <div class="absolute top-4 right-4 z-20">
                 <div class="px-3 py-1 rounded-full text-xs font-bold uppercase tracking-wider shadow-lg flex items-center gap-1.5 backdrop-blur-md"
                      :class="getStatusClass(project.statusStr)">
                    <span class="w-2 h-2 rounded-full bg-current" :class="{'animate-pulse': project.statusStr === 'LIVE'}"></span>
                    {{ getTranslatedStatus(project.statusStr) }}
                 </div>
            </div>
        </div>

        <!-- Main Content -->
        <div class="px-6 relative -mt-10 z-20 flex-1 flex flex-col">
            <!-- Header: Logo + Title -->
            <div class="flex items-end gap-4 mb-4">
                <div class="w-20 h-20 rounded-xl bg-card border-2 border-border p-1.5 shadow-xl shrink-0">
                    <img :src="project.logo" class="w-full h-full object-cover rounded-lg bg-background" />
                </div>
                <div class="mb-1 flex-1 min-w-0">
                    <h2 class="text-2xl font-bold text-foreground truncate" :title="project.name">{{ project.name }}</h2>
                    <div class="flex items-center gap-2 text-sm">
                        <span class="text-primary font-bold bg-primary/10 px-2 py-0.5 rounded text-xs">{{ project.symbol }}</span>
                        <span class="text-muted-foreground text-xs bg-muted px-2 py-0.5 rounded" v-if="project.typeLabel">{{ project.typeLabel }}</span>
                    </div>
                </div>
            </div>

            <!-- Description -->
            <p class="text-sm text-muted-foreground line-clamp-2 mb-6 h-10">{{ project.description }}</p>

            <!-- Key Info Grid -->
            <div class="grid grid-cols-2 gap-3 mb-6">
                 <div class="bg-muted/30 p-3 rounded-lg border border-border/50">
                    <div class="text-xs text-muted-foreground mb-1">{{ t('launchpad.sale_price') }}</div>
                    <div class="font-mono font-bold text-up text-sm">1 {{ project.symbol }} = {{ formatNumber(project.price) }} {{ project.acceptUnit?.split('-')[0] }}</div>
                 </div>
                 <div class="bg-muted/30 p-3 rounded-lg border border-border/50">
                    <div class="text-xs text-muted-foreground mb-1">{{ t('launchpad.total_supply') }}</div>
                    <div class="font-mono font-bold text-sm">{{ formatCompact(project.totalSupply) }} {{ project.symbol }}</div>
                 </div>
                 <div class="bg-muted/30 p-3 rounded-lg border border-border/50">
                    <div class="text-xs text-muted-foreground mb-1">{{ t('launchpad.min_max_buy') }}</div>
                    <div class="font-mono font-bold text-sm truncate">
                        {{ formatNumber(project.minLimit) }} / {{ project.maxLimit > 0 ? formatNumber(project.maxLimit) : t('launchpad.no_limit') }}
                    </div>
                 </div>
                 <div class="bg-muted/30 p-3 rounded-lg border border-border/50">
                    <div class="text-xs text-muted-foreground mb-1">{{ t('launchpad.accept') }}</div>
                    <div class="font-mono font-bold text-sm">{{ project.acceptUnit }}</div>
                 </div>
            </div>

            <!-- Progress Section -->
            <div class="mb-6 space-y-2">
                <div class="flex justify-between text-sm">
                     <span class="font-bold text-muted-foreground">{{ t('launchpad.progress') }}</span>
                     <span class="font-mono font-bold text-primary">{{ project.progress.toFixed(2) }}%</span>
                </div>
                <div class="h-3 bg-muted rounded-full overflow-hidden relative">
                    <div class="absolute inset-0 bg-stripes opacity-10"></div>
                    <div class="h-full bg-gradient-to-r from-primary to-neon-purple shadow-[0_0_10px_rgba(var(--primary),0.5)] transition-all duration-1000 ease-out relative"
                         :style="{ width: project.progress + '%' }">
                         <div class="absolute right-0 top-0 bottom-0 w-1 bg-white/50"></div>
                    </div>
                </div>
                <div class="flex justify-between text-xs font-mono text-muted-foreground">
                    <span>{{ formatCompact(project.tradedAmount) }} {{ project.symbol }}</span>
                    <span>{{ formatCompact(project.totalSupply) }} {{ project.symbol }}</span>
                </div>
            </div>

            <div class="flex-1"></div> <!-- Spacer -->

            <!-- Action Area -->
            <div class="space-y-4 pt-4 border-t border-border/50">
                 <!-- Time Status -->
                 <div class="flex justify-center items-center gap-2 text-sm font-mono mb-2">
                     <template v-if="project.statusStr === 'LIVE'">
                         <span class="text-muted-foreground">{{ t('launchpad.ends_in') }}</span>
                         <span class="text-foreground font-bold bg-muted px-2 rounded">{{ project.timeLeft }}</span>
                     </template>
                     <template v-else-if="project.statusStr === 'UPCOMING'">
                         <span class="text-muted-foreground">{{ t('launchpad.starts_in') }}</span>
                         <span class="text-foreground font-bold bg-muted px-2 rounded">{{ project.timeLeft }}</span>
                     </template>
                     <template v-else>
                         <span class="text-muted-foreground">{{ t('launchpad.sale_ended') }}</span>
                         <span class="text-muted-foreground bg-muted px-2 rounded">{{ project.endTime }}</span>
                     </template>
                 </div>

                 <!-- Buy Input -->
                 <div v-if="project.statusStr === 'LIVE'" class="flex gap-2">
                    <div class="relative flex-1 group/input">
                         <input type="number"
                                v-model="amounts[project.id]"
                                :placeholder="`${t('launchpad.amount_placeholder')} (${project.acceptUnit?.split('-')[0]})`"
                                class="w-full bg-background border border-border rounded-lg px-3 py-2.5 text-sm outline-none focus:border-primary transition-colors font-mono" />
                         <!-- Quick Max Button could go here -->
                    </div>
                    <button @click="handleSubscribe(project)"
                            :disabled="subscribing"
                            class="px-6 py-2.5 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all hover:shadow-[0_0_15px_rgba(var(--primary),0.4)] disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap">
                        {{ subscribing ? t('launchpad.subscribing') : t('launchpad.subscribe') }}
                    </button>
                 </div>

                 <button v-else class="w-full py-3 bg-muted/50 text-muted-foreground font-bold rounded-lg border border-border cursor-not-allowed">
                    {{ project.statusStr === 'UPCOMING' ? t('launchpad.coming_soon') : t('launchpad.sale_ended') }}
                 </button>

                 <!-- Detail Toggle -->
                 <div class="text-center">
                     <button @click="toggleDetail(project.id)" class="text-xs text-muted-foreground hover:text-primary underline transition-colors">
                         {{ expandedId === project.id ? t('launchpad.hide_details') : t('launchpad.view_details') }}
                     </button>
                 </div>
            </div>
        </div>

        <!-- Expanded Details (Accordion style) -->
        <div v-if="expandedId === project.id" class="border-t border-border bg-muted/10 p-6 animate-in slide-in-from-top-2">
            <h3 class="font-bold text-lg mb-4">{{ t('launchpad.project_intro') }}</h3>
            <div class="prose prose-invert prose-sm max-w-none text-muted-foreground" v-html="project.content"></div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import numeral from 'numeral'
import { useToast } from 'vue-toastification'
import { fetchActivityList, attendActivity, type IEOProject } from '@/api/activity'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const toast = useToast()
const subscribing = ref(false)
const amounts = ref<Record<number, number>>({})
const projects = ref<any[]>([])
const expandedId = ref<number | null>(null)
const activeStep = ref(-1)

const tabs = [
    { labelKey: 'launchpad.tabs.all', value: -1 },
    { labelKey: 'launchpad.tabs.upcoming', value: 0 },
    { labelKey: 'launchpad.tabs.live', value: 1 },
    { labelKey: 'launchpad.tabs.distributing', value: 2 },
    { labelKey: 'launchpad.tabs.ended', value: 3 }
]

const formatNumber = (val: number) => {
    return numeral(val).format('0,0.[0000]')
}

const formatCompact = (val: number) => {
    return numeral(val).format('0.00a').toUpperCase()
}

const getStatusClass = (statusStr: string) => {
    switch (statusStr) {
        case 'LIVE': return 'bg-up/20 text-up border border-up/30'
        case 'UPCOMING': return 'bg-primary/20 text-primary border border-primary/30'
        default: return 'bg-muted text-muted-foreground border border-border'
    }
}

const getTranslatedStatus = (statusStr: string) => {
    switch (statusStr) {
        case 'LIVE': return t('launchpad.status.live')
        case 'UPCOMING': return t('launchpad.status.upcoming')
        case 'ENDED': return t('launchpad.status.ended')
        case 'DISTRIBUTING': return t('launchpad.status.distributing')
        default: return statusStr
    }
}

const mapStepToStatus = (step: number) => {
    // 0: 未开始, 1: 进行中, 2: 派发中, 3: 已结束
    if (step === 0) return 'UPCOMING'
    if (step === 1) return 'LIVE'
    return 'ENDED'
}

const getProjectTypeLabel = (type: number) => {
    const map: Record<number, string> = {
        1: 'Panic Buy',
        2: 'Allocation',
        3: 'Airdrop',
        4: 'Subscription',
        5: 'Cloud Mining',
        6: 'Lock Drop'
    }
    // Return translated string if we ever add it to i18n
    return map[type] || 'IEO'
}

const calculateTimeLeft = (targetDateStr: string) => {
    const target = new Date(targetDateStr).getTime()
    const now = new Date().getTime()
    const diff = target - now

    if (diff <= 0) return '00d 00h 00m'

    const days = Math.floor(diff / (1000 * 60 * 60 * 24))
    const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60))
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60))

    return `${days}d ${hours}h ${minutes}m`
}

const toggleDetail = (id: number) => {
    expandedId.value = expandedId.value === id ? null : id
}

const switchTab = (step: number) => {
    activeStep.value = step
    loadProjects()
}

const loadProjects = async () => {
    try {
        const res = await fetchActivityList(1, 100, activeStep.value)
        const data = res.data?.data || res.data // Fallback if structure is flat
        if (data && data.content) {
            projects.value = data.content.map((item: IEOProject) => {
                const statusStr = mapStepToStatus(item.step)
                let timeLeft = ''

                if (statusStr === 'UPCOMING') {
                    timeLeft = calculateTimeLeft(item.startTime)
                } else if (statusStr === 'LIVE') {
                    timeLeft = calculateTimeLeft(item.endTime)
                }

                return {
                    id: item.id,
                    name: item.title,
                    symbol: item.unit,
                    typeLabel: getProjectTypeLabel(item.type),
                    logo: item.smallImageUrl,
                    banner: item.bannerImageUrl,
                    description: item.detail,
                    content: item.content, // HTML content

                    price: item.price,
                    totalSupply: item.totalSupply,
                    tradedAmount: item.tradedAmount,
                    acceptUnit: item.acceptUnit,
                    quoteAssetId: item.acceptAssetId,

                    minLimit: item.minLimitAmout,
                    maxLimit: item.maxLimitAmout,

                    progress: item.progress || (item.totalSupply > 0 ? (item.tradedAmount / item.totalSupply) * 100 : 0),
                    statusStr: statusStr,
                    timeLeft: timeLeft,
                    endTime: item.endTime
                }
            })
        }
    } catch (e) {
        console.error("Failed to load IEO projects", e)
    }
}

const handleSubscribe = async (project: any) => {
    const amount = amounts.value[project.id]
    if (!amount || amount <= 0) {
        toast.warning(t('launchpad.invalid_amount'))
        return
    }

    if (project.minLimit > 0 && amount < project.minLimit) {
        toast.warning(`${t('launchpad.min_buy')} ${project.minLimit}`)
        return
    }
    if (project.maxLimit > 0 && amount > project.maxLimit) {
        toast.warning(`${t('launchpad.max_buy')} ${project.maxLimit}`)
        return
    }

    subscribing.value = true

    try {
        await attendActivity({
            id: project.id,
            symbol: project.symbol,
            amount,
            price: project.price,
            quoteAssetId: project.quoteAssetId
        })
        toast.success(`${t('launchpad.subscribe_success')} ${amount} ${project.acceptUnit?.split('-')[0] || ''}`)
        loadProjects()
        amounts.value[project.id] = 0
    } catch (e: any) {
         toast.error(e.message || t('launchpad.subscribe_failed'))
    } finally {
        subscribing.value = false
    }
}

onMounted(() => {
    loadProjects()
})
</script>

<style scoped>
.bg-stripes {
  background-image: linear-gradient(
    45deg,
    rgba(255, 255, 255, 0.15) 25%,
    transparent 25%,
    transparent 50%,
    rgba(255, 255, 255, 0.15) 50%,
    rgba(255, 255, 255, 0.15) 75%,
    transparent 75%,
    transparent
  );
  background-size: 1rem 1rem;
}
</style>
