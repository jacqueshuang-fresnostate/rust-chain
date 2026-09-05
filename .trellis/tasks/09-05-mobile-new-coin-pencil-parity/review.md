# 手机新币页面 Pencil 终验复核

## Findings（已修复）

- 文件：`mobile/src/views/NewCoinsView.vue`、`mobile/src/styles/pencil-selected-pages.css`
  - 问题：打新活动内容缺少选稿规定的标题后 12px 间距，390px 深色运行时第一张卡片曾落在 Web body `y=304`/参考图含原生栏 `y=332`，比选定 `ZTtvY` 提前 40px。
  - 修复：锁定专区 `54/148/50/36px` 前置区、内容顶距 8px、标题 36px、标题后 12px，最终第一张卡片为 `x=16, y=344, 358x300`（参考图坐标含 28px 原生状态栏为 `y=372`）。
- 文件：`mobile/src/core/newCoinPresentation.ts`、`mobile/src/components/new-coin/{NewCoinProjectCard,NewCoinOpportunityCard,NewCoinRecordCard}.vue`、`mobile/src/views/NewCoinDetailView.vue`、`mobile/src/i18n/messages/{zh-CN,en}.ts`
  - 问题：交易先机错误依赖仅控制新币直接购买 mutation 的 `postListingPurchaseEnabled`；三个受支持解禁枚举可能裸露；缺失名称会被符号伪装；HIPPO 的 `100,000,000 HIPPO` 在固定卡片列内被截断。
  - 修复：机会只按权威 `post_listing_pair_id` 精确关联真实 ticker；集中映射 `immediate_on_listing`、`fixed_time`、`relative_period` 并为未知值保留真实原文；所有项目名称位改用双语“项目名称暂缺”回退；项目卡为完整财务字符串增加 title 和长度感知字号，保持 358x300 轨道不变。
- 文件：`mobile/src/api/requestAuth.ts`、`mobile/src/api/{client,market,newCoin}.ts`、`mobile/src/views/{NewCoins,NewCoinDetail,NewCoinRecords}View.vue`、`mobile/tests/request-layer.test.ts`
  - 问题：带陈旧会话进入公开专区/详情时，401 可能走刷新和登录过期路径；同时不能放松钱包、记录和写动作的认证边界。
  - 修复：显式公开请求标记只剥离凭证并关闭刷新/清会话路径，属于失败关闭；仅公开新币列表/详情及公开行情调用使用，订阅、购买、四类记录、钱包、手续费与释放调用均未标记。访客运行时保持公开路由，记录页在任何私有请求前短路。
- 文件：`mobile/src/components/new-coin/NewCoinRecordCard.vue`、`mobile/src/views/NewCoinRecordsView.vue`、`mobile/src/styles/pencil-selected-pages.css`
  - 问题：待结算/已完成卡片曾分别覆盖为橙/灰，与 `A9It6g`/`h4gfd` 导出图统一 forest/sage green 冲突；受 `.view-stack` transform 影响的 fixed 弹层也可能只覆盖路由容器。
  - 修复：三种样本状态统一 Logo 回退、左轨、状态点/字、主要结果、箭头与操作为同一绿系；真实后台 Logo 不被重着色。记录类型和手续费弹层、详情复核层 Teleport 到 body，并保留明暗主题变量、遮罩关闭、Escape、焦点闭环和焦点恢复。
- 文件：`mobile/src/core/newCoinModel.ts`、`mobile/src/api/newCoin.ts`、`src/modules/new_coin/{repository,infrastructure,presentation}.rs`、`tests/new_coin_routes.rs`
  - 问题：公开项目读模型没有输出后台资产名称/Logo和计价资产符号/Logo，Mobile 原映射还会把高精度金融值转为 Number。
  - 修复：列表/详情复用相同 LEFT JOIN 与 DTO 字段；Mobile 严格映射可空文本/Logo、ID、布尔和 Decimal 字符串，空白归一化、错误类型失败关闭，不推导外部 Logo。订单写入、供给会计、结算和解禁算法保持原样。
