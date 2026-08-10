# Research: 手机端 Home 真实收益历史曲线接入

- Query: 研究 `HomeView` 的 `portfolio-chart` 如何从页面会话资产估值采样切换到后端真实收益日序列，并覆盖 Pencil/SVG、周期控件、今日收益复用、会话隔离、隐私与状态、1 日零基线、累计曲线、可访问性和测试。
- Scope: mixed（内部源码、Trellis 规范、当前 Pencil 源和官方可访问性/Vue/SVG文档）
- Date: 2026-08-09

## Findings

### 1. 结论

推荐把首页右上“今日收益”和下方历史图表维持为两个独立数据消费者：

1. “今日收益”继续使用现有 `GET /wallet/today-return`、`mapTodayReturn` 和独立请求生命周期，不从历史响应反推，避免破坏 Assets/Home 已共享的严格合同。
2. 新增 `ReturnHistory` DTO/严格适配器及 `fetchReturnHistory(periodDays)`；图表只消费该响应，删除 `portfolioSamples` 和总资产 watcher 采样。
3. 历史读取直接复用通用 `createSessionRequestLifecycle`，以精确 `session.token` 为会话键；周期切换、重试、换号、退出和卸载统一 latest-request-wins。
4. 完整历史才生成 SVG。`partial/loading/error/guest/hidden` 均不得保留或绘制上一请求曲线。
5. 由几何层在真实日序列前增加“周期起点 = 0”的**派生绘图基线**；缺失日零点必须由后端返回，Mobile 不补造业务数据。

### 2. Files found

| Path | Description |
| --- | --- |
| `.trellis/tasks/08-09-mobile-home-return-history-chart/prd.md` | 已确定 UTC 日聚合、1/7/30/180 日、累计已实现收益、partial 不绘制及会话隔离目标。 |
| `mobile/src/views/HomeView.vue` | 当前首页数据、会话采样、SVG、隐私按钮和周期静态标签。 |
| `mobile/pencil/hippo-mobile-uiux.pen` | 当前 Home Member 浅/深画板 `miHnt` / `CvipW` 的原始几何。 |
| `mobile/pencil/exports/{miHnt,CvipW}.png` | 当前 Home Member 视觉导出，确认 390px、153px 曲线区和四周期栏。 |
| `mobile/src/styles/prototype-parity.css` | 生产 Home Member 302px 面板、82/153/43 三行及最终周期样式。 |
| `mobile/src/styles/prototype-base.css` | 旧版 `.portfolio-periods button` 与主题规则；转换为按钮后会重新参与级联。 |
| `mobile/src/api/wallet.ts` | 钱包 API；今日收益在此严格适配后返回。 |
| `mobile/src/core/todayReturn.ts` | 已实现 realized/USDT、十进制、UTC、complete/partial、缺价资产严格校验。 |
| `mobile/src/core/sessionRequest.ts` | 通用 token 会话键、请求版本、invalidate/stop 生命周期。 |
| `mobile/src/core/todayReturnPresentation.ts` | complete 才展示数值，partial/error/loading/hidden 的现成展示边界。 |
| `mobile/src/core/format.ts` | `asNumber` 会把非法值回落为 0，不适合金融历史适配器。 |
| `mobile/src/views/AssetsView.vue` | 同一页面中两个独立 `sessionRequest` 实例的可复用编排范例。 |
| `mobile/tests/today-return.test.ts` | 严格适配、负零、partial、latest-request-wins、隐私和 Today 独立性测试。 |
| `mobile/tests/pencil-selected-home-layout.test.ts` | 当前 Pencil/Home 几何合同，但仍明确要求 `portfolioSamples`，实现时必须更新。 |
| `mobile/tests/{root-prototype-parity,editorial-shell-home-markets,android-ui-foundation-slice-a,core-discovery-views,theme}.test.ts` | 当前 SVG、静态 1 日 active、CSS 与主题源码合同。 |
| `.trellis/spec/mobile/{index,backend-integration,pwa-and-shell}.md` | 44px 控件、真实数据、状态诚实、适配器和会话竞态合同。 |

