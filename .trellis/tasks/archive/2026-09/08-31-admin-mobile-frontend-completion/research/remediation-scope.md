# Admin 与 Mobile 全量整改范围（2026-08-31）

## 用户边界

- 实施范围：`web/**`（Admin）、`mobile/**`，以及为二者闭环所需的最小 `src/**`、`tests/**`、CI/脚本/文档调整。
- 排除范围：`pc/**`。任何 PC 审计问题均不得在本任务中顺手修改。
- 保留边界：工作树中已有未提交成果只能增量复用，不得 restore/reset/覆盖。

## 风险顺序

1. Admin 资金幂等、Decimal、会话和精确权限。
2. Mobile session epoch、行情 cold-start、请求 generation、Decimal 和批量撤单。
3. 两端 DTO/错误合同、实时 freshness 和私有推送。
4. PWA/Tauri、无障碍、i18n、资源预算、结构拆分。

## 实施切片与写入所有权

### A. Admin correctness/security

- FAD-P0-01、FAD-P1-01/02/03/04/05/06/07。
- 主要写入：`web/src/**`、`web/tests/**`；若必须，增加兼容后端 auth/idempotency 查询能力。
- 先补行为测试，再改 idempotency/session/access/query/decimal/socket primitives，最后迁移页面。

### B. Mobile core correctness

- FMD-P1-01/02/03/04、FMD-P2-01，加上错误 code 本地化与私有缓存失效。
- 主要写入：`mobile/src/api/**`、`mobile/src/stores/**`、`mobile/src/core/**`、订单页及定向测试。
- 必须用 deferred Promise/fake transport 验证旧 generation 不写回。

### C. Mobile platform/UX

- FMD-P1-05/06/07、FMD-P2-02/03/04/05，以及触控目标、PWA 提示、bundle 资源。
- 主要写入：Mobile shell、router、PWA/Tauri、私有 WS、KYC/Seconds 交互、构建脚本和行为测试。
- 保持 Pencil 页面视觉，不做无关布局重写。

### D. Admin engineering/UX

- FAD-P2-01/02/03/04、bundle/test budget、中文标题/环境配置。
- 在 Admin correctness 合入后做，避免同时编辑 shared providers/resource config 造成冲突。

## 验证矩阵

| 层 | 定向验证 | 最终验证 |
| --- | --- | --- |
| Admin | idempotency、permission matrix、session race、mutation retry、contract、decimal、socket tests | lint + typecheck + all Vitest + production build + bundle budget |
| Mobile | deferred market/session/orders、Decimal、batch cancel、private WS、PWA update、keyboard/a11y tests | type-check + all tests + build:pwa + build:tauri + artifact/budget assertions |
| Backend（如改） | 对应 module/unit/route tests | fmt + check + clippy -D warnings |
| Browser | Admin 登录/资源/权限/行情；Mobile 320/390/448 关键路由 | 无溢出、无 console error、焦点/键盘/触控/深浅主题通过 |

## 完成判定

- 审计 ID 有“修复文件 + 行为测试 + 验证命令”证据。
- 不接受只增加源码字符串测试、只隐藏 UI、只改类型却仍在运行时 `Number()` 转换资金值。
- 不接受用空数组、0、pending 或 silent catch 掩盖后端合同/未知状态。
- 不接受运行结果不确定时自动生成新的资金幂等键。