- 文件：`mobile/src/components/PageHeader.vue`、`mobile/scripts/{check-bundle-budget,check-source-size}.mjs`
  - 问题：需核实共享 Header 与预算调整是否扩大了任务面。
  - 修复：PageHeader 仅增加默认不生效的 `backIcon: 'arrow' | 'chevron'`，旧页面继续 Arrow，新币三页按选稿使用 Chevron；关联 Header 回归通过。源码预算只重基线新增选稿 CSS 的实际 976 行/26224 bytes；Bundle 仅把 CSS raw 上限 640 调至 656 KiB，gzip 仍为 128 KiB，最终实际 650.6/121.4 KiB。

## Findings（未修复；独立后续）

- `tests/new_coin_routes.rs` 的既有 `concurrent_new_coin_subscriptions_never_allocate_beyond_remaining_supply` 在一次性真实 MySQL 全套运行中返回状态 `[200, 500]`，而既有断言要求 `[200, 400]`；同次其余 10 个路由测试通过。该用例与相应订单写/供给锁定代码早于本任务，且本任务 PRD 明确排除订单写入、供给会计和结算语义，因此本轮没有保留任何订阅/购买幂等或锁定改动。应另立后端并发事务任务分析和修复。

## 运行时与合同证据

- 八张 390x920 导出 PNG 均按 390x892 Web body 比对：专区项目卡 358x300，机会卡 358x140；详情连续区为 Header 56、主视觉 210、阶段 112、规则 104、申购面板 328px；记录为 Header 58、筛选 56、列表顶距 10、卡片 358x168、卡间 14px。
- 真实 HIPPO 公开载荷在明暗主题均显示“项目名称暂缺”“上市即释放”和完整 `100,000,000 HIPPO`。320/390/448px 的专区两 Tab、详情和记录轨道均无水平滚动；控制台 warning/error 为空。
- 2026-09-05 再次核对线上公开接口，确认旧镜像响应仍缺少四个资产元数据字段；当前后端联表 DTO 是项目 Logo 与发行价计价符号的唯一公开修复来源。项目卡将发行价金额与计价符号拆成独立布局槽，长金额只压缩金额槽，权威计价符号不会再随末尾省略号一起消失。
- 机会卡和三种记录卡使用实际生产组件及本地只读夹具逐态比对；真实线上无权威 pair+ticker 时保持诚实空态，不生成 Pencil 样本行情。
- 四类记录 API 只在认证后并发读取一次，再按 `createdAt` 倒序合并和本地筛选；手续费支付与释放仍走原受保护接口。项目/计价 Logo和名称仅来自公开后端 DTO；机会行情只持有共享 Market Store 的一个消费者租约。

## Verification

- Mobile 聚焦：5 个文件，30/30 通过。
- Mobile TypeCheck：`npm --prefix mobile run type-check`、`type-check:tests` 通过。
- Mobile Governance：`check:source-size`、`check:test-quality` 通过。
- Mobile release gate：最终 Mobile 源码状态下通过；全量测试 672/672，PWA/Tauri production build、两类产物检查、Bundle 与治理门禁全部通过。
- Rust 格式/Lint/TypeCheck：`cargo fmt --all -- --check`、`cargo check --all-targets --all-features`、`cargo clippy --all-targets --all-features -- -D warnings` 通过。
- Rust 聚焦：`cargo test new_coin` 通过；一次性本机 MySQL 的公开列表/详情权威资产元数据测试通过，临时库在验证后删除。
- Diff：`git diff --check` 通过。
- Trellis：`python3 ./.trellis/scripts/task.py validate .trellis/tasks/09-05-mobile-new-coin-pencil-parity` 通过。
- 未提交、未推送；未编辑 `.pen`，也未还原或覆盖既有 `mobile/pencil` 脏文件。
