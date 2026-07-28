# 手机端 UI 像素级 1:1 复刻

## Goal

以 `mobile/sites-prototype/` 已发布 v16 原型为唯一视觉基线，重构
`mobile/` Vue 3 + Tauri 客户端，使 Web、PWA 与 Android 在相同手机视口
下呈现一致的页面结构、几何尺寸、排版、色值、图标和交互状态，同时保留
现有真实后端 API、认证、PWA 和原生构建能力。

## What I Already Know

- 已发布原型地址为
  `https://hippo-mobile-signal-2026.ikuboy.chatgpt.site/?version=16`。
- 原型完整源码位于 `mobile/sites-prototype/app/`，不是只能通过截图反推。
- 原型根页面由 `page.tsx` 定义，二级页面由 `secondary-pages.tsx` 定义，
  全部视觉令牌和几何规则位于 `globals.css`。
- 当前 `mobile/` 已经接入后端接口、PWA、Tauri 和 Android，但多个页面只
  对齐了视觉语言，没有复刻原型的真实 DOM 几何和状态层级。
- 当前最明显的偏差包括行情页结构、秒合约页面层级、资产/个人中心状态、
  资产图表高度、快捷入口间距、AI 卡片位置和异形底栏。
- 图标必须统一使用 Lucide，界面禁止表情符号。

## Requirements

- 原型源码是视觉和交互结构的 source of truth；不新增原型中不存在的根页
  hero、卡片或登录占位结构。
- 根页面完整对齐：首页、行情、现货、合约、资产、我的。
- 秒合约保持独立栏目，并按原型二级交易工作台实现，不与现货或合约合并。
- 异形七栏底部导航必须复刻原型尺寸、抬升中心按钮、层级和安全区。
- 二级页面使用原型 PageShell、字段、按钮、分段控件、弹窗和列表合同。
- 优先完整对齐用户已点名的消息中心、贷款、秒合约、安全中心；其余现有
  二级路由统一迁移到相同视觉系统。
- 动态金额、行情、账户和订单必须继续来自现有 API；接口失败时保留目标
  几何，使用同尺寸 skeleton/empty/error 状态，不用错误横幅推挤主结构。
- 未登录状态必须使用原型定义的访客/认证流程，不得用自创的大块
  `LoginRequiredState` 替换整页布局。
- 中文为像素验收基线，英文仍需支持且不得产生横向溢出。
- PWA manifest、service worker、离线壳层及 Tauri/Android 构建继续可用。

## Acceptance Criteria

- [x] 在 `390x844`、浅色、中文条件下，七个根入口与原型的区块顺序一致。
- [x] 同视口下，header、首屏主要区块和底栏的边界误差不超过 2 CSS px。
- [x] 主要字号、行高、按钮高度、字段高度、边框、圆角和色值来自原型 CSS。
- [x] 首页资产区、快捷入口、AI 卡片和首个行情区块在首屏的露出量与原型一致。
- [x] 行情页使用 `MARKET PULSE` hero、搜索、五分类、市场温度和三列行情表。
- [x] 现货与合约分别呈现原型交易台，并继续提交到现有交易 API。
- [x] 秒合约无根底栏，包含二级 header、市场板、方向、周期、金额、收益摘要、
  确认弹窗和记录区。
- [x] 资产页和个人中心的已登录/未登录状态不改变原型的核心几何合同。
- [x] 消息中心、贷款、安全中心与原型对应页面的首屏结构和控件状态一致。
- [x] 所有可操作图标来自 Lucide，界面没有 emoji。
- [x] `320x844` 和 `448x956` 无页面级横向滚动、文字遮挡或底栏溢出。
- [x] `npm run type-check`、`npm test`、`npm run build:pwa`、
  `npm run build:tauri` 全部通过。
- [x] Android debug APK 构建成功，并在可用设备上安装后完成启动与前台状态验收。

## Definition of Done

- 根页面和重点二级页通过同尺寸原型/本地截图对比。
- API 数据加载、认证、交易提交和路由回退没有行为回归。
- PWA 与 Tauri 构建通过。
- Android APK 可安装运行。
- 进度、规范、任务记录与提交完成。

## Out of Scope

- 修改后端接口协议或后端业务规则。
- 重做已发布 Sites 原型。
- 引入新的设计系统、非 Lucide 图标库或 Web3 钱包模块。
- 为了展示而伪造会被误认为真实账户或真实订单的数据。

## Technical Notes

- 视觉基线：
  - `mobile/sites-prototype/app/page.tsx`
  - `mobile/sites-prototype/app/secondary-pages.tsx`
  - `mobile/sites-prototype/app/globals.css`
- Vue 数据与行为基线：
  - `mobile/src/api/`
  - `mobile/src/stores/`
  - `mobile/src/router/index.ts`
- 详细差异和迁移策略见
  `research/prototype-parity-contract.md`。
