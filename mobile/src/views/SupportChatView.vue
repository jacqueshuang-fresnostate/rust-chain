<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  CircleAlert,
  Headphones,
  Inbox,
  LoaderCircle,
  MessageSquarePlus,
  MessageSquareX,
  RefreshCw,
  RotateCcw,
  Send,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  fetchCurrentSupportConversation,
  fetchSupportConversationMessages,
  markSupportConversationRead,
  patchSupportConversationStatus,
  postSupportConversationMessage,
  type Conversation,
  type Message,
  type SupportConversationStatus,
} from '@/api/support'
import {
  SUPPORT_MESSAGE_MAX_SCALARS,
  createSupportPollingController,
  createSupportSendAttempt,
  executeSupportSendAttempt,
  groupSupportMessages,
  latestRenderedStaffMessageId,
  mergeSupportHistoryPage,
  reconcileSupportMessages,
  resolveSupportChatViewState,
  supportMessageDayKey,
  supportMessageScalarLength,
  type SupportSendAttempt,
} from '@/core/supportChat'
import { currentIntlLocale } from '@/i18n'
import { useSessionStore } from '@/stores/session'

const session = useSessionStore()
const { t } = useI18n()
const conversation = ref<Conversation | null>(null)
const messages = ref<readonly Message[]>([])
const draft = ref('')
const initialLoading = ref(false)
const loadError = ref('')
const refreshError = ref('')
const statusError = ref('')
const readError = ref('')
const statusUpdating = ref(false)
const readUpdating = ref(false)
const pendingSend = ref<SupportSendAttempt | null>(null)
const sendState = ref<'idle' | 'sending' | 'failed'>('idle')
const sendError = ref('')
const draftError = ref('')
const threadEnd = ref<HTMLElement | null>(null)
const historyHasMore = ref(false)
const historyNextBeforeId = ref<number | null>(null)
const olderMessagesLoading = ref(false)
const olderMessagesError = ref('')

let sessionGeneration = 0
let reconciliationVersion = 0
let olderMessagesRequestVersion = 0
let historyPaginationInitialized = false
let confirmedReadMessageId = 0
let requestedReadMessageId = 0
let readLoop: Promise<void> | null = null

const groupedMessages = computed(() => groupSupportMessages(messages.value))
const draftLength = computed(() => supportMessageScalarLength(draft.value))
const isClosed = computed(() => conversation.value?.status === 'closed')
const hasUserMessages = computed(() => messages.value.some((message) => message.sender_type === 'user'))
const hasStaffMessages = computed(() => messages.value.some((message) => (
  message.sender_type === 'agent' || message.sender_type === 'admin'
)))
const lastUserMessageId = computed(() => {
  let latest: number | null = null
  for (const message of messages.value) {
    if (message.sender_type !== 'user') continue
    latest = latest === null ? message.id : Math.max(latest, message.id)
  }
  return latest
})
const viewState = computed(() => resolveSupportChatViewState({
  authenticated: session.isAuthenticated,
  loading: initialLoading.value,
  failed: Boolean(loadError.value),
  messageCount: messages.value.length,
}))
const assignmentLabel = computed(() => {
  const current = conversation.value
  if (!current) return t('supportChat.assignmentPending')
  const code = current.assigned_agent_code?.trim()
  if (code) return t('supportChat.assignedAgent', { code })
  if (current.assigned_agent_id !== null && current.assigned_agent_id !== undefined) {
    return t('supportChat.assignedAgentId', { id: current.assigned_agent_id })
  }
  return t('supportChat.unassigned')
})
const assignmentDescription = computed(() => {
  if (!conversation.value) return t('supportChat.assignmentPendingDescription')
  return conversation.value.assigned_agent_id === null
    || conversation.value.assigned_agent_id === undefined
    ? t('supportChat.unassignedDescription')
    : t('supportChat.assignedDescription')
})
const staffReadLabel = computed(() => {
  const unread = conversation.value?.staff_unread_count ?? 0
  return unread > 0
    ? t('supportChat.awaitingStaffRead', { count: unread })
    : t('supportChat.readByStaff')
})
const incomingReadLabel = computed(() => {
  if (readUpdating.value) return t('supportChat.markingRead')
  const unread = conversation.value?.user_unread_count ?? 0
  return unread > 0
    ? t('supportChat.newReplies', { count: unread })
    : t('supportChat.allRepliesRead')
})
const canSend = computed(() => (
  session.isAuthenticated
  && !initialLoading.value
  && !loadError.value
  && !pendingSend.value
  && draft.value.trim().length > 0
  && draftLength.value <= SUPPORT_MESSAGE_MAX_SCALARS
))

