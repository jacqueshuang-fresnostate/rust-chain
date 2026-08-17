# 后台模拟行情参数配置技术设计

## 权威数据模型

- `market_strategies` 保留当前策略概要与节点，`strategy_runs.active_version` 指向正在使用的不可变版本。
- `strategy_versions.config_json` 新增 `generator` 对象，保存场景代码、seed 模式、均值回归强度、噪声强度、影线强度与成交量形态；实际 seed 继续保存到既有 `strategy_versions.seed` 列。
- 旧快照没有 `generator` 时使用兼容默认值：`custom_path / auto / 0.55 / 1 / 0.75 / uniform`，确保旧算法输出不漂移。
- 创建、编辑、回滚都生成新版本；不修改历史版本行，不重写 MongoDB 历史 K 线。

## API 合同

### 场景预设

`GET /admin/api/v1/market-strategies/presets`

返回后端权威中文目录。每个预设包含场景代码、名称、说明、显式高级参数默认值和建议节点模板。后台应用预设后仍提交完整参数，因此版本快照不依赖未来可能变化的预设实现。

### 创建与修改

现有创建/修改请求增加 `generator`：

```json
{
  "scenario": "custom_path",
  "seed_mode": "auto",
  "seed": null,
  "regenerate_seed": false,
  "mean_reversion_strength": "0.55",
  "noise_scale": "1",
  "wick_scale": "0.75",
  "volume_shape": "uniform"
}
```

- 创建时 `auto` 生成 UUIDv7 seed；`fixed` 使用请求 seed。
- 修改时 `auto + regenerate_seed=false` 继承激活版本 seed，`auto + regenerate_seed=true` 生成新 seed；`fixed` 使用请求 seed。
- 服务端负责枚举、数值范围、seed 长度、节点时间/价格和状态冲突校验。

### 无副作用预览

`POST /admin/api/v1/market-strategies/preview`

- 接收与创建表单等价的完整草稿和 `sample_count`。
- 只读取交易对目录，不写 MySQL、MongoDB、Redis、WebSocket 或 worker checkpoint。
- 最多返回 240 根均匀采样的 1m 蜡烛、总分钟数和实际 `preview_seed`。

### 版本历史与回滚

- `GET /admin/api/v1/market-strategies/:id/versions`
- `POST /admin/api/v1/market-strategies/:id/versions/:version/restore`

回滚必须提供审计原因并要求策略非 `active`。服务端在事务内锁定策略与运行状态，复制目标版本快照/seed 为递增新版本，同步策略概要和节点，更新 `active_version`，写入策略事件及管理员审计。

## 生成器语义

- `mean_reversion_strength` 替换当前桥接扰动系数 `0.55`；扰动包络在锚点首尾归零，因此强度控制锚点区间内偏离并始终回归目标路径。
- `noise_scale` 乘到确定性价格噪声。
- `wick_scale` 替换当前影线系数 `0.75`。
- `volume_shape` 在既有确定性体积噪声基础上施加全局时间形态：均匀、递增、钟形、尾部放量。
- `scenario` 是版本元数据与后台预设来源；实际输出完全由显式节点和高级参数决定，避免隐藏规则。

## 模块边界

- 行情领域层持有枚举、默认值与 OHLCV 生成算法。
- 一个共享快照解析器负责把版本行转换为 `SyntheticMarketConfig`，实时 worker 与手动恢复必须复用它。
- 后台 presentation 只定义 DTO，service 做纯校验/规范化，application 编排事务与预览，infrastructure 只做 SQL。
- 路由保持薄层，不直接拼 SQL 或生成行情。

## 后台交互

- `/admin/market/strategies` 使用完整设置中心；旧 `/admin/market/strategies/actions` 只保留兼容重定向。
- 移除重复的“策略动作”导航项。
- 创建/编辑面板增加预设、高级参数、seed 控件、预览入口；列表增加版本历史入口和复制回滚操作。
- 保留既有启用、停用、删除、缺口检测与手动补偿能力。

## 测试重点

- 旧快照输出兼容、各高级参数实际影响输出、相同 seed 可重放。
- worker 与手动恢复对同一快照得到同一配置。
- 预览无副作用、采样上限和 seed 重放。
- 回滚的版本递增、快照复制、状态冲突、审计原因和事务原子性。
- 后台路由合并、请求负载、预设应用、预览和版本回滚交互。
