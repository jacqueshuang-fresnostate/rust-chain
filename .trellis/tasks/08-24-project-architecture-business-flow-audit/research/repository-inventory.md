# 仓库规模、工程治理与测试基线

## 审计口径

- 统计对象以 `git ls-files` 为准，避免把本机 `node_modules`、`target`、构建产物和 IDE 状态当成生产源码。
- 代码行数统计覆盖 `.rs`、`.ts`、`.tsx`、`.vue`、`.js`、`.jsx`、`.css`、`.sql`、`.yml`、`.yaml`、`.toml`、`.sh`。
- 测试形态通过测试文件中的源码读取断言（`readFileSync` 等）和 DOM/组件测试标记（Testing Library、`mount`、`render`）做静态分类；它反映测试层级，不代表单条测试质量。
- 基线日期：2026-08-24。

## 1. 仓库地图与规模

仓库共有 2,323 个 tracked files，主要生产/测试区域如下：

| 区域 | 代码文件数 | 代码行数 | 当前职责 |
| --- | ---: | ---: | --- |
| `src/` | 331 | 109,602 | Rust API、限界上下文、基础设施和后台工作器 |
| `mobile/` | 263 | 80,636 | Vue 3 手机 Web/PWA/Tauri 客户端 |
| `tests/` | 113 | 75,088 | Rust 单元、路由与工作器测试 |
| `web/` | 178 | 42,091 | React + Semi Design 管理后台 |
| `pc/` | 122 | 27,936 | Vue 3 + Tauri PC 客户端 |
| `migrations/` | 103 | 6,062 | SQLx/MySQL 增量迁移 |

根目录没有统一的 JS workspace 或任务编排器；Rust、后台、PC、手机分别维护构建命令和依赖锁定。这样可以独立构建，但跨端契约验证、统一 CI 入口和依赖升级需要手工同步。

## 2. 结构热点

### 前端热点

| 文件 | 行数 | 风险 |
| --- | ---: | --- |
| `mobile/src/styles/prototype-base.css` | 8,034 | 全局样式覆盖面过大，选择器优先级和回归影响难以隔离 |
| `mobile/src/views/TradeView.vue` | 5,935 | 行情会话、盘口、图表、下单、持仓和弹层集中在单一视图 |
| `mobile/src/styles/prototype-parity.css` | 3,707 | 设计稿差异补丁长期叠加，难以判断最终样式来源 |
| `mobile/src/views/SecondsView.vue` | 2,818 | 实时行情、并行订单、结算展示与交互状态耦合 |
| `mobile/src/views/AssetsView.vue` | 2,046 | 多账户、收益、划转和资产列表耦合 |
| `web/src/styles.css` | 2,721 | 管理后台全局样式边界较宽 |
| `pc/src/i18n/index.ts` | 2,212 | 多语言资源集中在单文件，评审和按功能拆分困难 |
| `pc/src/api/backendAdapters.ts` | 2,038 | 多业务 DTO 转换集中，契约漂移的影响面大 |
| `web/src/admin/resources/resourceConfigs.tsx` | 1,469 | 资源定义、表格、表单和动作配置聚合 |
| `web/src/admin/resources/actions/wallet.tsx` | 1,416 | 钱包后台动作集中，测试与权限边界变宽 |

这些文件不适合一次性重写。建议以业务能力为切片，先抽取纯数据适配器、会话 composable/hook、弹层和表单状态机，再将样式迁到与组件同域的层级；保留现有 façade 作为迁移期兼容入口。

### 后端热点与已有防线

生产 Rust 的主要单文件热点包括：

- `src/modules/prediction/infrastructure.rs`：1,830 行；
- `src/modules/new_coin/infrastructure.rs`：1,590 行；
- `src/modules/auth/infrastructure.rs`：1,506 行；
- `src/modules/market/infrastructure/adapters/provider.rs`：1,363 行；
- `src/modules/market/infrastructure/adapters/feed.rs`：1,264 行；
- `src/workers/market_feed.rs`：1,194 行；
- `src/modules/wallet/application.rs`：1,184 行；
- `src/workers/margin_liquidation.rs`：1,175 行。

项目已经有可执行的架构防线，不能将现状误判为“没有分层约束”：`tests/backend_architecture.rs:12-199` 校验可选 DDD 层、依赖方向、测试位置、生产文件 2,000 行上限，并对部分核心文件施加 1,200 行上限。当前不足是 1,200 行清单只覆盖 events、margin、admin routes 和 spot 等少数历史热点（`tests/backend_architecture.rs:162-199`），没有包含后来增长的 prediction/new_coin/auth/market provider 等文件。建议先补充守卫清单，再按真实职责拆分，避免迁移过程中继续增长。

## 3. 测试层级与工程基线