const polling = createSupportPollingController(async () => {
  await reconcileConversation(false, sessionGeneration)
})

function resetConversationState(): void {
  conversation.value = null
  messages.value = []
  draft.value = ''
  initialLoading.value = false
  loadError.value = ''
  refreshError.value = ''
  statusError.value = ''
  readError.value = ''
  statusUpdating.value = false
  readUpdating.value = false
  pendingSend.value = null
  sendState.value = 'idle'
  sendError.value = ''
  draftError.value = ''
  historyHasMore.value = false
  historyNextBeforeId.value = null
  olderMessagesLoading.value = false
  olderMessagesError.value = ''
  olderMessagesRequestVersion += 1
  historyPaginationInitialized = false
  confirmedReadMessageId = 0
  requestedReadMessageId = 0
  readLoop = null
}

async function activateSession(): Promise<void> {
  polling.stop()
  sessionGeneration += 1
  reconciliationVersion += 1
  const generation = sessionGeneration
  resetConversationState()
  if (!session.isAuthenticated) return
  await reconcileConversation(true, generation)
  if (generation === sessionGeneration && session.isAuthenticated) polling.start()
}

async function reconcileConversation(initial: boolean, generation: number): Promise<void> {
  if (!session.isAuthenticated || generation !== sessionGeneration) return
  const requestVersion = ++reconciliationVersion
  if (initial) initialLoading.value = true

  try {
    const current = await fetchCurrentSupportConversation()
    let incomingMessages: readonly Message[] = []
    let latestPage: Awaited<ReturnType<typeof fetchSupportConversationMessages>> | null = null
    let messagesFailure: unknown = null

    if (current.conversation) {
      try {
        latestPage = await fetchSupportConversationMessages({ limit: 100 })
        incomingMessages = latestPage.messages
      } catch (error) {
        messagesFailure = error
      }
    }

    if (
      generation !== sessionGeneration
      || requestVersion !== reconciliationVersion
      || !session.isAuthenticated
    ) return
    conversation.value = current.conversation
    if (latestPage && !historyPaginationInitialized) {
      const merged = mergeSupportHistoryPage(messages.value, latestPage)
      messages.value = merged.messages
      historyHasMore.value = merged.hasMore
      historyNextBeforeId.value = merged.nextBeforeId
      olderMessagesError.value = ''
      historyPaginationInitialized = true
    } else {
      messages.value = reconcileSupportMessages(messages.value, incomingMessages)
    }
    settlePendingSendFromMessages()
    loadError.value = ''
    refreshError.value = messagesFailure
      ? apiErrorMessage(messagesFailure, t('supportChat.messagesLoadFailed'))
      : ''
    if (initial) {
      initialLoading.value = false
      void scrollThreadToEndAfterRender()
    }
    void markRenderedStaffMessagesRead(generation)
  } catch (error) {
    if (
      generation !== sessionGeneration
      || requestVersion !== reconciliationVersion
      || !session.isAuthenticated
    ) return
    const message = apiErrorMessage(error, t('supportChat.loadFailed'))
    if (initial && !messages.value.length) loadError.value = message
    else refreshError.value = message
  } finally {
    if (initial && generation === sessionGeneration) initialLoading.value = false
  }
}

async function loadOlderMessages(): Promise<void> {
  const beforeId = historyNextBeforeId.value
  if (
    !session.isAuthenticated
    || !historyHasMore.value
    || beforeId === null
    || olderMessagesLoading.value
  ) return

  const generation = sessionGeneration
  const requestVersion = ++olderMessagesRequestVersion
  olderMessagesLoading.value = true
  olderMessagesError.value = ''
  try {
    const page = await fetchSupportConversationMessages({ beforeId, limit: 100 })
    if (
      generation !== sessionGeneration
      || requestVersion !== olderMessagesRequestVersion
      || !session.isAuthenticated
    ) return
    const merged = mergeSupportHistoryPage(messages.value, page)
    messages.value = merged.messages
    historyHasMore.value = merged.hasMore
    historyNextBeforeId.value = merged.nextBeforeId
    void markRenderedStaffMessagesRead(generation)
  } catch (error) {
    if (generation === sessionGeneration && requestVersion === olderMessagesRequestVersion) {
      olderMessagesError.value = apiErrorMessage(error, t('supportChat.olderMessagesLoadFailed'))
    }
  } finally {
    if (generation === sessionGeneration && requestVersion === olderMessagesRequestVersion) {
      olderMessagesLoading.value = false
    }
  }
}

