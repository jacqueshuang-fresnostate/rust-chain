# P0-03 合成行情与秒合约结算闭环研究

## 现状与风险

- 外部行情 worker 已把 ticker 归档到 `market_price_ticks`，事件键包含来源、交易对、观察时间和价格，
  然后才进入 Redis/触发器/广播链路。
- 合成行情从 `synthetic_market` 进入 `MarketIngestionService::ingest_and_publish_synthetic_ticker`，
  当前只做 Redis CAS、现货/杠杆触发和 WS 广播，没有写 `market_price_ticks`。
- 秒合约只使用事件时间窗口内的 `market_price_ticks` 作为结算证据；因此合成交易对可以展示实时价，
  但到期后找不到可审计的结算快照。
- 结算 worker 对缺少快照的到期订单持续延期，状态可以永久停在 `opened`。

## 合成 ticker 归档设计

1. 合成计划向 ingestion 传递 `strategy_id`、`active_version`、`lease_owner`。
2. 写归档时在短 MySQL 事务中锁定对应 `strategy_runs`，再次验证：
   - owner 相同；
   - `active_version` 相同；
   - `run_status` 可运行；
   - 租约尚未过期。
3. 插入 `market_price_ticks`：
   - `source='strategy'`；
   - `generation=active_version`；
   - `source_version='strategy:{strategy_id}:v{version}'`；
   - `event_key` 对来源、交易对、事件时间、规范化价格、版本和来源版本做 SHA-256。
4. 事件键唯一冲突按成功回放处理，禁止重复写同一快照。
5. MySQL 持锁复核与归档必须先于 Redis CAS；过期 owner、旧版本或非法事件时间不得留下 Redis ticker。
6. MySQL 归档已提交、Redis 后失败时，完全相同的时间与载荷重试必须复用单条归档并补齐缓存；
   同时间但不同载荷仍按冲突/陈旧拒绝。归档成功后才运行资金触发器和广播，已归档事件不重放这些副作用。

## 产品与下单 fail-closed

- 秒合约产品从非 active 切换为 active，以及用户开单锁定产品时，都必须调用同一“结算历史能力”校验。
- 对策略行情交易对，必须存在当前有效的策略运行记录、版本与可运行状态，同时 `lease_owner` 非空且 `lease_expires_at` 未过期。
- 对外部行情交易对，必须被启用的 `market_feed_configs` 覆盖且至少配置一个受支持 provider。
- 无法证明会产生 `market_price_ticks` 的交易对不得激活产品，也不得开新单；返回明确校验错误。

## 超龄订单异常终态

1. 新增可配置的最大等待时长，超过时不猜价、不使用 Redis 当前价替代事件时间快照。
2. 通过条件更新把订单转为 `manual_review`，记录失败代码、失败时间和最后一次查找窗口。
3. 写入独立、追加式结算异常记录，唯一关联订单，形成可审计证据。
4. `manual_review` 是受控运营终态：不继续自动结算，也不自动改动钱包；后续人工退款需要独立、强幂等资金命令，
   本 P0 不在缺少佣金冲正设计时擅自退款。
5. 产品恢复能力后，新订单正常进入事件时间结算；历史人工审核订单由后台流程单独处置。

## 验证计划

- 合成 ticker 正常归档、重复回放不重复、过期/错误 lease 被拒绝。
- 模拟 MySQL 归档已写、Redis 首次失败，重试后复用归档并补齐缓存；过期 lease 在 Redis 中不留孤儿值。
- 无归档能力的产品激活/开单 fail-closed；有效外部源与有效策略源允许。
- 秒合约在目标事件窗口选择确定性快照并结算；没有快照的超龄订单只转一次 `manual_review` 并写一条异常记录。
- worker 重启后重复扫描不重复结算、不重复转异常状态。

## 所有权建议

- 实现代理独占合成行情 ingestion、秒合约产品/下单/结算 worker、新迁移和相关后端测试。
- 不修改移动端当前脏文件、资金幂等迁移、PC 构建配置、公共进度文件与 Trellis 任务文件。