### 3. Current code patterns

#### 当前曲线不是真实历史

- `portfolioSamples` 是组件内 `number[]`（`mobile/src/views/HomeView.vue:60`）。
- watcher 监听登录态、估值就绪、缺价完整性和当前总资产；每次变化追加一个值，只留最近 32 个（`mobile/src/views/HomeView.vue:278-295`）。因此横轴是本次页面会话中的响应变化，不是天。
- 当前几何要求至少两个样本，按数组索引映射 x，并只按样本 min/max 缩放 y（`mobile/src/views/HomeView.vue:157-173`）。全零或单点都不会产生合适的真实历史图。

#### 周期控件当前不可交互

- 周期数组固定为 1/7/30/180（`mobile/src/views/HomeView.vue:76-79`），但模板使用 `span role=listitem`，且 active 永远是 `period.days === 1`（`mobile/src/views/HomeView.vue:402-410`）。
- 当前最终 CSS 只给 `span` 着色（`mobile/src/styles/prototype-parity.css:2630-2650`）；旧基线仍有 `.portfolio-periods button` 规则（`mobile/src/styles/prototype-base.css:2313-2332,3405-3414,4648-4655`）。改成按钮时必须在 parity 文件末端增加更高优先级的 Home 专属规则，不能依赖旧按钮规则。

#### 今日收益边界已经正确，应复用而非重写

- `mapTodayReturn` 固定 `scope=realized`、`reporting_asset=USDT`，拒绝未知状态、非法十进制、非 UTC 日起点和 complete+missing 的矛盾组合（`mobile/src/core/todayReturn.ts:36-83`）。
- 数字解析拒绝空串、指数、十六进制、布尔/空值、无限值，并归一化 `-0`（`mobile/src/core/todayReturn.ts:99-119`）。历史适配器不能使用会把非法值降为 0 的 `asNumber`（`mobile/src/core/format.ts:3-6`）。
- `createTodayReturnRequestLifecycle` 已委托给通用 `createSessionRequestLifecycle`（`mobile/src/core/todayReturn.ts:89-97`）。通用实现用精确 token、递增版本、`invalidate()` 与 `stop()` 排除旧响应（`mobile/src/core/sessionRequest.ts:13-47`）。
- Home 仅 complete 显示金额；隐私关闭优先掩码，partial/loading/error 不展示部分金额（`mobile/src/views/HomeView.vue:124-155,234-257`）。

#### 当前隐私遗漏

- `assetVisible` 只掩码总资产和今日收益文字（`mobile/src/views/HomeView.vue:124-155,349-363`），`portfolioGeometry` 和 `<path>/<circle>` 没有隐私条件（`mobile/src/views/HomeView.vue:157-173,376-400`）。接入收益历史后，曲线形状本身就是私密收益信息，hidden 时必须不渲染 path、端点和可访问数据表/描述。

### 4. Pencil and SVG layout contract

- 当前 Member 画板为浅色 `miHnt` 和深色 `CvipW`（`mobile/pencil/artboards.json:3-6`）。两者均为 390px 宽。
- Pencil 的 Portfolio 左右 padding 为 16px，内部信号宽 358px；信号区高 153px，水平线位于 y=30/60/90/120，路径宽 2，端点中心位于 x=358 附近（`mobile/pencil/hippo-mobile-uiux.pen:28482-28500,28625-28733`）。
- 周期视觉项为四等分、11px Geist Mono；Pencil 可见框高 32px、上 padding 4px（`mobile/pencil/hippo-mobile-uiux.pen:28736-28840`）。深色画板结构镜像一致（`mobile/pencil/hippo-mobile-uiux.pen:30673-31032`）。
- 生产层把 Member 面板固定为 `grid-template-rows: 82px 153px 43px`，总最小高度 302px（`mobile/src/styles/prototype-parity.css:2519-2530`）；图表固定 153px（`mobile/src/styles/prototype-parity.css:2604-2628`）。
- 当前 SVG 是 `viewBox="0 0 358 153" preserveAspectRatio="none"`，并使用 `vector-effect="non-scaling-stroke"`，可继续在 320–448px 宽度缩放而保持 2px 线宽（`mobile/src/views/HomeView.vue:381-399`）。
- Pencil 是平滑示意曲线并带 glow；生产 SVG 目前只有真实路径和端点。真实金融序列推荐继续使用 `M/L` 折线并加 `stroke-linecap/linejoin=round`，不要使用可能越过真实极值的普通 cubic spline。