async function markRenderedStaffMessagesRead(generation: number): Promise<void> {
  await nextTick()
  if (generation !== sessionGeneration || !session.isAuthenticated) return
  const target = latestRenderedStaffMessageId(messages.value)
  if (target === null) return
  if ((conversation.value?.user_unread_count ?? 0) <= 0) {
    confirmedReadMessageId = Math.max(confirmedReadMessageId, target)
    return
  }

  requestedReadMessageId = Math.max(requestedReadMessageId, target)
  if (readLoop) return readLoop

  readUpdating.value = true
  let currentReadLoop: Promise<void>
  currentReadLoop = (async () => {
    while (
      generation === sessionGeneration
      && session.isAuthenticated
      && requestedReadMessageId > confirmedReadMessageId
    ) {
      const nextMessageId = requestedReadMessageId
      try {
        const updatedConversation = await markSupportConversationRead(nextMessageId)
        if (generation !== sessionGeneration || !session.isAuthenticated) return
        reconciliationVersion += 1
        confirmedReadMessageId = nextMessageId
        readError.value = ''
        if (conversation.value && updatedConversation.id === conversation.value.id) {
          conversation.value = updatedConversation
        }
      } catch (error) {
        if (generation === sessionGeneration) {
          readError.value = apiErrorMessage(error, t('supportChat.readFailed'))
        }
        return
      }
    }
  })().finally(() => {
    if (generation === sessionGeneration) readUpdating.value = false
    if (readLoop === currentReadLoop) readLoop = null
  })
  readLoop = currentReadLoop
  return readLoop
}

function submitDraft(): void {
  const body = draft.value.trim()
  const length = supportMessageScalarLength(body)
  if (!body) {
    draftError.value = t('supportChat.messageRequired')
    return
  }
  if (length > SUPPORT_MESSAGE_MAX_SCALARS) {
    draftError.value = t('supportChat.messageTooLong', { max: SUPPORT_MESSAGE_MAX_SCALARS })
    return
  }

  draftError.value = ''
  const attempt = createSupportSendAttempt(body)
  pendingSend.value = attempt
  draft.value = ''
  void runSendAttempt(attempt, sessionGeneration)
}

function retryPendingSend(): void {
  const attempt = pendingSend.value
  if (!attempt || sendState.value === 'sending') return
  void runSendAttempt(attempt, sessionGeneration)
}

async function runSendAttempt(attempt: SupportSendAttempt, generation: number): Promise<void> {
  if (generation !== sessionGeneration || !session.isAuthenticated) return
  reconciliationVersion += 1
  sendState.value = 'sending'
  sendError.value = ''

  try {
    const result = await executeSupportSendAttempt(attempt, (body, clientMessageId) => (
      postSupportConversationMessage({ body, clientMessageId })
    ))
    if (generation !== sessionGeneration || !session.isAuthenticated) return
    reconciliationVersion += 1
    messages.value = reconcileSupportMessages(messages.value, [result.message])
    conversation.value = result.conversation
    pendingSend.value = null
    sendState.value = 'idle'
    await nextTick()
    threadEnd.value?.scrollIntoView({ block: 'end' })
    void reconcileConversation(false, generation)
  } catch (error) {
    if (generation !== sessionGeneration || !session.isAuthenticated) return
    if (pendingSend.value?.clientMessageId !== attempt.clientMessageId) return
    sendState.value = 'failed'
    sendError.value = apiErrorMessage(error, t('supportChat.sendFailed'))
  }
}

async function toggleConversationStatus(): Promise<void> {
  const current = conversation.value
  if (!current || statusUpdating.value) return
  const generation = sessionGeneration
  const nextStatus: SupportConversationStatus = current.status === 'closed' ? 'open' : 'closed'
  reconciliationVersion += 1
  statusUpdating.value = true
  statusError.value = ''
  try {
    const updated = await patchSupportConversationStatus(nextStatus)
    if (generation !== sessionGeneration || !session.isAuthenticated) return
    reconciliationVersion += 1
    conversation.value = updated
  } catch (error) {
    if (generation === sessionGeneration) {
      statusError.value = apiErrorMessage(error, t('supportChat.statusUpdateFailed'))
    }
  } finally {
    if (generation === sessionGeneration) statusUpdating.value = false
  }
}

