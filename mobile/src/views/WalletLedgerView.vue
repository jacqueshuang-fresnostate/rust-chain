<script setup lang="ts">
// ============================================================================
// 资金账单（钱包流水）页面
// 职责：在"交易记录"框架的账单 Tab 下展示钱包账单列表，提供
//   币种 / 交易类型 / 收支方向 / 日期 四类筛选、分页加载与失败重试；
//   底部筛选弹层复用全局模态工具（焦点圈定、Escape 关闭、焦点还原）。
// 边界：数据映射、去重、分页推进、过期请求隔离等核心逻辑都在
//   src/core/walletLedger.ts（经 @/api/wallet 转出），本组件只做状态接线与展示。
// ============================================================================

// Vue 组合式 API：computed 派生只读状态、ref 声明可变状态、
// watch 监听会话变化、onBeforeUnmount 在卸载前停掉后台控制器
import { computed, onBeforeUnmount, ref, watch } from 'vue'
// Lucide 图标：Check 选中勾、ChevronDown 下拉箭头、CircleAlert 警告圆圈、
// ListFilter 更多筛选、LoaderCircle 加载圈、X 关闭
import {
  Check,
  ChevronDown,
  CircleAlert,
  ListFilter,
  LoaderCircle,
  X,
} from 'lucide-vue-next'
// i18n：locale 是当前语言的响应式引用（用于建立语言切换依赖），t 是翻译函数
import { useI18n } from 'vue-i18n'
// 资产图标：按币种符号渲染图标，目录里没有图时走默认占位
import AssetMark from '@/components/AssetMark.vue'
// 未登录占位组件
import LoginRequiredState from '@/components/LoginRequiredState.vue'
// 账单空状态占位组件
import TransactionRecordEmptyState from '@/components/TransactionRecordEmptyState.vue'
// 交易记录页框架：四栏 Tab（含账单 Tab）+ 返回回退行为
import TransactionRecordsLayout from '@/components/TransactionRecordsLayout.vue'
// API 客户端工具：把任意请求异常转换为用户可读的 i18n 文案
import { apiErrorMessage } from '@/api/client'
// 账单领域层（@/api/wallet 再导出 src/core/walletLedger.ts）：
//   createWalletLedgerAssetDirectoryRequestLifecycle：资产目录请求生命周期（隔离过期响应）
//   createWalletLedgerPaginationController：分页控制器（首页 / 加载更多 / 重试 / 过期隔离）
//   fetchWalletAccounts / fetchWalletLedger：资产目录与账单分页接口
//   formatWalletLedgerDecimal：按资产精度 + locale 的十进制格式化
//   isWalletLedgerContractError：判断是否为后端契约错误（数据不合规）
//   WALLET_LEDGER_DATE_PRESETS / DIRECTIONS / FILTERS：筛选选项的固定枚举
//   walletLedger* 系列函数：文案 key、金额符号、方向推导、条目身份键、
//   手续费扣减值、类型展示、日期区间换算
//   type WalletLedger*：领域类型
import {
  createWalletLedgerAssetDirectoryRequestLifecycle,
  createWalletLedgerPaginationController,
  fetchWalletAccounts,
  fetchWalletLedger,
  formatWalletLedgerDecimal,
  isWalletLedgerContractError,
  WALLET_LEDGER_DATE_PRESETS,
  WALLET_LEDGER_DIRECTIONS,
  WALLET_LEDGER_FILTERS,
  walletLedgerAccountTranslationKey,
  walletLedgerAmountSign,
  walletLedgerCategoryTranslationKey,
  walletLedgerDatePresetTranslationKey,
  walletLedgerDateRange,
  walletLedgerDirectionForAmount,
  walletLedgerDirectionTranslationKey,
  walletLedgerEntryIdentity,
  walletLedgerFeeDebitAmount,
  walletLedgerTypePresentation,
  type WalletLedgerDatePreset,
  type WalletLedgerDateRange,
  type WalletLedgerDirection,
  type WalletLedgerEntry,
  type WalletLedgerFilter,
} from '@/api/wallet'
// 直接读取 i18n 模块的当前 locale（金额格式化需要 BCP47 语言标签）
import { currentIntlLocale } from '@/i18n'
// 十进制工具：decimalSign 返回 1/0/-1（判断手续费是否非零）；
// DecimalText 是规范化十进制字符串类型
import { decimalSign, type DecimalText } from '@/core/decimal'
// 模态弹层工具：弹层打开时圈定 Tab 焦点、Escape 关闭、锁背景滚动
import { useModalDialog } from '@/core/modalDialog'
// 会话 store：token 判断登录态；generation 是会话代际（登录/登出/切换时递增），
// 供控制器把旧会话发出的响应判为过期
import { useSessionStore } from '@/stores/session'

// 每页条数，与后端分页契约保持一致
const PAGE_SIZE = 30
// 底部筛选弹层的种类：'asset' 币种、'category' 交易类型、'more' 更多（方向+日期）
type FilterSheet = 'asset' | 'category' | 'more'

// 会话状态：登录 token 与会话代际
const session = useSessionStore()
// 当前语言引用与翻译函数
const { locale, t } = useI18n()
// 已加载的账单条目列表（跨页累积）
const entries = ref<WalletLedgerEntry[]>([])
// 当前选中的币种筛选（undefined 表示"全部资产"）
const activeAssetSymbol = ref<string>()
// 当前选中的交易类型筛选（'all' 表示全部）
const activeCategory = ref<WalletLedgerFilter>('all')
// 当前选中的收支方向筛选（'all' 表示全部）
const activeDirection = ref<WalletLedgerDirection>('all')
// 当前选中的日期预设（'all' 表示全部时间）
const activeDatePreset = ref<WalletLedgerDatePreset>('all')
// 日期预设换算出的具体起止区间（ISO 文本，作为请求参数）
const activeDateRange = ref<WalletLedgerDateRange>(walletLedgerDateRange('all'))
// 资产目录：可筛选的币种符号列表
const walletAssetSymbols = ref<string[]>([])
// 资产目录：币种符号 → 图标地址的映射
const walletAssetLogoUrls = ref<Record<string, string>>({})
// 资产目录请求的加载/错误状态（弹层内的状态行使用）
const assetDirectoryLoading = ref(false)
const assetDirectoryError = ref('')
// 首页加载中 / 加载更多进行中 / 数据已取完（不再显示加载更多按钮）
const loading = ref(false)
const loadingMore = ref(false)
const exhausted = ref(false)
// 首页加载错误 / 追加（加载更多）错误：分别隔离，互不覆盖
const initialError = ref<unknown | null>(null)
const appendError = ref<unknown | null>(null)
// 当前打开的筛选弹层种类；null 表示全部关闭
const openSheet = ref<FilterSheet | null>(null)
// 派生：是否有弹层打开（驱动遮罩渲染与焦点管理）
const filterSheetOpen = computed(() => openSheet.value !== null)
// 弹层本体 DOM 引用（焦点圈定的容器）
const filterDialog = ref<HTMLElement | null>(null)
// 三个筛选触发按钮的 DOM 引用（弹层关闭后把焦点还原到对应触发器）
const assetTrigger = ref<HTMLElement | null>(null)
const categoryTrigger = ref<HTMLElement | null>(null)
const moreTrigger = ref<HTMLElement | null>(null)