### 5. Recommended Mobile DTO

建议后端路径为 `GET /wallet/return-history?period_days=1|7|30|180`；最终路径以同任务后端合同为准，但 Mobile 应把路径差异封装在 `wallet.ts`。

```ts
export const RETURN_HISTORY_PERIODS = [1, 7, 30, 180] as const
export type ReturnHistoryPeriodDays = typeof RETURN_HISTORY_PERIODS[number]
export type ReturnHistoryStatus = 'complete' | 'partial'

export interface BackendReturnHistoryPoint {
  day_start_at: unknown
  amount: unknown
  basis_amount: unknown
  cumulative_amount: unknown
  cumulative_basis_amount: unknown
  status: unknown
  missing_price_assets: unknown
}

export interface BackendReturnHistory {
  scope: unknown
  reporting_asset: unknown
  period_days: unknown
  period_start_at: unknown
  calculated_at: unknown
  amount: unknown
  basis_amount: unknown
  rate: unknown
  status: unknown
  missing_price_assets: unknown
  points: unknown
}

export interface ReturnHistoryPoint {
  dayStartAt: number
  amount: number
  basisAmount: number
  cumulativeAmount: number
  cumulativeBasisAmount: number
  status: ReturnHistoryStatus
  missingPriceAssets: string[]
}

export interface ReturnHistory {
  scope: 'realized'
  reportingAsset: 'USDT'
  periodDays: ReturnHistoryPeriodDays
  periodStartAt: number
  calculatedAt: number
  amount: number
  basisAmount: number
  rate: number
  status: ReturnHistoryStatus
  missingPriceAssets: string[]
  points: ReturnHistoryPoint[]
}
```

字段责任：

- `amount/basis_amount/rate` 是整个周期汇总，语义与 Today DTO 一致；首页右上仍不读取这些字段，避免双源。
- point 的 `amount/basis_amount` 是该 UTC 日净收益/成本；`cumulative_*` 是从周期起点累计到该日。
- 缺失日由后端返回真实零点：`amount=0`、`basis_amount=0`、累计值延续。Mobile 不自行补日期。
- partial 可以保留后端部分合计用于诊断，但展示层不得绘制或格式化成完整收益。

### 6. Strict adapter contract

建议放在 `mobile/src/core/returnHistory.ts`，由 `mobile/src/api/wallet.ts` 只负责 HTTP 与调用 mapper。为避免严格规则分叉，可把 `todayReturn.ts` 的十进制、时间戳和缺价资产解析提取为 realized-return 私有共享 helper；提取必须由现有 `today-return.test.ts` 保证行为不变。

`mapReturnHistory(payload, expectedPeriod)` 必须：

1. 只接受白名单 period，且 `payload.period_days === expectedPeriod`。
2. 固定 `realized` / `USDT`；状态只接受 complete/partial。
3. 复用 Today 的严格十进制语法和 `-0 -> 0`；basis/cumulative basis 不得为负。
4. 时间戳归一到安全整数毫秒；`period_start_at` 和每个 `day_start_at` 必须 UTC 00:00。
5. `points.length === periodDays`，严格升序且相邻正好 86,400,000ms；首点等于 `periodStartAt`，末点等于 `calculatedAt` 所在 UTC 日。
6. complete point/top-level 不得含 missing assets；top-level missing 集合必须覆盖各 point 的并集；任一点 partial 时 top-level 必须 partial。
7. 最后一点 `cumulativeAmount/cumulativeBasisAmount` 必须与顶层 `amount/basisAmount` 一致。
8. 空活动不是空数组：必须是完整的 N 个零日点；空数组、断日、重复日、乱序、NaN/Infinity、指数/十六进制数字均拒绝。