function retryLoad(): void {
  void reconcileConversation(true, sessionGeneration)
}

async function scrollThreadToEndAfterRender(): Promise<void> {
  await nextTick()
  threadEnd.value?.scrollIntoView({ block: 'end' })
}

function settlePendingSendFromMessages(): void {
  const pending = pendingSend.value
  if (!pending) return
  const committed = messages.value.some((message) => (
    message.sender_type === 'user'
    && message.client_message_id === pending.clientMessageId
  ))
  if (!committed) return
  pendingSend.value = null
  sendState.value = 'idle'
  sendError.value = ''
}

function handleComposerEnter(event: KeyboardEvent): void {
  if (event.isComposing) return
  event.preventDefault()
  submitDraft()
}

function conversationStatusLabel(status: string): string {
  if (status === 'open') return t('supportChat.open')
  if (status === 'closed') return t('supportChat.closed')
  return status
}

function senderLabel(message: Message): string {
  if (message.sender_type === 'user') return t('supportChat.you')
  if (message.sender_type === 'agent') {
    const code = conversation.value?.assigned_agent_code?.trim()
    if (
      code
      && message.sender_id !== null
      && message.sender_id !== undefined
      && message.sender_id === conversation.value?.assigned_agent_id
    ) return t('supportChat.agentSender', { code })
    if (message.sender_id !== null && message.sender_id !== undefined) {
      return t('supportChat.agentSenderId', { id: message.sender_id })
    }
    return t('supportChat.agent')
  }
  if (message.sender_type === 'admin') return t('supportChat.platformSupport')
  return message.sender_type
}

function deliveryLabel(message: Message): string {
  if (message.read_by_recipient === true) return t('supportChat.readByStaff')
  if (message.read_by_recipient === false) return t('supportChat.notReadByStaff')
  return staffReadLabel.value
}

function groupLabel(dayKey: string, timestamp: number): string {
  const today = supportMessageDayKey(Date.now())
  const yesterdayDate = new Date()
  yesterdayDate.setDate(yesterdayDate.getDate() - 1)
  const yesterday = supportMessageDayKey(yesterdayDate.getTime())
  if (dayKey === today) return t('supportChat.today')
  if (dayKey === yesterday) return t('supportChat.yesterday')
  return new Intl.DateTimeFormat(currentIntlLocale(), {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  }).format(new Date(timestamp))
}