// 分页控制器：把"首页加载 / 加载更多 / 失败重试 / 过期请求隔离"收敛到核心层。
// 各 getter 每次取值都读取最新的会话与筛选状态，控制器据此判定响应是否过期；
// onChange 把核心层的不可变快照同步回上面的响应式状态
const paginationController = createWalletLedgerPaginationController({
  sessionKey: () => session.token,
  sessionGeneration: () => session.generation,
  selectedAssetSymbol: () => activeAssetSymbol.value,
  selectedCategory: () => activeCategory.value,
  selectedDirection: () => activeDirection.value,
  selectedDatePreset: () => activeDatePreset.value,
  selectedDateRange: () => activeDateRange.value,
  fetchPage: fetchWalletLedger,
  pageSize: PAGE_SIZE,
  onChange: (state) => {
    entries.value = state.entries
    loading.value = state.loading
    loadingMore.value = state.loadingMore
    exhausted.value = state.exhausted
    initialError.value = state.initialError
    appendError.value = state.appendError
  },
})
// 资产目录请求生命周期：同样隔离乱序响应、会话代际、未登录与卸载
const assetDirectoryController = createWalletLedgerAssetDirectoryRequestLifecycle({
  sessionKey: () => session.token,
  sessionGeneration: () => session.generation,
  fetchDirectory: () => fetchWalletAccounts(),
})
// 弹层无障碍：trapFocus 处理 Tab 循环与 Escape；setReturnFocus 记录触发器；
// '[data-dialog-initial]' 指定弹层打开时初始焦点落在标记元素上
const { trapFocus: trapFilterFocus, setReturnFocus } = useModalDialog(
  filterSheetOpen,
  filterDialog,
  '[data-dialog-initial]',
)

// 弹层标题：按当前打开的弹层种类返回对应文案
const filterSheetTitle = computed(() => {
  if (openSheet.value === 'asset') return t('ledger.assetPickerTitle')
  if (openSheet.value === 'category') return t('ledger.categoryPickerTitle')
  return t('ledger.morePickerTitle')
})

// 弹层头部"当前筛选"摘要：资产/分类弹层显示各自当前值，
// 更多弹层显示"方向 + 日期"的组合文案
const currentFilterLabel = computed(() => {
  if (openSheet.value === 'asset') return assetSheetLabel(activeAssetSymbol.value)
  if (openSheet.value === 'category') return categoryLabel(activeCategory.value)
  return t('ledger.moreFilterSummary', {
    direction: directionLabel(activeDirection.value),
    date: dateSheetLabel(activeDatePreset.value),
  })
})

// 页面级错误文案：已有数据时优先显示追加错误（首页内容仍然可用），
// 否则显示首页加载错误；内部再做契约错误与请求异常的区分
const error = computed(() => ledgerErrorMessage(
  entries.value.length ? appendError.value : initialError.value,
))

// 错误文案转换：null → 空串（无错误）；后端契约错误 → 统一"加载失败"文案；
// 其余请求异常交给 apiErrorMessage 生成可读文案
function ledgerErrorMessage(reason: unknown | null): string {
  if (reason === null) return ''
  const fallback = t('ledger.loadFailed')
  return isWalletLedgerContractError(reason) ? fallback : apiErrorMessage(reason, fallback)
}

// 加载入口：reset=true 走首页加载（筛选变化/重试首页都用它）；
// 否则在"追加失败重试"与"正常加载更多"之间二选一
async function load(reset = true): Promise<void> {
  if (reset) {
    await paginationController.loadInitial()
    return
  }
  if (appendError.value) await paginationController.retryLoadMore()
  else await paginationController.loadMore()
}

// 拉取资产目录（币种符号 + 图标地址）：
//   stale：响应已过期（会话已切换），保持现状直接返回（loading 不回退）；
//   guest：未登录 → 清空目录；
//   error：写入弹层错误文案；
//   成功：写入符号列表与图标映射
async function loadWalletAssetSymbols(): Promise<void> {
  assetDirectoryLoading.value = true
  assetDirectoryError.value = ''
  const result = await assetDirectoryController.load()
  if (result.state === 'stale') return

  assetDirectoryLoading.value = false
  if (result.state === 'guest') {
    walletAssetSymbols.value = []
    walletAssetLogoUrls.value = {}
    return
  }
  if (result.state === 'error') {
    assetDirectoryError.value = apiErrorMessage(result.error, t('ledger.assetLoadFailed'))
    return
  }
  walletAssetSymbols.value = result.value.symbols
  walletAssetLogoUrls.value = result.value.logoUrls
}

// 筛选变化后的统一动作：重置分页状态、关闭弹层、重新首页加载
function reloadForFilterChange(): void {
  paginationController.reset()
  closeFilterSheet()
  void load()
}

// 选择币种：与当前值相同则只关弹层；否则更新筛选并重载
function selectAsset(symbol?: string): void {
  if (symbol === activeAssetSymbol.value) {
    closeFilterSheet()
    return
  }
  activeAssetSymbol.value = symbol
  reloadForFilterChange()
}

// 选择收支方向：同上
function selectDirection(direction: WalletLedgerDirection): void {
  if (direction === activeDirection.value) {
    closeFilterSheet()
    return
  }
  activeDirection.value = direction
  reloadForFilterChange()
}

// 选择交易类型：同上
function selectCategory(category: WalletLedgerFilter): void {
  if (category === activeCategory.value) {
    closeFilterSheet()
    return
  }
  activeCategory.value = category
  reloadForFilterChange()
}

// 选择日期预设：同上，并把预设换算成具体的起止区间
function selectDate(preset: WalletLedgerDatePreset): void {
  if (preset === activeDatePreset.value) {
    closeFilterSheet()
    return
  }
  activeDatePreset.value = preset
  activeDateRange.value = walletLedgerDateRange(preset)
  reloadForFilterChange()
}

// 打开指定弹层：未登录直接忽略；先记录触发按钮（供焦点还原），再打开
function openFilterSheet(kind: FilterSheet): void {
  if (!session.isAuthenticated) return
  const trigger = kind === 'asset'
    ? assetTrigger.value
    : kind === 'category' ? categoryTrigger.value : moreTrigger.value
  setReturnFocus(trigger)
  openSheet.value = kind
}

