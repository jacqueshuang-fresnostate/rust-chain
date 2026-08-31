import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { createMemoryHistory, createRouter } from 'vue-router'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { goBackOr } from '../src/core/navigation.ts'
import {
  SUPPORT_RECONCILE_INTERVAL_MS,
  createSupportClientMessageId,
  createSupportPollingController,
  createSupportSendAttempt,
  executeSupportSendAttempt,
  mergeSupportHistoryPage,
  resolveSupportChatViewState,
  type SupportPollingScheduler,
} from '../src/core/supportChat.ts'
import type { Message } from '../src/api/support.ts'

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), 'utf8')
const routerSource = read('../src/router/index.ts')
const helpSource = read('../src/views/HelpSupportView.vue')
const chatSource = read('../src/views/SupportChatView.vue')
const supportApiSource = read('../src/api/support.ts')
const supportCoreSource = read('../src/core/supportChat.ts')

test('在线客服使用内部懒路由并安全回退到帮助页', () => {
  assert.match(routerSource, /const SupportChatView = \(\) => import\('@\/views\/SupportChatView\.vue'\)/)
  assert.match(
    routerSource,
    /path: '\/profile\/help\/chat', name: 'support-chat', component: SupportChatView, meta: \{ showBottomNav: false, depth: 2, backFallback: '\/profile\/help' \}/,
  )
  assert.match(chatSource, /<PageHeader[\s\S]*fallback="\/profile\/help"[\s\S]*:title="t\('supportChat\.title'\)"/)
})

test('直开客服页时返回动作 replace 到内部帮助页', async () => {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/profile/help', component: {} },
      { path: '/profile/help/chat', component: {} },
    ],
  })
  await router.replace('/profile/help/chat')
  await goBackOr(router, '/profile/help')
  assert.equal(router.currentRoute.value.fullPath, '/profile/help')
})