function messageTime(timestamp: number): string {
  return new Intl.DateTimeFormat(currentIntlLocale(), {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(new Date(timestamp))
}

watch(() => session.token, () => { void activateSession() })
watch(draft, () => {
  if (draftError.value) draftError.value = ''
})

onMounted(() => { void activateSession() })
onBeforeUnmount(() => {
  sessionGeneration += 1
  reconciliationVersion += 1
  polling.stop()
  olderMessagesRequestVersion += 1
  readLoop = null
})
</script>

<template>
  <main
    class="page page--plain support-chat-page"
    :data-chat-state="viewState"
    :aria-busy="initialLoading || sendState === 'sending' || statusUpdating"
  >
    <PageHeader
      :back="true"
      fallback="/profile/help"
      :pencil="true"
      :title="t('supportChat.title')"
    >
      <template #actions>
        <button
          v-if="session.isAuthenticated && conversation"
          class="icon-button"
          type="button"
          :aria-label="isClosed ? t('supportChat.reopenAction') : t('supportChat.closeAction')"
          :aria-busy="statusUpdating"
          :disabled="statusUpdating || sendState === 'sending'"
          @click="toggleConversationStatus"
        >
          <LoaderCircle v-if="statusUpdating" :size="19" class="spin" aria-hidden="true" />
          <MessageSquarePlus v-else-if="isClosed" :size="19" aria-hidden="true" />
          <MessageSquareX v-else :size="19" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>

    <section v-if="!session.isAuthenticated" class="support-chat-guest">
      <LoginRequiredState :description="t('supportChat.loginDescription')" />
    </section>

    <template v-else>
      <section
        v-if="!initialLoading && !loadError"
        class="support-chat-routing"
        :aria-label="t('supportChat.routingStatus')"
      >
        <span class="support-chat-routing__icon" aria-hidden="true">
          <Headphones :size="20" />
        </span>
        <span class="support-chat-routing__copy">
          <strong>{{ assignmentLabel }}</strong>
          <small>{{ assignmentDescription }}</small>
        </span>
        <span v-if="conversation" class="support-chat-routing__status">
          {{ conversationStatusLabel(conversation.status) }}
        </span>
      </section>

      <section v-if="initialLoading" class="support-chat-state" role="status">
        <span class="support-chat-state__plate">
          <LoaderCircle :size="25" class="spin" aria-hidden="true" />
        </span>
        <strong>{{ t('supportChat.loading') }}</strong>
        <p>{{ t('supportChat.loadingDescription') }}</p>
      </section>

      <section v-else-if="loadError" class="support-chat-state support-chat-state--error" role="alert">
        <span class="support-chat-state__plate">
          <CircleAlert :size="25" aria-hidden="true" />
        </span>
        <strong>{{ t('supportChat.loadErrorTitle') }}</strong>
        <p>{{ loadError }}</p>
        <button type="button" :disabled="initialLoading" @click="retryLoad">
          <RefreshCw :size="18" aria-hidden="true" />
          <span>{{ t('common.retry') }}</span>
        </button>
      </section>

      <template v-else>
        <div class="support-chat-notices" aria-live="polite">
          <p v-if="refreshError" class="support-chat-notice is-error">
            <CircleAlert :size="16" aria-hidden="true" />
            <span>{{ refreshError }}</span>
            <button type="button" @click="retryLoad">
              <RefreshCw :size="16" aria-hidden="true" />
              <span>{{ t('common.retry') }}</span>
            </button>
          </p>
          <p v-if="statusError" class="support-chat-notice is-error" role="alert">
            <CircleAlert :size="16" aria-hidden="true" />
            <span>{{ statusError }}</span>
          </p>
          <p v-if="readError" class="support-chat-notice is-error" role="alert">
            <CircleAlert :size="16" aria-hidden="true" />
            <span>{{ readError }}</span>
          </p>
        </div>

        <section class="support-chat-thread" :aria-label="t('supportChat.messageHistory')">
          <div
            v-if="conversation && (hasUserMessages || hasStaffMessages)"
            class="support-chat-read-state"
            role="status"
          >
            <span v-if="hasStaffMessages">{{ incomingReadLabel }}</span>
            <span v-if="hasUserMessages">{{ staffReadLabel }}</span>
          </div>

          <div
            v-if="historyHasMore || olderMessagesLoading || olderMessagesError"
            class="support-chat-history-pagination"
            aria-live="polite"
          >
            <p v-if="olderMessagesError" role="alert">{{ olderMessagesError }}</p>
            <button
              type="button"
              :disabled="olderMessagesLoading || !historyNextBeforeId"
              @click="loadOlderMessages"
            >
              <LoaderCircle v-if="olderMessagesLoading" :size="16" class="spin" aria-hidden="true" />
              <RotateCcw v-else-if="olderMessagesError" :size="16" aria-hidden="true" />
              <RefreshCw v-else :size="16" aria-hidden="true" />
              <span>
                {{ olderMessagesLoading
                  ? t('supportChat.loadingOlderMessages')
                  : olderMessagesError
                    ? t('supportChat.retryOlderMessages')
                    : t('supportChat.loadOlderMessages') }}
              </span>
            </button>
          </div>

          <section
            v-if="!messages.length && !pendingSend"
            class="support-chat-state support-chat-state--empty"
            role="status"
          >
            <span class="support-chat-state__plate">
              <Inbox :size="25" aria-hidden="true" />
            </span>
            <strong>{{ t('supportChat.emptyTitle') }}</strong>
            <p>{{ t('supportChat.emptyDescription') }}</p>
          </section>

          <section
            v-for="group in groupedMessages"
            :key="group.dayKey"
            class="support-message-group"
          >
            <h2>{{ groupLabel(group.dayKey, group.firstCreatedAt) }}</h2>
            <ol>
              <li
                v-for="message in group.messages"
                :key="message.id"
                class="support-message"
                :class="[
                  `is-${message.sender_type}`,
                ]"
              >
                <article>
                  <header>
                    <strong>{{ senderLabel(message) }}</strong>
                    <time :datetime="new Date(message.created_at).toISOString()">
                      {{ messageTime(message.created_at) }}
                    </time>
                  </header>
                  <p>{{ message.body }}</p>
                  <small
                    v-if="message.sender_type === 'user' && message.id === lastUserMessageId"
                    class="support-message__delivery"
                  >
                    {{ deliveryLabel(message) }}
                  </small>
                </article>
              </li>
            </ol>
          </section>

          <article
            v-if="pendingSend"
            class="support-message is-user support-message--pending"
            :class="{ 'is-failed': sendState === 'failed' }"
            :aria-busy="sendState === 'sending'"
          >
            <div>
              <header>
                <strong>{{ t('supportChat.you') }}</strong>
                <span>
                  {{ sendState === 'sending' ? t('supportChat.sending') : t('supportChat.sendFailedShort') }}
                </span>
              </header>
              <p>{{ pendingSend.body }}</p>
              <footer v-if="sendState === 'failed'" role="alert">
                <span>{{ sendError }}</span>
                <button type="button" @click="retryPendingSend">
                  <RotateCcw :size="17" aria-hidden="true" />
                  <span>{{ t('supportChat.retrySend') }}</span>
                </button>
              </footer>
            </div>
          </article>
          <span ref="threadEnd" class="support-chat-thread__end" aria-hidden="true" />
        </section>

        <section v-if="isClosed" class="support-chat-closed" role="status">
          <MessageSquareX :size="20" aria-hidden="true" />
          <span>
            <strong>{{ t('supportChat.closedTitle') }}</strong>
            <small>{{ t('supportChat.closedDescription') }}</small>
          </span>
          <button type="button" :disabled="statusUpdating" @click="toggleConversationStatus">
            <RefreshCw :size="17" aria-hidden="true" />
            <span>{{ t('supportChat.reopenAction') }}</span>
          </button>
        </section>

        <form class="support-chat-composer" @submit.prevent="submitDraft">
          <label class="support-chat-composer__field">
            <span class="sr-only">{{ t('supportChat.messageLabel') }}</span>
            <textarea
              v-model="draft"
              rows="1"
              :aria-describedby="draftError ? 'support-chat-draft-error' : 'support-chat-draft-meta'"
              :aria-invalid="draftError ? 'true' : undefined"
              :disabled="Boolean(pendingSend)"
              :placeholder="isClosed ? t('supportChat.closedPlaceholder') : t('supportChat.messagePlaceholder')"
              @keydown.enter.exact="handleComposerEnter"
            />
          </label>
          <button
            class="support-chat-composer__send"
            type="submit"
            :aria-label="t('supportChat.sendAction')"
            :disabled="!canSend"
          >
            <LoaderCircle v-if="sendState === 'sending'" :size="19" class="spin" aria-hidden="true" />
            <Send v-else :size="19" aria-hidden="true" />
          </button>
          <p
            v-if="draftError"
            id="support-chat-draft-error"
            class="support-chat-composer__error"
            role="alert"
          >
            {{ draftError }}
          </p>
          <p v-else id="support-chat-draft-meta" class="support-chat-composer__meta">
            <span>{{ isClosed ? t('supportChat.sendReopens') : t('supportChat.textOnly') }}</span>
            <span>{{ t('supportChat.characterCount', { count: draftLength, max: SUPPORT_MESSAGE_MAX_SCALARS }) }}</span>
          </p>
        </form>
      </template>
    </template>
  </main>
</template>

<style scoped>
.support-chat-page {
  background: var(--page);
  color: var(--text);
  display: flex;
  flex-direction: column;
  min-height: 100dvh;
  min-width: 0;
  overflow-x: clip;
}

.support-chat-guest {
  display: grid;
  flex: 1;
  padding:
    20px
    max(16px, env(safe-area-inset-right))
    calc(24px + env(safe-area-inset-bottom))
    max(16px, env(safe-area-inset-left));
}

.support-chat-routing {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 10px;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  min-height: 68px;
  padding: 10px 16px;
}

.support-chat-routing__icon {
  align-items: center;
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 28%, var(--line));
  border-radius: 50%;
  color: var(--positive);
  display: flex;
  height: 44px;
  justify-content: center;
  width: 44px;
}

