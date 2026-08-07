<script setup lang="ts">
import { computed, ref, type Component } from 'vue'
import {
  ArrowLeftRight,
  ChevronDown,
  ChevronRight,
  Info,
  Mail,
  MessageCircle,
  Search,
  ShieldCheck,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import PageHeader from '@/components/PageHeader.vue'

type HelpFaq = {
  id: string
  icon: Component
  titleKey: string
  summaryKey: string
  answerKey: string
}

const { t } = useI18n()
const query = ref('')
const expandedFaqId = ref('')
const supportChatUrl = configuredHttpUrl(import.meta.env.VITE_SUPPORT_CHAT_URL)
const supportEmail = configuredEmail(import.meta.env.VITE_SUPPORT_EMAIL)

const faqs: HelpFaq[] = [
  {
    id: 'deposit',
    icon: Info,
    titleKey: 'helpSupport.depositTitle',
    summaryKey: 'helpSupport.depositSummary',
    answerKey: 'helpSupport.depositAnswer',
  },
  {
    id: 'withdrawal-security',
    icon: ShieldCheck,
    titleKey: 'helpSupport.withdrawalTitle',
    summaryKey: 'helpSupport.withdrawalSummary',
    answerKey: 'helpSupport.withdrawalAnswer',
  },
  {
    id: 'account-transfer',
    icon: ArrowLeftRight,
    titleKey: 'helpSupport.transferTitle',
    summaryKey: 'helpSupport.transferSummary',
    answerKey: 'helpSupport.transferAnswer',
  },
]

const filteredFaqs = computed(() => {
  const keyword = query.value.trim().toLocaleLowerCase()
  if (!keyword) return faqs
  return faqs.filter((faq) => [faq.titleKey, faq.summaryKey, faq.answerKey]
    .some((key) => t(key).toLocaleLowerCase().includes(keyword)))
})

function configuredHttpUrl(value: unknown): string {
  const candidate = String(value ?? '').trim()
  if (!candidate) return ''
  try {
    const url = new URL(candidate)
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.toString() : ''
  } catch {
    return ''
  }
}

function configuredEmail(value: unknown): string {
  const candidate = String(value ?? '').trim()
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(candidate) ? candidate : ''
}

function toggleFaq(id: string): void {
  expandedFaqId.value = expandedFaqId.value === id ? '' : id
}

function openChat(): void {
  if (!supportChatUrl) return
  window.open(supportChatUrl, '_blank', 'noopener,noreferrer')
}

function openEmail(): void {
  if (!supportEmail) return
  window.location.assign(`mailto:${supportEmail}`)
}
</script>

<template>
  <main
    class="page page--plain pencil-page help-support-page"
    data-pencil-source="UouET FM5tp"
  >
    <PageHeader
      :back="true"
      :fallback="{ name: 'profile' }"
      :pencil="true"
      :title="t('helpSupport.title')"
    />

    <section class="help-support-hero">
      <h1>{{ t('helpSupport.heroTitle') }}</h1>
      <p>{{ t('helpSupport.heroDescription') }}</p>
    </section>

    <div class="help-support-search-wrap">
      <label class="help-support-search">
        <span class="sr-only">{{ t('helpSupport.searchLabel') }}</span>
        <Search :size="16" aria-hidden="true" />
        <input v-model="query" type="search" :placeholder="t('helpSupport.searchPlaceholder')">
      </label>
    </div>

    <div class="help-support-groups">
      <section class="help-support-group">
        <h2>{{ t('helpSupport.faqHeading') }}</h2>
        <div v-if="filteredFaqs.length" class="help-faq-list">
          <article v-for="faq in filteredFaqs" :key="faq.id" class="help-faq-item">
            <button
              class="help-support-row"
              type="button"
              :aria-expanded="expandedFaqId === faq.id"
              :aria-controls="`help-faq-answer-${faq.id}`"
              @click="toggleFaq(faq.id)"
            >
              <component :is="faq.icon" :size="18" aria-hidden="true" />
              <span class="help-support-row__copy">
                <strong>{{ t(faq.titleKey) }}</strong>
                <small>{{ t(faq.summaryKey) }}</small>
              </span>
              <ChevronDown v-if="expandedFaqId === faq.id" :size="16" aria-hidden="true" />
              <ChevronRight v-else :size="16" aria-hidden="true" />
            </button>
            <p
              v-show="expandedFaqId === faq.id"
              :id="`help-faq-answer-${faq.id}`"
              class="help-faq-answer"
            >
              {{ t(faq.answerKey) }}
            </p>
          </article>
        </div>
        <div v-else class="help-search-empty" role="status">
          <Search :size="22" aria-hidden="true" />
          <strong>{{ t('helpSupport.noResults') }}</strong>
          <span>{{ t('helpSupport.noResultsDescription') }}</span>
        </div>
      </section>

      <section class="help-support-group help-contact-group">
        <h2>{{ t('helpSupport.contactHeading') }}</h2>
        <button
          class="help-support-row"
          type="button"
          :disabled="!supportChatUrl"
          @click="openChat"
        >
          <MessageCircle :size="18" aria-hidden="true" />
          <span class="help-support-row__copy">
            <strong>{{ t('helpSupport.chatTitle') }}</strong>
            <small>{{ supportChatUrl ? t('helpSupport.channelConfigured') : t('helpSupport.channelUnavailable') }}</small>
          </span>
          <span class="help-support-row__status">
            {{ supportChatUrl ? t('helpSupport.openChannel') : t('helpSupport.unavailableShort') }}
            <ChevronRight v-if="supportChatUrl" :size="16" aria-hidden="true" />
          </span>
        </button>
        <button
          class="help-support-row"
          type="button"
          :disabled="!supportEmail"
          @click="openEmail"
        >
          <Mail :size="18" aria-hidden="true" />
          <span class="help-support-row__copy">
            <strong>{{ t('helpSupport.emailTitle') }}</strong>
            <small>{{ supportEmail || t('helpSupport.channelUnavailable') }}</small>
          </span>
          <span class="help-support-row__status">
            {{ supportEmail ? t('helpSupport.openChannel') : t('helpSupport.unavailableShort') }}
            <ChevronRight v-if="supportEmail" :size="16" aria-hidden="true" />
          </span>
        </button>
      </section>
    </div>
  </main>
</template>

<style scoped>
.help-support-page {
  background: var(--page);
  min-width: 0;
  overflow-x: clip;
  padding-bottom: calc(24px + env(safe-area-inset-bottom));
}

.help-support-hero {
  display: grid;
  gap: 8px;
  padding: 16px 20px 8px;
}

.help-support-hero h1 {
  color: var(--ink);
  font-size: 22px;
  font-weight: 750;
  line-height: 28px;
  margin: 0;
}

.help-support-hero p {
  color: var(--muted);
  font-size: 12px;
  line-height: 18px;
  margin: 0;
  max-width: 340px;
}

.help-support-search-wrap {
  padding: 0 16px;
}

.help-support-search {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 4px;
  color: var(--muted);
  display: grid;
  gap: 8px;
  grid-template-columns: 16px minmax(0, 1fr);
  height: 44px;
  padding: 0 12px;
}

.help-support-search:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 2px var(--focus-ring);
}