API 只暴露：

```ts
fetchReturnHistory(periodDays: ReturnHistoryPeriodDays): Promise<ReturnHistory>
```

### 7. Request lifecycle and Home orchestration

推荐为 Today、History、资产估值保留独立读取边界。历史直接复用通用生命周期：

```ts
const selectedPeriod = ref<ReturnHistoryPeriodDays>(1)
const returnHistory = ref<ReturnHistory | null>(null)
const returnHistoryState = ref<'idle' | 'loading' | 'complete' | 'partial' | 'error'>('idle')

const returnHistoryLifecycle = createSessionRequestLifecycle({
  sessionKey: () => session.token,
  request: () => fetchReturnHistory(selectedPeriod.value),
})
```

生命周期顺序：

- token watcher 使用 `{ immediate: true }`：先 `invalidate()`，同步清空 history/state，并在新会话默认重置到 1 日，再调用 load。
- 周期点击：先更新 `selectedPeriod`，立即清空旧曲线并进入 loading，再调用 load；不能在 7 日 active 时继续显示旧 1 日路径。
- load 返回 `stale` 时不改任何新状态；`guest` 清空为 idle；`loaded` 只提交 mapper 后 DTO；`error` 清空 DTO并进入 error。
- `onUnmounted` 调用 `stop()`；无需复制 SecondsHistory 的自定义 generation 实现。
- 周期 A→B→A、同周期重试、token A→B、logout 和 unmount 都由版本号隔离。底层请求可继续完成，但不能提交。
- Today 请求失败不影响 History，History 失败也不覆盖 Today。不要 `Promise.all` 绑定两块状态。

`HomeView` 当前资产读取只 watch `session.isAuthenticated`（`mobile/src/views/HomeView.vue:273`），不能作为历史实现范例，因为换号时 boolean 可能不变；历史必须 watch 精确 token。

### 8. State, privacy and presentation matrix

| State | Visual | Accessible state | Data retention |
| --- | --- | --- | --- |
| guest | 现有 Guest Hero；不显示 member chart | 不宣布私有收益 | 不请求，清空 DTO |
| hidden | 保留 153px 网格/遮罩，不渲染 path、dot、数值描述或隐藏表 | “收益已隐藏” | 可后台加载，但 DOM 不暴露收益描述 |
| loading | 保留 Pencil 153px 骨架，移除旧路径 | `aria-busy=true`、polite 状态 | DTO 先清空 |
| complete/non-zero | 绘制累计路径和端点 | 周期、累计值、起止日期文本替代 | 当前 token/period DTO |
| complete/all-zero | 绘制居中的零水平线；这是“真实零收益” | 明确宣布零收益 | 完整 N 日零点 |
| partial | 不绘制部分曲线；可显示本地化不完整状态 | visible 时可宣布缺价并提供重试；hidden 时只宣布隐藏 | 可保留 DTO 诊断，不用于 geometry |
| error | 不保留旧曲线，显示本地化错误与 44px 重试 | `role=alert` | DTO 清空 |

隐私优先级必须高于 loading/partial/error 文案，尤其不能在 hidden 时通过缺价资产代码、路径形状、端点位置、`aria-label` 或 screen-reader 数据表泄露账户活动。

### 9. Geometry recommendation

建议纯函数：

```ts
buildReturnHistoryGeometry(history: ReturnHistory): ReturnHistoryGeometry | null
```

规则：

1. 仅接受 `status === 'complete'`。
2. 绘图值始终为 `[0, ...points.map(p => p.cumulativeAmount)]`。首个 0 是周期起点派生基线，不写回 DTO，也不冒充某个业务日。
3. 1 日因此自然得到两个点：`(0, zeroY)` 与 `(358, currentY)`；满足单日单点可成线。
4. N 日点的 x 为 `index / N * 358`，最后一点固定 x=358；180 日共 181 个绘图点，成本很低。
5. y domain 必须包含 0：`min(0, ...values)` / `max(0, ...values)`。非平坦序列使用上下各 12px；全零时所有点固定 `y=76.5`，不得出现 NaN/Infinity，也不能贴顶。
6. 路径使用 `M` + `L`，点均限制在 `x=0..358`、`y=12..141`。末点沿用 Pencil 的右缘半裁切视觉。
7. 末值正/负/零分别使用 mint/coral/neutral；颜色不是唯一信息，文本替代必须包含符号和金额。
8. 保留当前 `viewBox`、`preserveAspectRatio="none"` 和 `non-scaling-stroke`，不引入外部图表依赖。

