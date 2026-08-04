# 移除后台装饰性英文标签

## Goal

后台管理界面保留 HIPPO 品牌标识和必要业务缩写，但不再展示 `HIPPO OPERATIONS`、`OPERATIONS`、`PRODUCTION OPERATIONS`、`SECURE ACCESS` 以及业务卡片中的纯装饰性英文 kicker。

## Requirements

- 登录页环境、安全与标题文案改为中文，不再出现 Operations 英文。
- 侧边栏品牌区域只展示 HIPPO，不展示 OPERATIONS。
- 共享 PageHeader 不展示 HIPPO OPERATIONS kicker。
- Dashboard、KYC、安全策略中已有中文标题时，移除重复英文 kicker。
- 浏览器 document title 使用中文“HIPPO 管理后台”。
- 保留 KYC、API、PC、HIPPO 等品牌或业务缩写。

## Acceptance Criteria

- [x] `web/src` 用户可见源码不再包含目标装饰性英文标签。
- [x] 共享页头与各业务页信息层级保持完整。
- [x] 后台类型检查、Lint、测试、构建与 diff 检查通过。