| 客户端 | 测试文件 | 源码文本断言 | DOM/组件测试标记 | 工程脚本 |
| --- | ---: | ---: | ---: | --- |
| 手机端 | 78 | 66 | 3 | 有 `type-check` 和 Node test |
| PC 端 | 20 | 15 | 0 | 只有 `type-check`，没有 `test` 脚本 |
| 管理后台 | 52 | 1 | 37 | 有 ESLint、TypeScript、Vitest/jsdom |

证据：

- 手机端使用 `node --test --experimental-strip-types tests/*.test.ts`，且开发依赖中没有 Vue Test Utils/jsdom（`mobile/package.json:6-43`）。
- PC 端没有测试脚本，也没有组件测试依赖（`pc/package.json:6-55`）。
- 管理后台已使用 Testing Library、jsdom 和 Vitest（`web/package.json:6-40`）。

源码合同测试对设计稿类名、路由常量和 API 文本很有价值，但无法证明 Vue 组件真实挂载、响应式更新、焦点管理、WebSocket 重连和并发旧响应隔离在浏览器运行时成立。建议保留这类轻量合同，同时为资金与交易关键路径增量增加：

1. Vue Test Utils + Vitest/jsdom 组件测试；
2. API/MSW 或等价的传输层 mock；
3. 5～10 条 Playwright 跨端烟雾用例（登录、现货下单、杠杆开平仓、秒合约结算、充提/闪兑）；
4. PC 增加正式 `test` 脚本，并纳入 CI。

## 4. CI 与发布治理

`.github/workflows/docker-image.yml:15-168` 当前只构建并发布多架构镜像。构建前没有独立执行：

- `cargo fmt --check`、Clippy、Rust 测试；
- 管理后台 lint/typecheck/test；
- PC 类型检查/测试；
- 手机端类型检查/测试/PWA 构建；
- Compose 配置验证、迁移兼容检查；
- SBOM、依赖/镜像漏洞扫描。

Docker 构建会间接运行管理后台 build 和 Rust release build，但这既不能替代测试，也会让错误在昂贵的多架构编译阶段才暴露。发布 job 与质量 job 没有 `needs` 关系，因此当前 main push 可能在测试缺失的情况下直接推送镜像。建议新增快速质量门并让 publish 显式依赖它，再把安全扫描和多架构构建放到后续阶段。

此外，workflow 使用 action 的浮动主版本标签（如 `actions/checkout@v6`、`docker/build-push-action@v7`，见 `.github/workflows/docker-image.yml:33-43`、`:69-87`），建议固定到 commit SHA，并通过 Dependabot/Renovate 更新。

## 5. 交付状态治理

`.trellis/tasks/` 当前共有 124 个任务：

- `in_progress` 66；
- `planning` 3；
- `review` 1；
- `done` 31；
- `completed` 23。

其中 113 个任务已创建至少 7 天，81 个至少 30 天，78 个至少 60 天；最老的活动任务已 73 天。大量历史任务长期停在 `in_progress`，会让“当前正在做什么”、任务依赖和完成率失真。

建议建立以下治理约束：

1. 每个开发者同时最多 1～2 个 `in_progress`；
2. 超过 7 天无进度自动标记待复核；
3. `done`、`completed` 统一语义并按月归档；
4. PR/提交必须关联一个活动任务；
5. 每周自动输出陈旧任务报告，不自动删除历史记录。

## 6. 本地工具索引

本地 `.codegraph` 约 445 MB，索引包含 `node_modules`、Rust `target` 和生成目录；此前查询返回过 `web/node_modules` 的符号，而不是生产代码。与此同时本机 `target` 约 15 GB，三个前端 `node_modules` 合计约 849 MB。Git 忽略规则是正确的（`.gitignore:9-27`），问题在本地代码图索引的排除策略。

建议为 Codegraph 增加 `target`、`node_modules`、`dist`、Tauri generated/target、设计导出目录的显式排除，并在重新索引前清理旧数据库。该项属于 P2 工具效率优化，不影响运行时。

## 7. 已执行基线验证

- `cargo fmt --all -- --check`：通过；
- `cargo clippy --all-targets -- -D warnings`：通过；
- `cargo test --lib`：沙箱内 5 条 wiremock 用例因禁止绑定本地端口失败；按相同命令在允许本地临时端口的环境重跑后 280/280 通过；
- `cargo test --test backend_architecture`：11/11 通过；
- `npm --prefix web run lint && npm --prefix web run typecheck && npm --prefix web test`：52 个文件、381 条测试通过；
- `npm --prefix pc run type-check`：通过；
- `npm --prefix mobile run type-check && npm --prefix mobile test`：482 条测试通过。

这些结果说明项目当前可编译/可验证的基础良好，也同时证明 CI 未复用现有质量能力是流程缺口，而不是项目缺少相应命令。