.support-chat-routing__copy {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.support-chat-routing__copy strong,
.support-chat-routing__copy small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.support-chat-routing__copy strong {
  color: var(--text);
  font-size: 13px;
  line-height: 18px;
}

.support-chat-routing__copy small {
  color: var(--muted);
  font-size: 10px;
  line-height: 15px;
}

.support-chat-routing__status {
  background: var(--surface-2);
  border: 1px solid var(--line);
  color: var(--muted-strong);
  font-size: 10px;
  font-weight: 650;
  line-height: 18px;
  padding: 2px 8px;
  white-space: nowrap;
}

.support-chat-notices {
  display: grid;
}

.support-chat-notice {
  align-items: center;
  background: var(--negative-soft);
  border-bottom: 1px solid color-mix(in srgb, var(--negative) 24%, var(--line));
  color: var(--negative);
  display: grid;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  line-height: 16px;
  margin: 0;
  min-width: 0;
  padding: 8px 16px;
}

.support-chat-notice button,
.support-chat-state button,
.support-message--pending footer button,
.support-chat-closed button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  color: var(--text);
  display: inline-flex;
  font-size: 11px;
  font-weight: 650;
  gap: 6px;
  justify-content: center;
  min-height: 44px;
  padding: 0 12px;
}

.support-chat-notice button {
  min-height: 44px;
}

