//! events bounded context presentation layer.
//!
//! 表现层：定义事件运维接口的请求响应 DTO，以及 WebSocket 握手与订阅指令的解析结构。
//! 分页与状态筛选的归一化统一在本层完成，应用层只消费已规整的参数，不再重复夹取边界。
//! 所有时间字段序列化为毫秒时间戳，可空时间用专门的可选序列化模块以保证空值输出为 null。
//! 响应类型刻意不含事件载荷、消息载荷与处理令牌，运维面板只看状态与路由信息。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 私有 WebSocket 握手查询串。
/// 令牌走查询串而非请求头，是因为浏览器 WebSocket API 无法自定义头部；
/// 字段可空只是反序列化形态，缺失或空串在鉴权阶段都会被判为未授权。
#[derive(Debug, Deserialize)]
pub(crate) struct PrivateWsQuery {
    /// 访问令牌；用户私有端点要求 user scope，代理私有端点要求 agent scope，两者都必须未被撤销。
    pub(crate) token: Option<String>,
}

/// 公共 WebSocket 连接建立后客户端下发的订阅指令。
/// 多频道端点靠它在连接期间动态增减订阅，单频道端点的订阅在握手时已固定。
#[derive(Debug, Deserialize)]
pub(crate) struct PublicWsCommand {
    /// 操作类型，用于区分订阅与退订。
    pub(crate) op: String,
    /// 目标频道名。
    pub(crate) channel: String,
    /// 交易对符号，行情类频道需要，其余频道可省略。
    pub(crate) symbol: Option<String>,
    /// K 线周期，仅 K 线频道需要。
    pub(crate) interval: Option<String>,
}

#[derive(Debug, Deserialize)]
/// 管理端事件列表查询；分页和空状态由表现层统一规范化。
/// outbox 与 inbox 两个列表接口共用该结构，因此状态取值的合法集合取决于查询目标。
pub(crate) struct EventRecordsQuery {
    /// 按状态精确筛选，空白或缺失表示不限状态。
    pub(crate) status: Option<String>,
    /// 单页条数，缺省 50。
    pub(crate) limit: Option<u32>,
    /// 分页偏移，缺省 0。
    pub(crate) offset: Option<u32>,
}

