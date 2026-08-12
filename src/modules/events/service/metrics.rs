//! inbox 消费结果、RabbitMQ disposition、指标与告警。

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumedInboxMessage {
    Consumed,
    Duplicate,
    Malformed,
    Retried {
        attempt_count: u32,
        next_retry_at: DateTime<Utc>,
    },
    DeadLettered {
        attempt_count: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsumedInboxBatch {
    pub consumed: u32,
    pub duplicates: u32,
    pub retried: u32,
    pub dead_lettered: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventInboxMetrics {
    pub total: u32,
    pub consumed: u32,
    pub duplicates: u32,
    pub retried: u32,
    pub dead_lettered: u32,
    pub alerts: Vec<EventInboxAlert>,
}

#[derive(Debug)]
pub struct ProcessedInboxDelivery {
    pub result: AppResult<ConsumedInboxMessage>,
    pub disposition: InboxDeliveryDisposition,
    pub alert: Option<EventInboxAlert>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventInboxAlert {
    pub kind: EventInboxAlertKind,
    pub severity: EventInboxAlertSeverity,
    pub count: u32,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EventInboxAlertKind {
    RetryBacklog,
    DeadLetter,
    ProcessingError,
    MalformedDelivery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EventInboxAlertSeverity {
    Warning,
    Critical,
}

impl ConsumedInboxBatch {
    /// 把一条消费终态累计进当前批次；Malformed 延续既有重复计数语义。
    /// 仅修改内存计数，不持久化、不告警；每条实际处理结果只能调用一次，避免重复统计。
    pub(super) fn record(&mut self, result: ConsumedInboxMessage) {
        match result {
            ConsumedInboxMessage::Consumed => self.consumed += 1,
            ConsumedInboxMessage::Duplicate | ConsumedInboxMessage::Malformed => {
                self.duplicates += 1;
            }
            ConsumedInboxMessage::Retried { .. } => self.retried += 1,
            ConsumedInboxMessage::DeadLettered { .. } => self.dead_lettered += 1,
        }
    }

    /// 将批次计数转换为运维指标，并只为 retry backlog 与 dead-letter 生成聚合告警。
    /// 该纯映射不写数据库、不发通知；重复调用一致，日志需显式调用 alert.emit。
    pub fn metrics(&self) -> EventInboxMetrics {
        // 将批次结果转成运维快照，并只对需要人工关注的重试/死信生成告警。
        let mut alerts = Vec::new();
        if self.retried > 0 {
            alerts.push(EventInboxAlert::retry_backlog(self.retried));
        }
        if self.dead_lettered > 0 {
            alerts.push(EventInboxAlert::dead_letter(self.dead_lettered));
        }

        EventInboxMetrics {
            total: self.consumed + self.duplicates + self.retried + self.dead_lettered,
            consumed: self.consumed,
            duplicates: self.duplicates,
            retried: self.retried,
            dead_lettered: self.dead_lettered,
            alerts,
        }
    }
}

impl ProcessedInboxDelivery {
    /// 将消费结果归类为 broker disposition，并把可确认的坏消息折叠为 Malformed 终态。
    /// 只生成决策，不实际 ACK/reject 或写状态；普通处理错误保留给 broker 重入队。
    pub fn from_result(result: AppResult<ConsumedInboxMessage>) -> Self {
        let disposition = InboxDeliveryDisposition::from_result(&result);
        let alert = EventInboxAlert::from_delivery_result(&result);
        let result = if disposition == InboxDeliveryDisposition::Ack
            && matches!(result, Err(ref error) if is_malformed_delivery_error(error))
        {
            Ok(ConsumedInboxMessage::Malformed)
        } else {
            result
        };

        Self {
            result,
            disposition,
            alert,
        }
    }
}

impl EventInboxAlert {
    /// 提取已处理 delivery 的预分类告警；无告警返回 None，不记录日志或外部通知。
    pub fn from_processed_delivery(processed: &ProcessedInboxDelivery) -> Option<Self> {
        processed.alert.clone()
    }

    /// 从消费结果映射 retry/dead-letter/坏消息/处理错误告警；正常和重复终态不告警。
    /// 该纯函数不 ACK、不持久化、不输出日志。
    pub fn from_delivery_result(result: &AppResult<ConsumedInboxMessage>) -> Option<Self> {
        match result {
            Ok(ConsumedInboxMessage::Retried { .. }) => Some(Self::retry_backlog(1)),
            Ok(ConsumedInboxMessage::DeadLettered { .. }) => Some(Self::dead_letter(1)),
            Err(error) if is_malformed_delivery_error(error) => Some(Self::malformed_delivery()),
            Err(_) => Some(Self::processing_error()),
            Ok(
                ConsumedInboxMessage::Consumed
                | ConsumedInboxMessage::Duplicate
                | ConsumedInboxMessage::Malformed,
            ) => None,
        }
    }

    fn retry_backlog(count: u32) -> Self {
        Self {
            kind: EventInboxAlertKind::RetryBacklog,
            severity: EventInboxAlertSeverity::Warning,
            count,
            message: "事件 inbox 存在待重试消息".to_owned(),
        }
    }

    fn dead_letter(count: u32) -> Self {
        Self {
            kind: EventInboxAlertKind::DeadLetter,
            severity: EventInboxAlertSeverity::Critical,
            count,
            message: "事件 inbox 存在死信消息".to_owned(),
        }
    }

    fn processing_error() -> Self {
        Self {
            kind: EventInboxAlertKind::ProcessingError,
            severity: EventInboxAlertSeverity::Critical,
            count: 1,
            message: "事件 inbox 投递处理失败，将重新入队".to_owned(),
        }
    }

    fn malformed_delivery() -> Self {
        Self {
            kind: EventInboxAlertKind::MalformedDelivery,
            severity: EventInboxAlertSeverity::Warning,
            count: 1,
            message: "事件 inbox 投递格式异常，已确认跳过".to_owned(),
        }
    }

    /// 以结构化 tracing 级别发出告警：Warning 使用 warn，Critical 使用 error。
    /// 这是唯一日志副作用，不发送外部通知、不修改 inbox；调用方应避免重复 emit。
    pub fn emit(&self) {
        match self.severity {
            EventInboxAlertSeverity::Warning => tracing::warn!(
                kind = ?self.kind,
                count = self.count,
                message = %self.message,
                "事件 inbox 告警"
            ),
            EventInboxAlertSeverity::Critical => tracing::error!(
                kind = ?self.kind,
                count = self.count,
                message = %self.message,
                "事件 inbox 告警"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxDeliveryDisposition {
    Ack,
    RejectRequeue,
}

impl InboxDeliveryDisposition {
    /// 决定 ACK 或 reject+requeue：终态、持久化 retry 与坏消息均 ACK，处理错误重入队。
    /// 该纯决策不实际操作 delivery，也不推进数据库状态。
    pub fn from_result(result: &AppResult<ConsumedInboxMessage>) -> Self {
        match result {
            Ok(ConsumedInboxMessage::Retried { .. }) => Self::Ack,
            Err(error) if is_malformed_delivery_error(error) => Self::Ack,
            Err(_) => Self::RejectRequeue,
            Ok(
                ConsumedInboxMessage::Consumed
                | ConsumedInboxMessage::Duplicate
                | ConsumedInboxMessage::Malformed
                | ConsumedInboxMessage::DeadLettered { .. },
            ) => Self::Ack,
        }
    }
}

fn is_malformed_delivery_error(error: &AppError) -> bool {
    matches!(error, AppError::Validation(message) if message.starts_with("invalid event payload json:") || message == "event message_id is required" || message == "event idempotency_key is required")
}