test('帮助页在线客服入口始终进入站内会话并保留次要邮箱渠道', () => {
  assert.match(helpSource, /const router = useRouter\(\)/)
  assert.match(helpSource, /void router\.push\(\{ name: 'support-chat' \}\)/)
  assert.match(helpSource, /t\('helpSupport\.chatDescription'\)/)
  assert.match(helpSource, /import\.meta\.env\.VITE_SUPPORT_EMAIL/)
  assert.match(helpSource, /window\.location\.assign\(`mailto:/)
  assert.doesNotMatch(helpSource, /supportChatUrl|configuredHttpUrl|window\.open|target=["']_blank/)
})

test('移动端客服 API 使用受保护客户端和精确 REST 合同', () => {
  assert.match(supportApiSource, /client\.get<CurrentSupportConversationResponse>\(requestUrl\('\/support\/conversation'\)\)/)
  assert.match(supportApiSource, /client\.get<SupportMessagesResponse>\(requestUrl\('\/support\/conversation\/messages'\), \{/)
  assert.match(supportApiSource, /before_id: pagination\.beforeId/)
  assert.match(supportApiSource, /limit: pagination\.limit/)
  assert.match(supportApiSource, /client\.post<SendMessageResult>\(requestUrl\('\/support\/conversation\/messages'\)/)
  assert.match(supportApiSource, /client_message_id: input\.clientMessageId/)
  assert.match(supportApiSource, /requestUrl\('\/support\/conversation\/read'\)/)
  assert.match(supportApiSource, /message_id: messageId/)
  assert.match(supportApiSource, /client\.patch<Conversation>\(requestUrl\('\/support\/conversation\/status'\)/)
  assert.match(supportApiSource, /'user' \| 'agent' \| 'admin'/)
  assert.doesNotMatch(supportApiSource, /\| 'system'/)
})

test('访客分支显示登录状态且不会请求客服会话', () => {
  assert.equal(resolveSupportChatViewState({
    authenticated: false,
    loading: true,
    failed: true,
    messageCount: 4,
  }), 'guest')
  assert.match(chatSource, /v-if="!session\.isAuthenticated" class="support-chat-guest"/)
  assert.match(chatSource, /<LoginRequiredState :description="t\('supportChat\.loginDescription'\)"/)
  assert.match(chatSource, /if \(!session\.isAuthenticated\) return/)
})

test('失败重试复用同一个 client_message_id', async () => {
  const firstGenerated = createSupportClientMessageId(1_700_000_000_000, 'first-entropy')
  const secondGenerated = createSupportClientMessageId(1_700_000_000_000, 'second-entropy')
  assert.match(firstGenerated, /^[A-Za-z0-9_-]{8,64}$/)
  assert.notEqual(firstGenerated, secondGenerated)

  const attempt = createSupportSendAttempt('  Keep this immutable  ', () => 'mobile-fixed-token-01')
  const calls: Array<{ body: string, clientMessageId: string }> = []
  let callCount = 0
  const sender = async (body: string, clientMessageId: string): Promise<string> => {
    calls.push({ body, clientMessageId })
    callCount += 1
    if (callCount === 1) throw new Error('transient')
    return 'sent'
  }

  await assert.rejects(executeSupportSendAttempt(attempt, sender), /transient/)
  assert.equal(await executeSupportSendAttempt(attempt, sender), 'sent')
  assert.deepEqual(calls, [
    { body: 'Keep this immutable', clientMessageId: 'mobile-fixed-token-01' },
    { body: 'Keep this immutable', clientMessageId: 'mobile-fixed-token-01' },
  ])
  assert.match(chatSource, /function retryPendingSend\(\)[\s\S]*runSendAttempt\(attempt, sessionGeneration\)/)
  assert.match(chatSource, /executeSupportSendAttempt\(attempt/)
})

test('定时 REST 对账在卸载时清理且不会重复启动', async () => {
  const callbacks = new Map<number, () => void>()
  const cleared: unknown[] = []
  let setCount = 0
  const scheduler: SupportPollingScheduler = {
    setInterval(callback, delay) {
      assert.equal(delay, SUPPORT_RECONCILE_INTERVAL_MS)
      setCount += 1
      callbacks.set(17, callback)
      return 17
    },
    clearInterval(handle) {
      cleared.push(handle)
      callbacks.delete(Number(handle))
    },
  }
  let refreshCount = 0
  const polling = createSupportPollingController(async () => {
    refreshCount += 1
  }, SUPPORT_RECONCILE_INTERVAL_MS, scheduler)

  polling.start()
  polling.start()
  assert.equal(setCount, 1)
  callbacks.get(17)?.()
  await Promise.resolve()
  assert.equal(refreshCount, 1)
  polling.stop()
  assert.deepEqual(cleared, [17])
  assert.equal(polling.isRunning(), false)
  assert.match(chatSource, /onBeforeUnmount\(\(\) => \{[\s\S]*polling\.stop\(\)/)
})

test('客服页通过共享 support lease 消费实时提示并仅执行 REST 权威对账', () => {
  const leaseSetup = sourceSlice(
    chatSource,
    'usePrivateUserStreamLease({',
    'onBeforeUnmount(() => {',
  )
  const backgroundReconciliation = sourceSlice(
    chatSource,
    'function requestSupportBackgroundReconciliation',
    'function resetConversationState',
  )

  assert.match(chatSource, /import \{ usePrivateUserStreamLease \} from '@\/composables\/usePrivateUserStreamLease'/)
  assert.match(leaseSetup, /topic: 'support'/)
  assert.match(leaseSetup, /consumerId: 'support-chat'/)
  assert.match(leaseSetup, /enabled: \(\) => session\.isAuthenticated/)
  assert.match(leaseSetup, /onOpen: \(\) => \{ void requestSupportBackgroundReconciliation\(\) \}/)
  assert.match(leaseSetup, /onEvent: \(\) => \{ void requestSupportBackgroundReconciliation\(\) \}/)
  assert.match(backgroundReconciliation, /supportBackgroundRefreshQueued = true/)
  assert.match(backgroundReconciliation, /await reconcileConversation\(false, generation\)/)
  assert.match(chatSource, /createSupportPollingController\(requestSupportBackgroundReconciliation\)/)
  assert.match(chatSource, /watch\(\(\) => session\.generation/)
  assert.doesNotMatch(chatSource, /createPrivateUserStream|privateUserWebSocketUrl|privateUserStream\.(?:start|stop)/)
})

test('客服页面覆盖读游标、分组消息、关闭重开与完整状态', () => {
  assert.match(chatSource, /fetchCurrentSupportConversation\(\)/)
  assert.match(chatSource, /fetchSupportConversationMessages\(\{ limit: 100 \}\)/)
  assert.match(chatSource, /await nextTick\(\)[\s\S]*latestRenderedStaffMessageId\(messages\.value\)/)
  assert.match(chatSource, /await markSupportConversationRead\(nextMessageId\)/)
  assert.match(chatSource, /groupSupportMessages\(messages\.value\)/)
  assert.match(chatSource, /patchSupportConversationStatus\(nextStatus\)/)
  assert.match(chatSource, /conversation\.value = result\.conversation/)
  assert.match(chatSource, /loadOlderMessages\(\)/)
  assert.match(chatSource, /loadOlderMessages\(\)[\s\S]*markRenderedStaffMessagesRead\(generation\)/)
  assert.match(chatSource, /olderMessagesLoading/)
  assert.match(chatSource, /olderMessagesError/)
  assert.match(chatSource, /t\('supportChat\.retryOlderMessages'\)/)
  assert.match(chatSource, /support-chat-state--empty/)
  assert.match(chatSource, /sendState === 'failed'/)
  assert.match(chatSource, /env\(safe-area-inset-bottom\)/)
  assert.match(chatSource, /height: 44px;/)
  assert.match(chatSource, /min-height: 44px;/)
  assert.doesNotMatch(chatSource, /<svg|\p{Extended_Pictographic}/u)
})

test('生产移动端不再引用外部客服 URL 且中英文键完整对称', () => {
  const mobileSource = collectSourceFiles(fileURLToPath(new URL('../src', import.meta.url)))
  assert.doesNotMatch(mobileSource, /VITE_SUPPORT_CHAT_URL/)
  assert.doesNotMatch(`${helpSource}\n${chatSource}`, /https?:\/\//)

  const keys = new Set<string>()
  for (const source of [helpSource, chatSource]) {
    for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1])
  }
  for (const key of keys) {
    assert.notEqual(resolveMessage(zhCN, key), undefined, `zh-CN missing ${key}`)
    assert.notEqual(resolveMessage(en, key), undefined, `en missing ${key}`)
  }
  assert.deepEqual(Object.keys(zhCN.supportChat).sort(), Object.keys(en.supportChat).sort())
  assert.match(supportCoreSource, /SUPPORT_RECONCILE_INTERVAL_MS = 5_000/)
})

test('更早消息分页按游标合并、去重并保持升序', () => {
  const message = (id: number, createdAt: number, body: string): Message => ({
    id,
    conversation_id: 7,
    sender_type: 'user',
    sender_id: 9,
    client_message_id: `mobile-page-${id}`,
    body,
    read_by_recipient: false,
    created_at: createdAt,
  })
  const latest = mergeSupportHistoryPage([], {
    messages: [message(102, 2_000, '边界'), message(103, 3_000, '最新')],
    has_more: true,
    next_before_id: 102,
  })
  assert.equal(latest.hasMore, true)
  assert.equal(latest.nextBeforeId, 102)

  const complete = mergeSupportHistoryPage(latest.messages, {
    messages: [message(101, 1_000, '更早'), message(102, 2_000, '边界')],
    has_more: false,
    next_before_id: null,
  })
  assert.deepEqual(complete.messages.map((item) => item.id), [101, 102, 103])
  assert.equal(complete.hasMore, false)
  assert.equal(complete.nextBeforeId, null)
})

function collectSourceFiles(root: string): string {
  const chunks: string[] = []
  for (const entry of readdirSync(root)) {
    const path = `${root}/${entry}`
    if (statSync(path).isDirectory()) chunks.push(collectSourceFiles(path))
    else if (/\.(?:ts|vue|css)$/.test(entry)) chunks.push(readFileSync(path, 'utf8'))
  }
  return chunks.join('\n')
}

function resolveMessage(messages: unknown, key: string): unknown {
  return key.split('.').reduce<unknown>((value, segment) => {
    if (!value || typeof value !== 'object') return undefined
    return (value as Record<string, unknown>)[segment]
  }, messages)
}

function sourceSlice(source: string, start: string, end: string): string {
  const startIndex = source.indexOf(start)
  const endIndex = source.indexOf(end, startIndex)
  assert.ok(startIndex >= 0 && endIndex > startIndex, `missing source slice ${start} -> ${end}`)
  return source.slice(startIndex, endIndex)
}