### 10. Accessible interaction

- 周期项改为原生 `<button type="button">`；外层使用 `role="group"` 和本地化 label，每个按钮使用 `aria-pressed="selectedPeriod === period.days"`、`aria-controls` 指向图表。原生按钮自动支持 Enter/Space。
- 不使用 `tablist/tab`，除非同时实现 roving tabindex、左右/Home/End 键和 tabpanel 合同。项目现有规范也优先建议按钮组 + `aria-pressed`（`.trellis/spec/mobile/index.md:157-160`）。
- 当前全局 button 最小高度 44px（`mobile/src/styles/base.css:127-138`）。Pencil 可见 period 仅 32px、生产行 43px；实现应让透明按钮真实 bounding box 至少 44px，并用轻微负 margin/overflow 保持外层 302px 和 153px chart 不变，同时验证 320/390/448px。
- 增加 Home 专属 `:focus-visible` 完整 focus ring；不能只靠 mint 文本颜色表示 active。
- SVG 可继续 `aria-hidden=true`，但外层使用 `<figure>` + 本地化 `<figcaption>`。complete 且 visible 时提供 `.sr-only` 表格（日期、当日收益、累计收益）作为长文本替代；hidden/partial/error 时不渲染该数据表。
- loading/partial/error 的状态文本放在独立 polite live region；错误重试为可聚焦 44px 按钮。SVG 本身不需要变成 181 个可聚焦点。

### 11. Existing tests and recommended changes

#### Existing coverage to retain

- `mobile/tests/today-return.test.ts:19-99`：realized、UTC、负零、partial 与非法输入。
- `mobile/tests/today-return.test.ts:101-146`：访客、并发、换号、退出、卸载迟到响应。
- `mobile/tests/today-return.test.ts:148-163,165-224`：Home/Assets 仅 complete 显示、隐私和正负零。
- `mobile/tests/pencil-selected-home-layout.test.ts:29-71`：Guest/Member 分支及 302/153 几何；其中 `:45-47` 的 `portfolioSamples` 断言必须替换。
- `mobile/tests/root-prototype-parity.test.ts:184-193`：SVG 与固定 1 日 active 源码合同；需改为动态 selected period。
- `mobile/tests/editorial-shell-home-markets.test.ts:82-109`、`android-ui-foundation-slice-a.test.ts:37-78`、`core-discovery-views.test.ts:52-63`：Home SVG/周期/真实数据存在性。
- `mobile/tests/theme.test.ts:103-128`：当前仍检查旧 `.portfolio-periods button.active` 主题规则，转换后应对准最终 Home selector。

#### Add `mobile/tests/return-history.test.ts`

1. **DTO/adapter**：四个允许周期；raw→camel；秒/毫秒归一；负零；非法 scope/asset/status/数字；period echo mismatch；点数、首尾、乱序、重复、断日；complete+missing 矛盾；partial 并集；最终累计与顶层不一致。
2. **Geometry**：1 日单点生成两点且基线为 0；7/30/180 累计点数；全零 y=76.5；正/负/跨零均在 12..141；末点 x=358；路径不含 NaN/Infinity 且只含 M/L。
3. **Lifecycle with deferred promises**：guest 不请求；1→7 旧 1 日迟到；1→7→1 ABA；同周期重试；token A→B；logout；unmount；adapter error 后重试成功。
4. **Presentation/privacy**：hidden 不生成 path/dot/figcaption 数值/隐藏表；partial 不生成 geometry；loading/error 不保留前序 path；真实零与 unavailable 分离；today 和 history 一边失败不影响另一边。
5. **Source contract**：不存在 `portfolioSamples`/资产 watcher；请求参数来自白名单；buttons + `aria-pressed` + `aria-controls`；`data-portfolio-source` 改为真实 realized history；无 demo/random/fallback 点。
6. **Pencil/runtime**：浅/深主题在 320×720、390×844/892、448×900 检查 Member 总高 302、chart 153、四按钮等宽、按钮 bounding box ≥44、无横向溢出；键盘切周期、焦点 ring、error retry、隐私切换均可达。
7. **i18n**：中英文补齐 history loading/partial/error/retry/hidden/chart summary/table caption 等键并保持对称。

