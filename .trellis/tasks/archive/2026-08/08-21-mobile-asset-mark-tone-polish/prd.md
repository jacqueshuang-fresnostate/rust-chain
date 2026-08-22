# 优化手机端资产标识材质

## Goal

优化生产手机端共享 `AssetMark`，重点修复 `asset-mark--tone-2` 在真实资产图片外层仍附加色调、描边和高光的问题。后台 Logo 成功加载时只显示完整圆形图片，不增加高光、渐变、描边、内环或阴影，同时保留无图片时可识别的符号首字母回退。

## What I already know

- `AssetMark` 位于 `mobile/src/components/AssetMark.vue`，被资产、行情、交易、闪兑和借贷等多个页面复用。
- 色调由资产符号哈希决定；`BTC` 当前命中 `tone-2`，而 `tone-2` 直接使用全局绿色 `--accent`。
- 即使后台图片成功加载，组件仍应用哈希色调的背景、描边和内环；真实 Logo 因此会被无关色调包裹。
- 合约交易头部还通过 `TradeView.vue` 强制应用绿色描边，导致真实 Logo 外观被页面局部样式污染。
- 后台返回的 Logo 必须保持权威；图片缺失或加载失败时只能显示精确资产符号首字母，不得推测或引入外部 Logo。

## Assumptions

- 本次只调整共享资产标识及其交易头部局部覆盖，不改变资产数据、路由、接口或业务行为。
- 已加载图片应只做圆形裁切，不再受符号哈希色调或装饰材质影响。
- `tone-2` 仅服务无图回退态，并继续使用现有主题语义色，不写死为某一种资产品牌色。

## Requirements

- 为 `AssetMark` 明确区分图片态和首字母回退态。
- 图片态只保留圆形裁切和现有尺寸，不改变后台图片内容，不添加背景高光、渐变、描边、内环或阴影。
- 回退态保留确定性哈希色调，并使用简洁的主题色平面圆形，不添加高光或阴影。
- 首字母字号随 `size` 合理缩放，24–54px 常用尺寸均保持居中、可读。
- 删除交易头部对 `AssetMark` 的绿色描边覆盖，让共享圆形 Logo 外观生效。
- 明暗主题均保持清晰边界、对比度和无水平溢出。

## Acceptance Criteria

- [x] 后台 Logo 加载成功时，只显示圆形图片，不出现高光、渐变、描边、内环、阴影或 `tone-2` 色圈。
- [x] 图片缺失或连续加载失败时，仍显示简洁的主题色资产符号首字母。
- [x] 交易页头部不再覆盖共享资产标识为绿色描边样式，28px 几何尺寸保持不变。
- [x] 图片来源、失败递进、无图回退和可访问名称的现有行为不变。
- [x] 相关自动化测试、类型检查及 PWA/Tauri 构建通过。
- [x] 在 390x844 手机视口的浅色和深色主题中完成运行时检查，无水平溢出。

## Definition of Done

- 测试已补充或更新。
- `npm run type-check`、`npm test`、`npm run build:pwa`、`npm run build:tauri` 通过。
- 相关移动端规范与 `docs/superpowers/PROGRESS.md` 已更新。

## Out of Scope

- 不修改后台 Logo 上传、资产接口、行情数据或交易业务。
- 不重新设计页面头部、交易布局或其他按钮。
- 不引入第三方图标库、外部资产 Logo 服务或新依赖。

## Technical Notes

- 主要实现：`mobile/src/components/AssetMark.vue`。
- 局部覆盖：`mobile/src/views/TradeView.vue`。
- 规范：`.trellis/spec/mobile/pwa-and-shell.md`、`.trellis/spec/mobile/backend-integration.md`。
- 保持现有 `buildAssetMarkImageSources` / `assetMarkImageSourceAt` 递进逻辑不变。