// 关闭弹层（openSheet 置空，模板随即卸载弹层；焦点还原由模态工具完成）
function closeFilterSheet(): void {
  openSheet.value = null
}

// 弹层键盘事件转发：Tab 在弹层内循环、Escape 关闭
function handleFilterKeydown(event: KeyboardEvent): void {
  trapFilterFocus(event, closeFilterSheet)
}

// 触发器文案：未选具体币种时显示"币种"占位
function assetTriggerLabel(symbol?: string): string {
  return symbol || t('ledger.currencyFilterTrigger')
}

// 弹层内文案：未选时显示"全部资产"
function assetSheetLabel(symbol?: string): string {
  return symbol || t('ledger.assetAll')
}

// 收支方向文案
function directionLabel(direction: WalletLedgerDirection): string {
  return t(walletLedgerDirectionTranslationKey(direction))
}

// 交易类型文案
function categoryLabel(category: WalletLedgerFilter): string {
  return t(walletLedgerCategoryTranslationKey(category))
}

// 交易类型触发器文案：'all' 显示"交易类型"占位，其余显示具体类型名
function categoryTriggerLabel(category: WalletLedgerFilter): string {
  return category === 'all' ? t('ledger.transactionTypeFilterTrigger') : categoryLabel(category)
}

// 日期触发器文案：'all' 显示"日期"占位
function dateLabel(preset: WalletLedgerDatePreset): string {
  return preset === 'all'
    ? t('ledger.dateFilterTrigger')
    : t(walletLedgerDatePresetTranslationKey(preset))
}

// 弹层内日期文案：'all' 显示"全部日期"
function dateSheetLabel(preset: WalletLedgerDatePreset): string {
  return preset === 'all' ? t('ledger.dateAll') : dateLabel(preset)
}

// 无障碍标签："筛选名：当前选中值"
function filterSelectionLabel(filter: string, value: string): string {
  return t('ledger.filterSelectionLabel', { filter, value })
}

// 条目类型文案：变动类型映射到 i18n key
function entryLabel(entry: WalletLedgerEntry): string {
  return t(walletLedgerTypePresentation(entry.changeType).translationKey)
}

// 条目来源小字：展示层给的来源描述；缺失时退回原始变动类型串
function entryExecutionMeta(entry: WalletLedgerEntry): string {
  const source = walletLedgerTypePresentation(entry.changeType).source
  return source || entry.changeType
}

// 交易对位置文案：账单没有交易对语义，展示与条目类型一致（保住 Pencil 版式字段）
// 注意：函数体保持"只有 return"的形状（回归用正则锁定，体内不能加注释）
function entryPair(entry: WalletLedgerEntry): string {
  return entryLabel(entry)
}

// 条目币种图标：从资产目录映射里查，查不到返回 undefined（AssetMark 走默认占位）
function entryLogoUrl(entry: WalletLedgerEntry): string | undefined {
  return walletAssetLogoUrls.value[entry.symbol]
}