impl EventRecordsQuery {
    /// 将 HTTP 查询参数规范化为应用层输入，统一空状态与分页边界。
    /// 状态裁剪首尾空白，裁剪后为空串降级为 `None`，避免空白串被当成真实筛选条件而查不到数据。
    /// 条数缺省 50 并夹到 1 至 100，防止零条空转与超大结果集；偏移缺省 0 并截断到 100000，
    /// 因为事件表持续增长，超大偏移会退化为全表扫描加文件排序。
    /// 归一只做边界收敛，不校验状态字面量是否属于合法状态集合。
    pub(crate) fn normalize(self) -> EventRecordListParams {
        EventRecordListParams {
            status: self
                .status
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            limit: self.limit.unwrap_or(50).clamp(1, 100),
            offset: self.offset.unwrap_or(0).min(100_000),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 应用层可直接消费的事件列表查询参数，边界已收敛完毕。
/// 与原始查询串的差别是条数与偏移不再可空，取值也已落在允许范围内。
pub(crate) struct EventRecordListParams {
    /// 已裁剪的状态筛选，`None` 表示不限状态。
    pub(crate) status: Option<String>,
    /// 单页条数，取值范围 1 至 100。
    pub(crate) limit: u32,
    /// 分页偏移，不超过 100000。
    pub(crate) offset: u32,
}

#[derive(Debug, Serialize)]
/// 事件运维列表响应，顶层 JSON 合同固定为 `{records,total}`。
/// 泛型使 outbox 与 inbox 两类记录复用同一外层结构，前端只需实现一套分页解析。
pub(crate) struct EventRecordsResponse<T> {
    /// 当前页的记录数组。
    pub(crate) records: Vec<T>,
    /// 符合筛选条件的总数，不受分页参数影响。
    pub(crate) total: i64,
}

impl<T> EventRecordsResponse<T> {
    /// 构造事件运维列表合同，固定保留 `records` 与 `total` 两个顶层字段。
    /// 之所以提供构造函数而不让调用方直接建结构体，是为了让两个列表用例走同一入口，
    /// 避免今后新增外层字段时漏改其中一处。本函数不做任何校验或截断。
    pub(crate) fn new(records: Vec<T>, total: i64) -> Self {
        Self { records, total }
    }
}

#[derive(Debug, Serialize)]
/// outbox 运维记录响应，同时作为死信重排成功合同。
/// 两个接口共用同一形状，重排成功后前端可直接用返回值替换列表中的对应行。
/// 不含事件载荷，运维视角只关心投递状态与路由信息。
pub(crate) struct OutboxRecordResponse {
    /// 事件主键，死信重排接口以它定位目标。
    pub(crate) id: u64,
    /// 聚合类型，标明事件所属的业务对象类别。
    pub(crate) aggregate_type: String,
    /// 聚合实例标识。
    pub(crate) aggregate_id: String,
    /// 事件类型名，消费方据此分派处理。
    pub(crate) event_type: String,
    /// 消息路由键，决定投递到哪些队列。
    pub(crate) routing_key: String,
    /// 发布状态，取值为 pending、retry、published 或 dead_letter。
    pub(crate) status: String,
    /// 已累计的发布失败次数，达到策略阈值后转入死信。
    pub(crate) retry_count: i32,
    #[serde(with = "crate::time::option_unix_millis")]
    pub(crate) next_retry_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::time::option_unix_millis")]
    pub(crate) published_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::time::unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
/// inbox 运维记录响应，用于排查某个消费者的失败与积压。
/// 不含消息载荷与处理令牌：前者体量大，后者是并发租约凭据，暴露出去会让外部具备干扰消费的可能。
pub(crate) struct InboxRecordResponse {
    /// 记录主键。
    pub(crate) id: u64,
    /// 消费者名称，同一条消息可被多个消费者各自独立去重与消费。
    pub(crate) consumer_name: String,
    /// 消息标识，与消费者名共同构成主要去重键。
    pub(crate) message_id: String,
    /// 消费状态，取值为 processing、consumed、retry 或 dead_letter。
    pub(crate) status: String,
    /// 已累计的消费失败次数。
    pub(crate) retry_count: i32,
    /// 最近一次失败的错误摘要，成功消费后会被清空。
    pub(crate) error_message: Option<String>,
    #[serde(with = "crate::time::option_unix_millis")]
    pub(crate) consumed_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::time::unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
/// outbox 死信重排请求；`reason` 将写入管理员审计日志。
/// 请求体只有原因一项，重排目标由路径参数给出，重排后的状态与次数由服务端固定语义决定，不接受客户端指定。
pub(crate) struct RequeueOutboxRequest {
    /// 重排原因，字段可空但业务上必填，会与状态变更同事务写入审计。
    pub(crate) reason: Option<String>,
}

impl RequeueOutboxRequest {
    /// 规范化死信重排原因；运维干预必须留下非空且可追溯的审计说明。
    /// 缺省与纯空白等同处理，裁剪后为空即返回 `AppError::Validation`，因此不会出现有重排无理由的审计记录。
    /// 校验发生在开启事务之前，非法请求不会占用数据库事务，也不会改动任何事件状态。
    /// 此处只判非空，不限制长度也不做内容过滤。
    pub(crate) fn require_reason(&self) -> crate::error::AppResult<String> {
        let reason = self.reason.as_deref().unwrap_or_default().trim().to_owned();
        if reason.is_empty() {
            return Err(crate::error::AppError::Validation(
                "reason is required".to_owned(),
            ));
        }
        Ok(reason)
    }
}
