# 线上新币资产元数据缺口

## 2026-09-05 运行时证据

- `GET https://hipoex.cllbmz.kdns.fr/api/v1/new-coins?limit=50` 返回的 HIPPO 项目包含 `asset_id=5` 与 `quote_asset_id=2`，但尚未返回 `name`、`logo_url`、`quote_asset_symbol`、`quote_asset_logo_url`。
- `/api/v1/markets` 能返回已上市交易对的基础/计价资产 Logo，但当前 HIPPO 尚未配置后上市交易对，因此该目录不能作为打新项目 Logo 的权威回退。
- `/api/v1/wallet/deposit-assets` 与 `/api/v1/wallet/withdraw-assets` 需要用户令牌，而且会受充提开关过滤；它们不能作为公开打新专区的资产目录。

## 结论

打新专区必须继续以新币公开列表/详情 DTO 自身携带的资产元数据为唯一公开来源。当前工作树的后端列表与详情已经通过 `asset_id`、`quote_asset_id` 联表 `assets`，分别输出项目资产 `logo_url` 与计价资产 `symbol`；Mobile 直接映射并展示这些字段，不按符号拼 URL，也不把某个固定币种写死为计价单位。

线上页面在新后端镜像部署前仍会收到旧合同，只能诚实显示回退标记；部署包含本任务改动的后端镜像后，项目 Logo 和发行价计价符号会随同一项目响应出现，无需额外私有请求。