.help-support-search input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 12px;
  height: 42px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.help-support-search input::placeholder {
  color: var(--muted);
}

.help-support-groups {
  display: grid;
  gap: 14px;
  padding: 12px 20px 0;
}

.help-support-group {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.help-support-group h2 {
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
  line-height: 16px;
  margin: 0;
}

.help-faq-list,
.help-contact-group {
  min-width: 0;
}

.help-faq-item {
  border-bottom: 1px solid var(--hairline);
}

.help-support-row {
  align-items: center;
  background: var(--surface);
  border: 0;
  border-bottom: 1px solid var(--hairline);
  color: var(--ink);
  display: grid;
  gap: 12px;
  grid-template-columns: 18px minmax(0, 1fr) auto;
  height: 64px;
  min-height: 64px;
  min-width: 0;
  padding: 0 16px;
  text-align: left;
  width: 100%;
}

.help-faq-item .help-support-row {
  border-bottom: 0;
}

.help-support-row__copy {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.help-support-row__copy strong {
  font-size: 13px;
  font-weight: 650;
  line-height: 18px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.help-support-row__copy small {
  color: var(--muted);
  font-size: 10px;
  line-height: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.help-support-row__status {
  align-items: center;
  color: var(--positive);
  display: inline-flex;
  font-size: 11px;
  font-weight: 600;
  gap: 2px;
  white-space: nowrap;
}

.help-support-row:disabled {
  color: var(--muted);
  cursor: default;
  opacity: 1;
}

.help-support-row:disabled .help-support-row__status {
  color: var(--muted);
}

.help-support-row:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: -2px;
}

.help-faq-answer {
  color: var(--muted-strong);
  font-size: 11px;
  line-height: 17px;
  margin: 0;
  padding: 0 16px 14px 46px;
}

.help-search-empty {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
  min-height: 156px;
  padding: 24px 20px;
  text-align: center;
}

.help-search-empty strong {
  color: var(--ink);
  font-size: 14px;
}

.help-search-empty span {
  font-size: 11px;
  line-height: 17px;
}

@media (max-width: 340px) {
  .help-support-hero,
  .help-support-groups {
    padding-inline: 16px;
  }

  .help-support-row {
    gap: 10px;
    padding-inline: 12px;
  }

  .help-support-row__status {
    font-size: 10px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .help-support-page *,
  .help-support-page *::before,
  .help-support-page *::after {
    scroll-behavior: auto !important;
    transition: none !important;
  }
}
</style>