.support-chat-thread {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 280px;
  min-width: 0;
  padding:
    10px
    max(16px, env(safe-area-inset-right))
    16px
    max(16px, env(safe-area-inset-left));
}

.support-chat-read-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-wrap: wrap;
  font-size: 10px;
  gap: 6px 12px;
  justify-content: center;
  line-height: 16px;
  min-height: 24px;
}

.support-chat-history-pagination {
  align-items: center;
  display: grid;
  gap: 8px;
  justify-items: center;
  padding: 6px 0 12px;
}

.support-chat-history-pagination p {
  color: var(--negative);
  font-size: 10px;
  line-height: 15px;
  margin: 0;
  max-width: 100%;
  overflow-wrap: anywhere;
  text-align: center;
}

.support-chat-history-pagination button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  color: var(--muted-strong);
  display: inline-flex;
  font-size: 11px;
  font-weight: 650;
  gap: 6px;
  justify-content: center;
  min-height: 44px;
  max-width: 100%;
  padding: 0 14px;
}

.support-chat-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 10px;
  justify-content: center;
  min-height: 280px;
  padding: 36px 20px;
  text-align: center;
}

.support-chat-state__plate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--muted);
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

.support-chat-state strong {
  color: var(--text);
  font-size: 15px;
  line-height: 21px;
}

.support-chat-state p {
  font-size: 11px;
  line-height: 17px;
  margin: 0;
  max-width: 300px;
}

.support-chat-state--error .support-chat-state__plate,
.support-chat-state--error strong {
  color: var(--negative);
}

.support-message-group {
  display: grid;
  gap: 8px;
}

.support-message-group + .support-message-group {
  margin-top: 14px;
}

.support-message-group h2 {
  color: var(--muted);
  font-size: 10px;
  font-weight: 600;
  line-height: 18px;
  margin: 0;
  text-align: center;
}

.support-message-group ol {
  display: grid;
  gap: 10px;
  list-style: none;
  margin: 0;
  padding: 0;
}

.support-message {
  display: flex;
  justify-content: flex-start;
  min-width: 0;
}

.support-message.is-user,
.support-message--pending {
  justify-content: flex-end;
}

.support-message article,
.support-message--pending > div {
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 14px 14px 14px 3px;
  box-shadow: inset 0 1px 0 color-mix(in srgb, var(--text) 5%, transparent);
  display: grid;
  gap: 6px;
  max-width: min(82%, 330px);
  min-width: 0;
  padding: 10px 12px;
}

.support-message.is-user article,
.support-message--pending > div {
  background: var(--accent-soft);
  border-color: color-mix(in srgb, var(--accent) 25%, var(--line));
  border-radius: 14px 14px 3px 14px;
}

.support-message header,
.support-message--pending header {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 9px;
  gap: 10px;
  justify-content: space-between;
  line-height: 14px;
  min-width: 0;
}

