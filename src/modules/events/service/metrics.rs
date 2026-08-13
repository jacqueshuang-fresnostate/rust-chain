//! inbox 消费结果、RabbitMQ disposition、指标与告警。
//!
//! 本文件把「业务处理结果」翻译成三样东西：给 broker 的确认动作、给运维的计数指标、给值班的告警。
//! 关键判断是何时向 broker 确认：只要失败已被持久化到 inbox（转重试或死信），就应当 ACK，
//! 因为本地已经记下待办，让 broker 重投反而制造重复；只有未能落库的处理错误才 reject 并重入队。
//! 格式非法的消息同样 ACK，因为重投永远不可能成功，留在队列里只会无限循环。
//! 本文件全部为纯计算与日志，不访问数据库、不操作 broker、不重执业务逻辑。

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// 单条 inbox 消息的消费终态，是后续确认决策与指标统计的唯一输入。
/// 五个分支覆盖：处理成功、去重命中、报文不可解析、已落重试、已落死信。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumedInboxMessage {
    /// 业务处理成功且 inbox 已推进为已消费终态。
    Consumed,
    /// 命中去重：该消息此前已被消费或已由其他实例持有，本次不重复处理。
    Duplicate,
    /// 报文格式非法无法处理，已按可确认的坏消息跳过，不再重投。
    Malformed,
    /// 业务处理失败但未耗尽重试预算，已落库为待重试并排定下次到期时间。
    Retried {
        /// 已累计的失败次数。
        attempt_count: u32,
        /// 下次可重放时刻，由退避策略给出。
        next_retry_at: DateTime<Utc>,
    },
    /// 重试预算耗尽，已落库为死信，不再自动重放。
    DeadLettered {
        /// 最终失败次数。
        attempt_count: u32,
    },
}

/// 一批消费结果的计数汇总，四项之和即本批处理的消息总数。
/// 注意报文非法被并入重复计数，因此该项同时包含真去重与坏消息两类。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsumedInboxBatch {
    /// 成功消费的条数。
    pub consumed: u32,
    /// 去重命中与格式非法的合计条数。
    pub duplicates: u32,
    /// 转入待重试的条数。
    pub retried: u32,
    /// 转入死信的条数。
    pub dead_lettered: u32,
}

/// 面向运维输出的批次指标快照，在批次计数之外补上总数与需要人工关注的告警列表。
/// 只有重试与死信会产生告警，成功与去重属于正常状态不生成任何条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventInboxMetrics {
    /// 本批处理的消息总数，等于其余四项之和。
    pub total: u32,
    /// 成功消费的条数。
    pub consumed: u32,
    /// 去重与坏消息合计条数。
    pub duplicates: u32,
    /// 转入待重试的条数。
    pub retried: u32,
    /// 转入死信的条数。
    pub dead_lettered: u32,
    /// 按批次聚合出的告警，为空表示本批无需人工关注。
    pub alerts: Vec<EventInboxAlert>,
}

/// 单条 broker delivery 的完整处理结论，把结果、确认动作与告警打包在一起。
/// 消费循环拿到它后只需照做：按确认动作回应 broker、按需发出告警、把结果计入批次。
#[derive(Debug)]
pub struct ProcessedInboxDelivery {
    /// 归类后的消费结果；可确认的坏消息已被折叠为 `Malformed` 而不再是错误。
    pub result: AppResult<ConsumedInboxMessage>,
    /// 应对 broker 采取的确认动作。
    pub disposition: InboxDeliveryDisposition,
    /// 需要发出的告警，`None` 表示本条无需告警。
    pub alert: Option<EventInboxAlert>,
}

/// 一条待发出的运维告警，构造后不会自动输出，需显式调用发出方法。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventInboxAlert {
    /// 告警类别，决定这属于哪种异常。
    pub kind: EventInboxAlertKind,
    /// 严重级别，决定日志用警告还是错误级输出。
    pub severity: EventInboxAlertSeverity,
    /// 涉及的消息条数；单条告警恒为 1，批次聚合告警为该类累计数。
    pub count: u32,
    /// 面向值班人员的中文说明。
    pub message: String,
}

/// 告警类别，四类对应四种需要区分处置的异常。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EventInboxAlertKind {
    /// 存在待重试消息，属于可自愈的暂时性堆积。
    RetryBacklog,
    /// 存在死信消息，自动重试已放弃，必须人工介入。
    DeadLetter,
    /// 处理失败且未能落库，消息将被重投，可能反复出现。
    ProcessingError,
    /// 报文格式异常，已确认跳过，通常意味着上下游契约不一致。
    MalformedDelivery,
}

