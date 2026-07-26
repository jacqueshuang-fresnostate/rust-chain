# 钱包链网关契约

## 边界

Rust 业务服务只管理用户余额、冻结资金、提现审核和链事件幂等，不保存公链私钥。签名、广播、节点访问和区块扫描由独立链网关完成。

链网关配置存储在 `wallet_chain_gateways`：

- `network` 必须与 `deposit_network_configs.network` 及提现请求网络一致。
- `broadcast_url` 用于提现广播。
- `event_poll_url` 用于轮询充值和提现回执。
- `auth_token_encrypted` 使用服务端 `credential_encryption_key` 加密。
- 只有 `status = active` 的配置参与 worker。

worker 默认启动，可通过以下环境变量控制：

```text
WALLET_CHAIN_WORKER_ENABLED=true
WALLET_CHAIN_WORKER_INTERVAL_SECONDS=5
WALLET_CHAIN_WORKER_BATCH_LIMIT=50
WALLET_CHAIN_WORKER_MAX_ATTEMPTS=5
```

## 提现广播

业务服务向 `broadcast_url` 发送 `POST` JSON。重试始终复用同一个 `request_id`，网关必须按该字段幂等，不能重复创建链上交易。

```json
{
  "request_id": "019c...",
  "network": "tron",
  "asset_symbol": "USDT",
  "address": "T...",
  "amount": "100.000000",
  "fee": "1.000000"
}
```

成功响应：

```json
{
  "tx_hash": "abc123",
  "block_height": 123456,
  "confirmations": 0
}
```

`tx_hash` 必须非空、不能包含空白且最长 255 字符。广播前的终态失败会解冻用户资金；已经返回交易哈希后不允许自动解冻。

## 链事件轮询

业务服务向 `event_poll_url` 发送：

```text
GET ?cursor=<opaque>&limit=50
Authorization: Bearer <token>
```

响应中的 `cursor` 是不透明值。只有整页充值和提现回执全部处理成功后，业务服务才推进游标。

```json
{
  "next_cursor": "opaque-next-cursor",
  "withdrawals": [
    {
      "request_id": "019c...",
      "network": "tron",
      "tx_hash": "abc123",
      "block_height": 123456,
      "confirmations": 20,
      "status": "confirmed",
      "failure_reason": null
    }
  ],
  "deposits": [
    {
      "asset_symbol": "USDT",
      "network": "tron",
      "address": "T...",
      "memo": null,
      "tx_hash": "deposit123",
      "event_index": 0,
      "amount": "50.000000",
      "block_height": 123450,
      "confirmations": 20
    }
  ]
}
```

提现回执状态只接受：

- `broadcasted`：保存交易哈希并更新确认数。
- `confirmed`：从冻结余额完成最终扣除；重复回执不会重复扣账。
- `failed`：未取得交易哈希时自动解冻；已广播时进入 `manual_review` 并继续冻结，后续真实 `confirmed` 回执仍可完成扣账。

充值以 `(network, tx_hash, event_index)` 去重，达到网络要求确认数后只入账一次。链重组通过后台冲正接口处理；可用余额不足时进入人工处理，禁止产生负余额。

## 运维要求

- 链网关、节点和业务数据库必须使用独立凭据并限制网络访问。
- 上线前必须在真实 MySQL、Redis 和测试链环境验证重复广播、重复确认、网关超时、链重组与服务重启恢复。
- `request_id`、交易哈希和事件游标必须进入集中日志，但不得记录私钥、助记词或明文网关令牌。