推荐质量门禁：

```bash
npm --prefix mobile run type-check
npm --prefix mobile test
npm --prefix mobile run build:pwa
npm --prefix mobile run build:tauri
```

### 12. External references

- [WAI-ARIA APG Button Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/button/)：原生按钮支持 Enter/Space；toggle button 用 `aria-pressed` 暴露状态。
- [WCAG 2.2 Understanding 1.1.1 Non-text Content](https://www.w3.org/WAI/WCAG22/Understanding/non-text-content.html) 与 [WAI Complex Images](https://www.w3.org/WAI/tutorials/images/complex/)：图表需要短文本替代；复杂数据需要描述或等价长文本/表格。
- [WCAG 2.2 Target Size Minimum](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum) / [Target Size Enhanced](https://www.w3.org/WAI/WCAG22/Understanding/target-size-enhanced)：AA 最低 24px；项目自身采用更强的 44×44px 控件合同。
- [MDN `preserveAspectRatio`](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Attribute/preserveAspectRatio) 与 [`vector-effect`](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Attribute/vector-effect)：当前 viewBox 非等比缩放及 `non-scaling-stroke` 行为符合现有响应式 SVG 结构。
- [Vue Watchers](https://vuejs.org/guide/essentials/watchers.html)：异步副作用需要排除 stale 结果。当前安装 Vue 3.5.39，但本项目已有更强的 token+version `sessionRequest`，应优先复用该项目合同。

### 13. Related specs

- `.trellis/spec/mobile/backend-integration.md:144-162`：受保护请求、刷新和会话清理。
- `.trellis/spec/mobile/backend-integration.md:198-205`：generation、ABA 和迟到请求隔离的既有市场会话合同。
- `.trellis/spec/mobile/backend-integration.md:271-273,358-433`：history latest-request-wins、互斥状态、错误矩阵和测试方式。
- `.trellis/spec/mobile/index.md:15-24,33-47`：Mobile 质量门禁与 44px 触控合同。
- `.trellis/spec/mobile/index.md:171-173,196-214`：图表/信号语义色、生产数据真实和加载失败不得填 demo 值。
- `.trellis/spec/mobile/pwa-and-shell.md`：选中 Pencil 页面几何、真实 API 状态、320–448px 与无横向溢出边界。
- `.trellis/spec/guides/cross-layer-thinking-guide.md:20-84`：后端 DTO→适配器→组件的格式和错误边界。
- `.trellis/spec/guides/code-reuse-thinking-guide.md:18-60`：复用 Today 严格解析和通用生命周期，避免重复实现漂移。

## Caveats / Not Found

- 当前仓库未发现 `return-history` 后端路由、Mobile DTO/adapter 或对应测试；上述字段名和路径是推荐合同，需与后端研究/实现最终对齐。
- 历史非稳定币应使用事件时、日终历史价格还是当前价仍是后端口径问题。Mobile 不能用当前 ticker 修补历史；后端标记 partial 时客户端必须不绘制。
- 当前 Pencil Member 画板只给出正收益完整态，没有负收益、零收益、loading、partial、error 或 hidden 画板；这些状态必须在不改变 302/153/43 外部几何的生产层补齐。
- 当前 Mobile Node 测试大量是源码正则合同，不足以证明焦点、ARIA、真实 bounding box 和 SVG 缩放；需要浏览器运行时验证。
- `task.py current --source` 返回无 active task；本次依据用户明确指定的 `.trellis/tasks/08-09-mobile-home-return-history-chart/` 路径写入，没有修改业务代码或其他文件。