/// 告警严重级别，仅影响日志输出等级，不改变消息的确认与状态推进。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EventInboxAlertSeverity {
    /// 可自愈或影响有限，用警告级记录。
    Warning,
    /// 需人工介入，用错误级记录。
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
    /// 每类最多产出一条告警且 `count` 为该类累计条数，因此一批里出现十条死信只会告警一次，
    /// 这与逐条处理时产生的单条告警是两套粒度，调用方应避免两者同时输出造成重复噪音。
    /// 成功与去重不产生告警，因为它们属于正常运行结果。
    /// `total` 由四项相加得出，是派生值而非独立计数。
    /// 该纯映射不写数据库、不发通知；重复调用结果一致，实际输出日志需显式调用告警的发出方法。
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
    /// 把一次消费结果加工成完整处理结论，同时算出确认动作与告警。
    /// 关键一步是折叠：当确认动作为 ACK 且错误属于报文非法时，把 `Err` 改写成 `Malformed` 成功值，
    /// 使批次统计能把这类消息计入重复而不是让错误一路上抛；判定顺序上告警在折叠之前生成，
    /// 因此告警仍能区分「格式异常」与普通处理失败。
    /// 只生成决策，不实际 ACK 或 reject，也不推进任何数据库状态；未落库的处理错误保持原样交给 broker 重入队。
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
    /// 取出处理结论中已算好的告警副本，无需重新分类。
    /// 告警在构造处理结论时就已确定，这里只做读取，避免在结果被折叠之后重算导致分类失真。
    /// 返回 `None` 表示本条无需告警；本函数不写日志也不发外部通知。
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

    /// 构造待重试堆积告警，定级为警告，因为消息已落库并会在退避到期后自动重放，属于可自愈情形。
    /// `count` 由调用方给出：逐条处理时为 1，批次聚合时为该批的重试总数。
    fn retry_backlog(count: u32) -> Self {
        Self {
            kind: EventInboxAlertKind::RetryBacklog,
            severity: EventInboxAlertSeverity::Warning,
            count,
            message: "事件 inbox 存在待重试消息".to_owned(),
        }
    }

    /// 构造死信告警，定级为严重，因为自动重试已放弃，消息不再有任何机会被处理，必须人工介入。
    /// `count` 同样支持单条与批次聚合两种粒度。
    fn dead_letter(count: u32) -> Self {
        Self {
            kind: EventInboxAlertKind::DeadLetter,
            severity: EventInboxAlertSeverity::Critical,
            count,
            message: "事件 inbox 存在死信消息".to_owned(),
        }
    }

    /// 构造处理失败告警，定级为严重：失败未能落进 inbox，消息会被 broker 重投并可能持续复现。
    /// 条数固定为 1，因为这类告警只在逐条处理时产生，不参与批次聚合。
    fn processing_error() -> Self {
        Self {
            kind: EventInboxAlertKind::ProcessingError,
            severity: EventInboxAlertSeverity::Critical,
            count: 1,
            message: "事件 inbox 投递处理失败，将重新入队".to_owned(),
        }
    }

    /// 构造报文异常告警，定级为警告：消息已被确认跳过不会堆积，但通常说明上下游消息契约已不一致。
    /// 条数固定为 1，仅在逐条处理时产生。
    fn malformed_delivery() -> Self {
        Self {
            kind: EventInboxAlertKind::MalformedDelivery,
            severity: EventInboxAlertSeverity::Warning,
            count: 1,
            message: "事件 inbox 投递格式异常，已确认跳过".to_owned(),
        }
    }

    /// 以结构化 tracing 级别发出告警：Warning 使用 warn，Critical 使用 error。
    /// 两个分支的字段集合与事件名完全相同，只有级别不同，便于日志系统按统一规则采集后再按级别分流。
    /// 类别、条数与说明作为结构化字段输出而非拼进正文，使告警可被直接聚合统计。
    /// 这是本文件唯一的副作用，不发送外部通知也不修改 inbox 状态。
    /// 调用方需自行避免重复发出：逐条告警与批次聚合告警会覆盖同一批消息，同时输出会造成重复计数。
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

/// 对 broker delivery 的确认动作，只有两种取值。
/// 判定原则是「本地是否已记下这条消息的后续处置」：记下了就 ACK，没记下才让 broker 重投。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxDeliveryDisposition {
    /// 确认收妥，broker 不再重投；用于全部已落库的终态与可跳过的坏消息。
    Ack,
    /// 拒收并要求重新入队，用于未能落库的处理错误，让 broker 稍后再投一次。
    RejectRequeue,
}

impl InboxDeliveryDisposition {
    /// 决定 ACK 或 reject+requeue：终态、持久化 retry 与坏消息均 ACK，处理错误重入队。
    /// 判定依据是本地是否已记下该消息的后续处置，已记下就没有让 broker 重投的必要。
    /// 唯一走重入队的是未落库的处理错误，此时本地没有任何记录，只能靠 broker 再投一次以免丢失。
    /// 该纯决策不实际确认或拒收消息，也不推进任何数据库状态。
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

/// 识别「重投也不可能成功」的报文级错误，命中者应被确认跳过而非重入队。
/// 判定范围严格限定为三类：载荷 JSON 无法解析、缺少消息标识、缺少幂等键，
/// 这三者都源于消息本身的结构缺陷，重试多少次结果都一样。
/// 通过错误前缀与固定文案匹配，因此改动这些校验错误的措辞会静默改变确认行为，需同步更新此处。
/// 其余任何错误都视为可恢复故障，交由 broker 重投。
fn is_malformed_delivery_error(error: &AppError) -> bool {
    matches!(error, AppError::Validation(message) if message.starts_with("invalid event payload json:") || message == "event message_id is required" || message == "event idempotency_key is required")
}