// 条目时间：按本地时区格式化为 年/月/日 时:分:秒；
// 读取 locale.value 是为了建立响应依赖：语言切换时时间文本重新求值
function entryTime(entry: WalletLedgerEntry): string {
  void locale.value
  const date = new Date(entry.createdAt)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}/${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

// 金额/余额的可见文本：按条目的权威精度与当前 locale 做十进制格式化；
// 读取 locale.value 建立语言切换的响应依赖
function ledgerDecimal(value: DecimalText, precisionScale: number, assetSymbol: string): string {
  void locale.value
  return formatWalletLedgerDecimal(value, currentIntlLocale(), precisionScale, assetSymbol)
}

// 带符号总额：正数前加 "+"；负数自带 "-"；极小值（"<0.00000001" 这类）不再加符号，
// 避免出现 "+<" 的怪异组合
function signedAmount(entry: WalletLedgerEntry): string {
  const amount = ledgerDecimal(entry.amount, entry.precisionScale, entry.symbol)
  return `${amount.startsWith('<') ? '' : walletLedgerAmountSign(entry.amount)}${amount}`
}

// 数量列：当前账单契约只暴露净账户变动额，没有权威的成交毛数量，
// 因此按合同固定显示 "--"（保留 Pencil 版式字段，但不把净额冒充成交数量）
function quantity(entry: WalletLedgerEntry): string {
  // 当前账单契约只暴露净变动额，没有权威的成交毛数量；
  // 保留 Pencil 版式字段，但不把净变动额冒充为交易数量。
  void entry
  return '--'
}

// 收支方向文案：由权威金额符号推导（正=收入，负=支出，零=无方向显示 "--"）
function entryDirectionLabel(entry: WalletLedgerEntry): string {
  const direction = walletLedgerDirectionForAmount(entry.amount)
  return direction ? directionLabel(direction) : '--'
}

// 总额的语义色三态：正数→收入绿（is-buy），负数→支出红（is-sell），
// 零→中性墨色（is-ink）；方向的权威来源是金额符号本身
function directionTone(entry: WalletLedgerEntry): 'is-buy' | 'is-sell' | 'is-ink' {
  const direction = walletLedgerDirectionForAmount(entry.amount)
  return direction === 'credit' ? 'is-buy' : direction === 'debit' ? 'is-sell' : 'is-ink'
}

// 手续费可见文本：已知非零时显示十进制扣减值；未知（0/-0）显示 "--"
function feeAmount(entry: WalletLedgerEntry): string {
  return feeIsKnown(entry)
    ? ledgerDecimal(walletLedgerFeeDebitAmount(entry.fee), entry.precisionScale, entry.symbol)
    : '--'
}

// 手续费精确值（title 提示）：原始扣减值 + 币种符号
function exactFeeAmount(entry: WalletLedgerEntry): string {
  return feeIsKnown(entry) ? `${walletLedgerFeeDebitAmount(entry.fee)} ${entry.symbol}` : '--'
}

// 手续费是否已知：符号非零即已知（0 与 -0 都视为未知）
function feeIsKnown(entry: WalletLedgerEntry): boolean {
  return decimalSign(entry.fee) !== 0
}

// 手续费颜色：未知为零 → 中性（is-ink）；非零 → 支出红（is-sell）
function feeTone(entry: WalletLedgerEntry): 'is-sell' | 'is-ink' {
  return decimalSign(entry.fee) === 0 ? 'is-ink' : 'is-sell'
}

// 总额 title：展示后端原始精确金额，避免可见文本精度截断造成歧义
function exactAmountTitle(entry: WalletLedgerEntry): string {
  return t('ledger.amountExact', { amount: entry.amount, symbol: entry.symbol })
}

// 数量 title：与可见文本一致，固定 "--"
function exactQuantityTitle(entry: WalletLedgerEntry): string {
  void entry
  return '--'
}

// 整行无障碍描述：类型、币种、金额、余额、手续费、账户、时间拼成一句，
// 供账单行的 aria-label 使用
function entryAccessibleDetails(entry: WalletLedgerEntry): string {
  return t('ledger.entryDetails', {
    type: entryLabel(entry),
    asset: entry.symbol,
    amount: `${entry.amount} ${entry.symbol}`,
    balance: `${entry.balanceAfter} ${entry.symbol}`,
    fee: exactFeeAmount(entry),
    account: t(walletLedgerAccountTranslationKey(entry.accountType)),
    time: entryTime(entry),
  })
}

// 会话重置：清空两个控制器的状态与缓存、关闭弹层、
// 所有筛选恢复默认值、清空资产目录与错误状态
function resetSessionState(): void {
  paginationController.reset()
  assetDirectoryController.invalidate()
  closeFilterSheet()
  activeAssetSymbol.value = undefined
  activeCategory.value = 'all'
  activeDirection.value = 'all'
  activeDatePreset.value = 'all'
  activeDateRange.value = walletLedgerDateRange('all')
  walletAssetSymbols.value = []
  walletAssetLogoUrls.value = {}
  assetDirectoryLoading.value = false
  assetDirectoryError.value = ''
}

// 监听会话 token 与代际：任一变化都整体重置；
// 已登录则拉取资产目录 + 首页账单；immediate 保证组件挂载时立即按当前会话执行一次
watch(() => [session.token, session.generation] as const, ([token]) => {
  resetSessionState()
  if (token) {
    void loadWalletAssetSymbols()
    void load()
  }
}, { immediate: true })

// 卸载清理：停掉两个控制器，之后的响应一律不再写回状态
onBeforeUnmount(() => {
  assetDirectoryController.stop()
  paginationController.stop()
})
</script>

<template>
  <!-- 页面骨架：复用交易记录页框架（当前高亮"账单"Tab），返回回退到资产页；
       data-pencil-source 记录 Pencil 设计稿节点编号，供设计比对回归定位 -->
  <TransactionRecordsLayout
    class="wallet-pencil-page wallet-ledger-pencil"
    active-tab="ledger"
    :back-fallback="{ name: 'assets' }"
    data-pencil-source="kcP5D A85if"
  >
    <!-- 顶部筛选栏：币种、交易类型两个下拉触发器 + 右侧"更多筛选"（方向/日期） -->
    <nav class="ledger-filter-bar" :aria-label="t('ledger.filterBarLabel')">
      <!-- 币种筛选触发器：选中具体币种时高亮；未登录禁用；
           带 dialog 语义与展开状态；点击打开资产弹层 -->
      <button
        ref="assetTrigger"
        class="ledger-filter-trigger"
        :class="{ 'is-active': Boolean(activeAssetSymbol) }"
        type="button"
        :disabled="!session.isAuthenticated"
        :aria-label="filterSelectionLabel(t('ledger.currencyFilterTrigger'), assetSheetLabel(activeAssetSymbol))"
        aria-haspopup="dialog"
        :aria-expanded="openSheet === 'asset'"
        @click="openFilterSheet('asset')"
      >
        <span>{{ assetTriggerLabel(activeAssetSymbol) }}</span>
        <ChevronDown :size="16" aria-hidden="true" />
      </button>
      <!-- 交易类型筛选触发器：非"全部"时高亮 -->
      <button
        ref="categoryTrigger"
        class="ledger-filter-trigger"
        :class="{ 'is-active': activeCategory !== 'all' }"
        type="button"
        :disabled="!session.isAuthenticated"
        :aria-label="filterSelectionLabel(t('ledger.transactionTypeFilterTrigger'), categoryLabel(activeCategory))"
        aria-haspopup="dialog"
        :aria-expanded="openSheet === 'category'"
        @click="openFilterSheet('category')"
      >
        <span>{{ categoryTriggerLabel(activeCategory) }}</span>
        <ChevronDown :size="16" aria-hidden="true" />
      </button>
      <!-- 弹性占位：把"更多筛选"按钮推到最右 -->
      <span class="ledger-filter-bar__spacer" aria-hidden="true" />
      <!-- 更多筛选触发器：方向或日期非默认值时高亮；纯图标按钮 -->
      <button
        ref="moreTrigger"
        class="ledger-filter-more"
        :class="{ 'is-active': activeDirection !== 'all' || activeDatePreset !== 'all' }"
        type="button"
        :disabled="!session.isAuthenticated"
        :aria-label="t('ledger.morePickerTitle')"
        aria-haspopup="dialog"
        :aria-expanded="openSheet === 'more'"
        @click="openFilterSheet('more')"
      >
        <ListFilter :size="24" aria-hidden="true" />
      </button>
    </nav>

    <!-- 内容区：按"未登录 → 首页错误 → 首页加载中 → 列表 → 空状态"互斥分支渲染 -->
    <div class="ledger-content">
      <!-- 未登录：登录引导占位 -->
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('ledger.loginDescription')"
      />
      <template v-else>
        <!-- 首页加载失败且无数据：整页错误态（警示图标 + 服务不可用 + 重试） -->
        <div v-if="error && !entries.length" class="ledger-state ledger-state--error" role="alert">
          <span class="ledger-state__plate"><CircleAlert :size="24" aria-hidden="true" /></span>
          <strong>{{ t('common.serviceUnavailable') }}</strong>
          <span>{{ error }}</span>
          <button type="button" :disabled="loading" @click="load()">{{ t('common.retry') }}</button>
        </div>
        <!-- 首页加载中且无数据：加载态 -->
        <div v-else-if="loading && !entries.length" class="ledger-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('ledger.loading') }}</span>
        </div>
        <!-- 账单列表：每条记录一个 article 列表项；
             key 用 accountType:id 复合身份（现货与杠杆的相同数字 id 不冲突）；
             aria-label 提供整行完整信息 -->
        <div v-else-if="entries.length" class="ledger-list" role="list">
          <article
            v-for="entry in entries"
            :key="walletLedgerEntryIdentity(entry)"
            class="ledger-row"
            role="listitem"
            :aria-label="entryAccessibleDetails(entry)"
          >
            <!-- 行头：左侧币种图标 + 符号；右侧带符号总额
                 （按金额符号上方向色，title 展示原始精确金额） -->
            <header class="ledger-row__header">
              <div class="ledger-row__asset">
                <AssetMark :symbol="entry.symbol" :src="entryLogoUrl(entry)" :size="30" />
                <strong>{{ entry.symbol }}</strong>
              </div>
              <strong class="ledger-row__total numeric" :class="directionTone(entry)" :title="exactAmountTitle(entry)">
                {{ signedAmount(entry) }}
              </strong>
            </header>

            <!-- 明细区：类型标题 / 数量 / 账户·方向·来源 / 手续费 -->
            <div class="ledger-row__details">
              <!-- 条目类型（占据交易对的位置） -->
              <strong class="ledger-row__pair" :title="entryPair(entry)">{{ entryPair(entry) }}</strong>
              <!-- 数量：按合同固定展示占位符 -->
              <div class="ledger-row__quantity">
                <span>{{ t('ledger.quantity') }}</span>
                <strong class="numeric" :title="exactQuantityTitle(entry)">{{ quantity(entry) }}</strong>
              </div>

              <!-- 账户类型 · 收支方向 · 来源小字 -->
              <div class="ledger-row__execution">
                <span>{{ t(walletLedgerAccountTranslationKey(entry.accountType)) }} ·</span>
                <strong :class="directionTone(entry)">{{ entryDirectionLabel(entry) }}</strong>
                <small :title="entryExecutionMeta(entry)">{{ entryExecutionMeta(entry) }}</small>
              </div>
              <!-- 手续费：非零显示十进制扣减值并标支出红，未知显示占位符 -->
              <div class="ledger-row__fee">
                <span>{{ t('ledger.feeLabel') }}</span>
                <strong class="numeric" :class="feeTone(entry)" :title="exactFeeAmount(entry)">
                  {{ feeAmount(entry) }}
                </strong>
              </div>
            </div>

            <!-- 行脚：左侧时间 + 右侧变动后余额 -->
            <footer class="ledger-row__footer">
              <!-- time 元素：datetime 属性给机器可读 UTC，可见文本是本地时区格式 -->
              <time class="numeric" :datetime="new Date(entry.createdAt).toISOString()" :title="entryTime(entry)">{{ entryTime(entry) }}</time>
              <!-- 变动后余额：可见文本按资产精度格式化，title 保留原始精确值 -->
              <div class="ledger-row__balance">
                <span>{{ t('ledger.accountBalance') }}</span>
                <strong class="numeric" :title="`${entry.balanceAfter} ${entry.symbol}`">
                  {{ ledgerDecimal(entry.balanceAfter, entry.precisionScale, entry.symbol) }}
                </strong>
              </div>
            </footer>
          </article>
        </div>
        <!-- 无数据：空状态占位 -->
        <TransactionRecordEmptyState v-else :title="t('ledger.empty')" :description="t('ledger.emptyDescription')" />

        <!-- 加载更多失败（已有数据时）：列表下方内联错误条 + 重试 -->
        <div v-if="error && entries.length" class="ledger-inline-error" role="alert">
          <CircleAlert :size="16" aria-hidden="true" />
          <span>{{ error }}</span>
          <button type="button" :disabled="loadingMore" @click="load(false)">{{ t('common.retry') }}</button>
        </div>
        <!-- 加载更多按钮：首页加载中、已取完或无数据时隐藏；
             追加进行中禁用并把文案换成"加载中" -->
        <button
          v-if="!loading && !exhausted && entries.length"
          class="ledger-load-more"
          type="button"
          :aria-busy="loadingMore"
          :disabled="loadingMore"
          @click="load(false)"
        >
          {{ loadingMore ? t('common.loading') : t('common.loadMore') }}
        </button>
      </template>
    </div>

    <!-- 筛选弹层 Teleport 到 body：避免受父级 overflow 裁剪 -->
    <Teleport to="body">
      <!-- 遮罩：点击空白处关闭（.self 保证只拦截遮罩自身，不拦截弹层内部点击） -->
      <div v-if="filterSheetOpen" class="pencil-sheet-mask ledger-filter-mask" @click.self="closeFilterSheet">
        <!-- 弹层本体：对话框语义 + 模态标记 + 标题关联（aria-labelledby）；
             tabindex=-1 允许编程聚焦；keydown 交给焦点圈定工具 -->
        <section
          ref="filterDialog"
          class="pencil-sheet ledger-filter-sheet"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="`ledger-${openSheet}-filter-title`"
          tabindex="-1"
          @keydown="handleFilterKeydown"
        >
          <!-- 顶部拖动指示条（纯视觉装饰） -->
          <div class="pencil-sheet__handle" aria-hidden="true" />
          <!-- 弹层头：动态标题 + 当前筛选摘要 + 关闭按钮 -->
          <header>
            <div class="ledger-filter-sheet__heading">
              <h2 :id="`ledger-${openSheet}-filter-title`">{{ filterSheetTitle }}</h2>
              <p>{{ t('ledger.filterCurrent', { value: currentFilterLabel }) }}</p>
            </div>
            <button class="ledger-filter-sheet__close" type="button" :aria-label="t('ledger.filterClose')" @click="closeFilterSheet">
              <X :size="20" aria-hidden="true" />
            </button>
          </header>

          <!-- 币种选择列表：第一项"全部资产"，其余来自资产目录；
               选中项高亮并打勾；data-dialog-initial 标记初始焦点应落在当前选中项 -->
          <div v-if="openSheet === 'asset'" class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.assetPickerTitle') })">
            <button type="button" :class="{ 'is-selected': !activeAssetSymbol }" :aria-pressed="!activeAssetSymbol" :data-dialog-initial="!activeAssetSymbol ? '' : undefined" @click="selectAsset()">
              <span>{{ t('ledger.assetAll') }}</span><Check v-if="!activeAssetSymbol" :size="18" aria-hidden="true" />
            </button>
            <button v-for="symbol in walletAssetSymbols" :key="symbol" type="button" :class="{ 'is-selected': activeAssetSymbol === symbol }" :aria-pressed="activeAssetSymbol === symbol" :data-dialog-initial="activeAssetSymbol === symbol ? '' : undefined" @click="selectAsset(symbol)">
              <span>{{ symbol }}</span><Check v-if="activeAssetSymbol === symbol" :size="18" aria-hidden="true" />
            </button>
            <!-- 资产目录的三种状态行：加载中 / 失败（可重试）/ 为空 -->
            <div v-if="assetDirectoryLoading" class="ledger-filter-sheet__state" role="status">
              <LoaderCircle :size="18" class="spin" aria-hidden="true" /><span>{{ t('ledger.assetLoading') }}</span>
            </div>
            <div v-else-if="assetDirectoryError" class="ledger-filter-sheet__state" role="alert">
              <span>{{ assetDirectoryError }}</span><button type="button" @click="loadWalletAssetSymbols">{{ t('common.retry') }}</button>
            </div>
            <p v-else-if="!walletAssetSymbols.length" class="ledger-filter-sheet__state" role="status">{{ t('ledger.assetEmpty') }}</p>
          </div>

          <!-- 交易类型选择列表：固定分类全集逐项渲染 -->
          <div v-else-if="openSheet === 'category'" class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.categoryPickerTitle') })">
            <button v-for="category in WALLET_LEDGER_FILTERS" :key="category" type="button" :class="{ 'is-selected': activeCategory === category }" :aria-pressed="activeCategory === category" :data-dialog-initial="activeCategory === category ? '' : undefined" @click="selectCategory(category)">
              <span>{{ categoryLabel(category) }}</span><Check v-if="activeCategory === category" :size="18" aria-hidden="true" />
            </button>
          </div>

          <!-- 更多筛选弹层内容：方向与日期两组 -->
          <div v-else class="ledger-more-filters">
            <!-- 收支方向组：全部/收入/支出 -->
            <section>
              <h3>{{ t('ledger.directionPickerTitle') }}</h3>
              <div class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.directionPickerTitle') })">
                <button v-for="direction in WALLET_LEDGER_DIRECTIONS" :key="direction" type="button" :class="{ 'is-selected': activeDirection === direction }" :aria-pressed="activeDirection === direction" :data-dialog-initial="activeDirection === direction ? '' : undefined" @click="selectDirection(direction)">
                  <span>{{ directionLabel(direction) }}</span><Check v-if="activeDirection === direction" :size="18" aria-hidden="true" />
                </button>
              </div>
            </section>
            <!-- 日期预设组：全部/今天/近7天/近30天 -->
            <section>
              <h3>{{ t('ledger.datePickerTitle') }}</h3>
              <div class="ledger-filter-options" role="list" :aria-label="t('ledger.filterOptionsLabel', { filter: t('ledger.datePickerTitle') })">
                <button v-for="preset in WALLET_LEDGER_DATE_PRESETS" :key="preset" type="button" :class="{ 'is-selected': activeDatePreset === preset }" :aria-pressed="activeDatePreset === preset" @click="selectDate(preset)">
                  <span>{{ preset === 'all' ? t('ledger.dateAll') : dateLabel(preset) }}</span><Check v-if="activeDatePreset === preset" :size="18" aria-hidden="true" />
                </button>
              </div>
            </section>
          </div>
        </section>
      </div>
    </Teleport>
  </TransactionRecordsLayout>
</template>

<style scoped>
/* 账单页专属色板（亮色为默认值）：画布/卡片/铬层全白、墨色正文、
   浅分隔线、灰次级文字；前三个变量把语义色映射给弹层复用的通用名 */
.wallet-ledger-pencil {
  --ink: var(--wallet-record-ink);
  --muted: var(--wallet-record-row-muted);
  --positive: var(--wallet-record-buy);
  --wallet-record-active: #18d38d;
  --wallet-record-buy: #0dbe7b;
  --wallet-record-canvas: #ffffff;
  --wallet-record-card: #ffffff;
  --wallet-record-chrome: #ffffff;
  --wallet-record-ink: #111714;
  --wallet-record-row-line: #edf1ef;
  --wallet-record-row-muted: #8a948f;
  --wallet-record-sell: #ff5878;
  --wallet-record-tab-line: #eef1ef;
  --wallet-record-tab-muted: #7b8680;
  background: var(--wallet-record-canvas);
  color: var(--wallet-record-ink);
  min-width: 0;
  overflow-x: clip;
}

/* 暗色主题：只覆盖色值，同名变量；纯黑画布、亮绿买入色、浅色正文 */
:global(html[data-theme='dark'] .wallet-ledger-pencil) {
  --wallet-record-buy: #45efae;
  --wallet-record-canvas: #000000;
  --wallet-record-card: #000000;
  --wallet-record-chrome: #000000;
  --wallet-record-ink: #f3f7f5;
  --wallet-record-row-line: #17221c;
  --wallet-record-row-muted: #8f9b94;
  --wallet-record-tab-line: #18231d;
  --wallet-record-tab-muted: #8f9b94;
}

/* 顶部筛选栏：水平排列、58px 高、24px 间距、铬层底色 */
.ledger-filter-bar {
  align-items: center;
  background: var(--wallet-record-chrome);
  box-sizing: border-box;
  display: flex;
  gap: 24px;
  height: 58px;
  min-height: 58px;
  min-width: 0;
  padding: 0 16px;
}

/* 两个触发器按钮的公共基座：去默认边框底色、44px 触达高度 */
.ledger-filter-trigger,
.ledger-filter-more {
  background: transparent;
  border: 0;
  color: var(--wallet-record-ink);
  height: 44px;
  min-height: 44px;
  padding: 0;
}

/* 文字触发器排版：行内 flex、16px 半粗、间距 8px、整体可收缩不换行 */
.ledger-filter-trigger {
  align-items: center;
  display: inline-flex;
  flex: 0 1 auto;
  font-size: 16px;
  font-weight: 600;
  gap: 8px;
  line-height: 22px;
  min-width: 0;
  white-space: nowrap;
}

/* 触发器文本：允许收缩并显示省略号 */
.ledger-filter-trigger span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 下拉箭头图标：不被压缩 */
.ledger-filter-trigger svg {
  flex: 0 0 auto;
}

/* 弹性占位：把"更多筛选"按钮推到最右 */
.ledger-filter-bar__spacer {
  flex: 1 1 auto;
  height: 1px;
  min-width: 0;
}

/* 更多筛选按钮：44px 方形、内容居中 */
.ledger-filter-more {
  display: grid;
  flex: 0 0 44px;
  place-items: center;
  width: 44px;
}

/* 触发器激活态：用主题强调色 */
.ledger-filter-trigger.is-active,
.ledger-filter-more.is-active {
  color: var(--wallet-record-active);
}

/* 禁用态（未登录）：保持原样不弱化，也不显示禁用光标 */
.ledger-filter-trigger:disabled,
.ledger-filter-more:disabled {
  cursor: default;
  opacity: 1;
}

/* 键盘焦点环：所有可点控件统一 2px 描边，去掉默认 outline */
.ledger-filter-trigger:focus-visible,
.ledger-filter-more:focus-visible,
.ledger-state button:focus-visible,
.ledger-inline-error button:focus-visible,
.ledger-load-more:focus-visible,
.ledger-filter-sheet button:focus-visible {
  box-shadow: 0 0 0 2px var(--focus-ring);
  outline: 0;
}

/* 内容区：画布底色、横向裁剪、底部留白叠加 iOS 安全区 */
.ledger-content {
  background: var(--wallet-record-canvas);
  min-width: 0;
  overflow-x: clip;
  padding-bottom: calc(28px + env(safe-area-inset-bottom));
}

/* 列表容器：块级、零内边距（正式通栏外框） */
.ledger-list {
  box-sizing: border-box;
  display: block;
  min-width: 0;
  padding: 0;
}

/* 单条账单行：通栏卡片、底部 1px 分隔线、无圆角无阴影；
   三行网格（行头/明细/行脚）、行距 9px、最小高 190px、内边距 12px 18px */
.ledger-row {
  align-items: stretch;
  background: var(--wallet-record-card);
  border: 0;
  border-bottom: 1px solid var(--wallet-record-row-line);
  border-radius: 0;
  box-sizing: border-box;
  box-shadow: none;
  display: grid;
  gap: 9px;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto auto auto;
  justify-content: stretch;
  min-height: 190px;
  min-width: 0;
  overflow: hidden;
  padding: 12px 18px;
  width: 100%;
}

/* 行头两栏网格：资产列 0.8fr、总额列 1.2fr，最小高 30px */
.ledger-row__header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.2fr);
  min-height: 30px;
  min-width: 0;
}