.support-message header strong,
.support-message--pending header strong {
  color: var(--muted-strong);
  font-size: 10px;
  font-weight: 650;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.support-message p,
.support-message--pending p {
  color: var(--text);
  font-size: 13px;
  line-height: 19px;
  margin: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.support-message__delivery {
  color: var(--muted);
  font-size: 9px;
  line-height: 14px;
  text-align: right;
}

.support-message--pending {
  margin-top: 10px;
}

.support-message--pending.is-failed > div {
  background: var(--negative-soft);
  border-color: color-mix(in srgb, var(--negative) 28%, var(--line));
}

.support-message--pending footer {
  color: var(--negative);
  display: grid;
  font-size: 10px;
  gap: 8px;
  line-height: 15px;
}

.support-message--pending footer button {
  justify-self: end;
}

.support-chat-thread__end {
  display: block;
  height: 1px;
}

.support-chat-closed {
  align-items: center;
  background: var(--surface-2);
  border-top: 1px solid var(--line);
  display: grid;
  gap: 10px;
  grid-template-columns: 24px minmax(0, 1fr) auto;
  padding: 10px 16px;
}

.support-chat-closed > svg {
  color: var(--muted);
}

.support-chat-closed > span {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.support-chat-closed strong {
  color: var(--text);
  font-size: 12px;
  line-height: 17px;
}

.support-chat-closed small {
  color: var(--muted);
  font-size: 10px;
  line-height: 15px;
}

.support-chat-composer {
  background: color-mix(in srgb, var(--surface) 96%, transparent);
  border-top: 1px solid var(--line);
  bottom: 0;
  display: grid;
  gap: 6px 8px;
  grid-template-columns: minmax(0, 1fr) 44px;
  padding:
    10px
    max(12px, env(safe-area-inset-right))
    max(10px, env(safe-area-inset-bottom))
    max(12px, env(safe-area-inset-left));
  position: sticky;
  z-index: 20;
}

.support-chat-composer__field {
  align-items: center;
  background: var(--surface-2);
  border: 1px solid var(--line);
  display: flex;
  min-height: 44px;
  min-width: 0;
}

.support-chat-composer__field:focus-within {
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.support-chat-composer textarea {
  background: transparent;
  border: 0;
  color: var(--text);
  font: inherit;
  font-size: 13px;
  line-height: 20px;
  max-height: 104px;
  min-height: 42px;
  min-width: 0;
  outline: 0;
  padding: 11px 12px;
  resize: none;
  width: 100%;
}

.support-chat-composer textarea::placeholder {
  color: var(--muted);
}

.support-chat-composer textarea:disabled {
  cursor: wait;
  opacity: 0.7;
}

.support-chat-composer__send {
  align-items: center;
  align-self: start;
  background: var(--accent);
  border: 1px solid var(--accent);
  border-radius: 50%;
  color: var(--surface);
  display: flex;
  height: 44px;
  justify-content: center;
  min-height: 44px;
  padding: 0;
  width: 44px;
}

.support-chat-composer__send:disabled {
  background: var(--surface-3);
  border-color: var(--line);
  color: var(--muted);
}

.support-chat-composer__meta,
.support-chat-composer__error {
  display: flex;
  font-size: 9px;
  gap: 8px;
  grid-column: 1 / -1;
  justify-content: space-between;
  line-height: 14px;
  margin: 0;
}

.support-chat-composer__meta {
  color: var(--muted);
}

.support-chat-composer__error {
  color: var(--negative);
}

.support-chat-page button:focus-visible,
.support-chat-page textarea:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.support-chat-composer textarea:focus-visible {
  outline: 0;
}

.support-chat-page button:disabled {
  cursor: default;
}

@media (max-width: 340px) {
  .support-chat-routing,
  .support-chat-closed {
    padding-inline: 12px;
  }

  .support-chat-routing {
    grid-template-columns: 44px minmax(0, 1fr);
  }

  .support-chat-routing__status {
    grid-column: 2;
    justify-self: start;
  }

  .support-chat-thread {
    padding-inline: 12px;
  }

  .support-message article,
  .support-message--pending > div {
    max-width: 88%;
  }

  .support-chat-closed {
    grid-template-columns: 24px minmax(0, 1fr);
  }

  .support-chat-closed button {
    grid-column: 2;
    justify-self: start;
  }
}

@media (prefers-reduced-motion: reduce) {
  .support-chat-page *,
  .support-chat-page *::before,
  .support-chat-page *::after {
    scroll-behavior: auto !important;
    transition: none !important;
  }
}
</style>