/* 三个区块的网格子项都允许收缩（防止长内容把布局撑破） */
.ledger-row__header > *,
.ledger-row__details > *,
.ledger-row__footer > * {
  min-width: 0;
}

/* 明细区：两栏等宽网格、间距 8px 16px */
.ledger-row__details {
  display: grid;
  gap: 8px 16px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin-top: 0;
  min-width: 0;
  padding-top: 0;
}

/* 币种图标 + 符号：左对齐、溢出隐藏 */
.ledger-row__asset {
  align-items: center;
  display: flex;
  flex: 1 1 auto;
  gap: 9px;
  overflow: hidden;
}

/* 币种符号：20px 半粗、单行省略 */
.ledger-row__asset strong {
  color: var(--wallet-record-ink);
  font-size: 20px;
  font-weight: 650;
  line-height: 28px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 总额数字：18px、右对齐、单行省略；
   文字颜色默认墨色，动态语义类（买入/卖出/中性）随后覆盖 */
.ledger-row__total {
  color: var(--wallet-record-ink);
  font-size: 18px;
  font-weight: 500;
  line-height: 24px;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 条目类型标题（占交易对位置）：15px 半粗、垂直居中、单行省略 */
.ledger-row__pair {
  align-self: center;
  color: var(--wallet-record-ink);
  display: block;
  font-size: 15px;
  font-weight: 600;
  line-height: 22px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 数量/手续费/余额三列的公共结构：标签 + 值的两行网格 */
.ledger-row__quantity,
.ledger-row__fee,
.ledger-row__balance {
  align-items: center;
  display: grid;
  min-width: 0;
}

/* 数量列：标签与值左右排布、间距 6px */
.ledger-row__quantity {
  gap: 6px;
  grid-template-columns: auto minmax(0, 1fr);
}

/* 数量标签：13px 灰色 */
.ledger-row__quantity > span {
  color: var(--wallet-record-row-muted);
  font-size: 13px;
  font-weight: 500;
  line-height: 19px;
}

/* 数量值：15px 右对齐、单行省略 */
.ledger-row__quantity strong {
  color: var(--wallet-record-ink);
  font-size: 15px;
  font-weight: 500;
  line-height: 22px;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 账户/方向/来源块：两行网格（上行 20px：账户+方向；下行 18px：来源） */
.ledger-row__execution {
  align-items: center;
  column-gap: 4px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  grid-template-rows: 20px 18px;
  min-width: 0;
  overflow: hidden;
}

/* 账户类型：14px 半粗、不换行 */
.ledger-row__execution > span {
  color: var(--wallet-record-ink);
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  white-space: nowrap;
}

/* 收支方向文字：15px 半粗、单行省略（颜色由语义类决定） */
.ledger-row__execution > strong {
  font-size: 15px;
  font-weight: 650;
  line-height: 22px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 来源小字：独占整行、12px 灰色、单行省略 */
.ledger-row__execution > small {
  color: var(--wallet-record-row-muted);
  font-size: 12px;
  font-weight: 500;
  grid-column: 1 / -1;
  line-height: 18px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 手续费列：右对齐、两行（标签 16px 高、值 18px 高） */
.ledger-row__fee {
  align-content: start;
  gap: 2px;
  grid-template-rows: 16px 18px;
  justify-items: end;
}

/* 手续费标签：12px 灰色 */
.ledger-row__fee span {
  color: var(--wallet-record-row-muted);
  font-size: 12px;
  font-weight: 500;
  line-height: 18px;
}

/* 手续费值：12px 右对齐、单行省略 */
.ledger-row__fee strong {
  font-size: 12px;
  font-weight: 500;
  line-height: 18px;
  max-width: 100%;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 行脚：两栏等宽网格、底部对齐 */
.ledger-row__footer {
  align-items: end;
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin-top: 0;
  min-width: 0;
  padding-top: 0;
}

/* 时间：13px 灰色、单行省略 */
.ledger-row__footer time {
  color: var(--wallet-record-row-muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 19px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 余额列：标签与值左右排布、间距 6px */
.ledger-row__balance {
  gap: 6px;
  grid-template-columns: auto minmax(0, 1fr);
}

/* 余额标签：13px 灰色 */
.ledger-row__balance span {
  color: var(--wallet-record-row-muted);
  font-size: 13px;
  font-weight: 500;
  line-height: 19px;
}

/* 余额值：14px 右对齐、单行省略 */
.ledger-row__balance strong {
  color: var(--wallet-record-ink);
  font-size: 14px;
  font-weight: 500;
  line-height: 19px;
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 语义色三态：收入绿 / 支出红 / 中性墨色。
   三条规则必须位于同等优先级的默认色规则之后（同优先级下后者生效） */
.is-buy {
  color: var(--wallet-record-buy);
}

.is-sell {
  color: var(--wallet-record-sell);
}

.is-ink {
  color: var(--wallet-record-ink);
}

/* 数字统一使用等宽字体 + 表格数字，保证纵向对齐稳定 */
.numeric {
  font-family: var(--font-geist-mono), var(--data-font);
  font-variant-numeric: tabular-nums;
}

/* 加载态：居中排列、最小高 180px、灰色文字 */
.ledger-loading {
  align-items: center;
  color: var(--wallet-record-row-muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

/* 整页状态容器（错误等）：纵向居中、最小高 225px、内边距 48px 20px */
.ledger-state {
  align-items: center;
  color: var(--wallet-record-row-muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 12px;
  justify-content: center;
  min-height: 225px;
  padding: 48px 20px;
  text-align: center;
}

/* 状态图标底座：56px 圆形、浅色面、细边框 */
.ledger-state__plate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--wallet-record-row-line);
  border-radius: 50%;
  color: var(--wallet-record-row-muted);
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

/* 状态主文案：15px 半粗墨色 */
.ledger-state strong {
  color: var(--wallet-record-ink);
  font-size: 15px;
  font-weight: 650;
  line-height: 20px;
}

/* 次级说明文案：限宽 300px */
.ledger-state > span:last-child {
  line-height: 17px;
  max-width: 300px;
}

/* 错误态：图标底座与主文案使用支出红 */
.ledger-state--error .ledger-state__plate,
.ledger-state--error strong {
  color: var(--wallet-record-sell);
}

/* 状态区按钮与加载更多：胶囊形、透明底、细边框、强调色文字、44px 触达 */
.ledger-state button,
.ledger-load-more {
  background: transparent;
  border: 1px solid var(--wallet-record-row-line);
  border-radius: 999px;
  color: var(--wallet-record-active);
  font-size: 11px;
  min-height: 44px;
  padding: 0 18px;
}

/* 内联错误条：软红底、圆角、三列网格（图标/文本/按钮）、最小高 44px */
.ledger-inline-error {
  align-items: center;
  background: var(--negative-soft);
  border-radius: 12px;
  color: var(--wallet-record-sell);
  display: grid;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 18px minmax(0, 1fr) auto;
  margin: 10px 16px 0;
  min-height: 44px;
  padding: 0 8px 0 10px;
}

/* 错误文本：允许任意断行防止溢出 */
.ledger-inline-error span {
  min-width: 0;
  overflow-wrap: anywhere;
}

/* 内联重试按钮：透明底、继承红色 */
.ledger-inline-error button {
  background: transparent;
  color: inherit;
  min-height: 44px;
  padding: 0 8px;
}

/* 加载更多按钮：左右 16px 外距、宽度 = 容器宽 - 32px */
.ledger-load-more {
  margin: 10px 16px 0;
  width: calc(100% - 32px);
}

/* 弹层遮罩：弹层水平居中 */
.ledger-filter-mask {
  justify-items: center;
}

/* 筛选弹层：宽度上限 448px，文字用通用墨色 */
.ledger-filter-sheet {
  --muted: var(--wallet-ledger-muted);
  color: var(--ink);
  max-width: 448px;
}

/* 弹层头：标题与关闭按钮间距 12px */
.ledger-filter-sheet > header {
  gap: 12px;
}

/* 弹层标题区：纵排、间距 1px */
.ledger-filter-sheet__heading {
  display: grid;
  gap: 1px;
  min-width: 0;
}

/* 弹层标题：18px 半粗 */
.ledger-filter-sheet__heading h2 {
  color: var(--ink);
  font-size: 18px;
  font-weight: 650;
  line-height: 24px;
}

/* 当前筛选摘要：11px 灰色 */
.ledger-filter-sheet__heading p {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
  margin: 0;
}

/* 弹层关闭按钮：44px 方形、内容居中 */
.ledger-filter-sheet__close {
  align-items: center;
  background: transparent;
  color: var(--ink);
  display: inline-flex;
  flex: 0 0 44px;
  height: 44px;
  justify-content: center;
  padding: 0;
  width: 44px;
}

/* 筛选选项列表容器 */
.ledger-filter-options {
  display: grid;
  min-width: 0;
}

/* 每个选项行：最小高 56px、底部分隔线、左文右勾两列、13px 半粗 */
.ledger-filter-options > button {
  align-items: center;
  background: transparent;
  border-bottom: 1px solid var(--hairline);
  color: var(--ink);
  display: grid;
  font-size: 13px;
  font-weight: 600;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) 20px;
  min-height: 56px;
  padding: 0 4px;
  text-align: left;
}

/* 选中项：文字用正向绿 */
.ledger-filter-options > button.is-selected {
  color: var(--positive);
}

/* 更多筛选内容：两组纵排、间距 18px、限高可滚动 */
.ledger-more-filters {
  display: grid;
  gap: 18px;
  max-height: min(520px, calc(100dvh - 190px - env(safe-area-inset-bottom)));
  min-width: 0;
  overflow-y: auto;
}

/* 组容器允许收缩 */
.ledger-more-filters section {
  min-width: 0;
}

/* 组标题：12px 灰色、底部留 4px */
.ledger-more-filters h3 {
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
  line-height: 18px;
  margin: 0;
  padding: 0 4px 4px;
}

/* 资产目录状态行（加载中/失败/为空）：居中、最小高 72px */
.ledger-filter-sheet__state {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 12px;
  gap: 8px;
  justify-content: center;
  margin: 0;
  min-height: 72px;
  text-align: center;
}

/* 目录重试按钮：透明底、正向绿 */
.ledger-filter-sheet__state button {
  background: transparent;
  color: var(--positive);
  min-height: 44px;
  padding: 0 8px;
}

/* 未登录提示条：透明底、顶部分隔线、三列网格（图标/文案/按钮） */
.wallet-login-prompt {
  background: transparent;
  background-image: none;
  border: 0;
  border-top: 1px solid var(--wallet-record-row-line);
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) auto;
  margin: 0 18px;
  min-height: 72px;
  padding: 10px 0;
}

/* 提示图标：34px 方形、强调底色、正向绿图标 */
.wallet-login-prompt :deep(.login-required__icon) {
  background: var(--accent-soft);
  border: 0;
  color: var(--positive);
  height: 34px;
  width: 34px;
}

/* 提示文案组：间距 2px */
.wallet-login-prompt :deep(.login-required__copy) {
  gap: 2px;
}

/* 提示主文案：13px */
.wallet-login-prompt :deep(.login-required__copy strong) {
  font-size: 13px;
}

/* 提示说明文案：11px 灰色 */
.wallet-login-prompt :deep(.login-required__copy p) {
  color: var(--wallet-record-row-muted);
  font-size: 11px;
  line-height: 1.4;
}

/* 登录按钮：胶囊形、44px 触达高度 */
.wallet-login-prompt :deep(.button) {
  border-radius: 999px;
  min-height: 44px;
  padding-inline: 14px;
}

/* 加载圈旋转动画：0.8 秒一圈、线性循环 */
.spin {
  animation: spin .8s linear infinite;
}

/* 旋转动画帧：转到 360° */
@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 窄屏（≤340px）适配：压缩行头列宽、明细与行脚间距，
   行脚改单列，未登录提示按钮独占一行 */
@media (max-width: 340px) {
  .ledger-list {
    padding-inline: 0;
  }

  .ledger-row {
    padding: 12px 18px;
  }

  .ledger-row__header {
    gap: 10px;
    grid-template-columns: minmax(0, 0.78fr) minmax(0, 1.22fr);
  }

  .ledger-row__details {
    gap: 8px 12px;
  }

  .ledger-row__footer {
    gap: 6px;
    grid-template-columns: minmax(0, 1fr);
  }

  .ledger-row__footer time {
    font-size: 12px;
  }

  .ledger-row__balance {
    width: 100%;
  }

  .wallet-login-prompt {
    align-items: center;
    grid-template-columns: 34px minmax(0, 1fr);
  }

  .wallet-login-prompt :deep(.button) {
    grid-column: 1 / -1;
    width: 100%;
  }
}

/* 减少动态偏好：关闭加载圈旋转动画 */
@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
