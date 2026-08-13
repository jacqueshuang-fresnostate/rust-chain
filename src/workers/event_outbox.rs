//! 事件发件箱投递后台任务。
//!
//! 业务事务只把待发事件写进发件箱表，与业务数据同事务提交，因此事件不会因消息中间件不可用而丢失。
//! 本 worker 定时把这些记录取出投递到 RabbitMQ，并按结果推进为已发布、待重试或死信。
//! 真正的扫描、发送与状态推进都在事件上下文的发件箱服务中实现，这里只负责按周期驱动并记录汇总日志。
//! 投递语义是至少一次：发送与状态标记之间存在崩溃窗口，多实例同时扫描也可能重复发送，
//! 因此下游必须按消息幂等键去重。

use crate::{error::AppResult, modules::events::EventOutboxService, state::AppState};
use chrono::Utc;
use tokio::time::{Duration, interval};
use tracing::{error, info};

/// 单轮按 ID 顺序扫描最多 100 条已提交的 pending/到期 retry outbox，并在 RabbitMQ `basic_publish` future 完成后推进状态。
/// publisher 未启用 broker-confirm 模式，因此 `published` 不代表收到 broker ACK；发送失败按 5 次、固定 30 秒策略落 retry/dead-letter 后继续，仓储错误终止本轮。
/// 已推进前项不回滚，业务事务不会在 worker 中重放；发送/标记崩溃窗口与多实例扫描都要求下游按 message_id 幂等。
pub async fn run_once(state: &AppState) -> AppResult<()> {
    let service = EventOutboxService::from_state(state)?;
    let summary = service.publish_once(Utc::now()).await?;
    info!(
        attempted = summary.attempted,
        published = summary.published,
        retried = summary.retried,
        dead_lettered = summary.dead_lettered,
        "事件 outbox 发布周期完成"
    );

    Ok(())
}

/// 以至少 1 秒间隔轮询 outbox；单周期数据库或 broker 编排错误只记录并进入下一轮，不以未确认消息伪造成功。
/// `next_retry_at`、published/dead-letter 终态和消息幂等键承担崩溃恢复；循环不维护内存游标，多实例重复发布由下游 inbox 去重。
pub async fn run_loop(state: AppState, interval_seconds: u64) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        if let Err(error) = run_once(&state).await {
            error!(%error, "事件 outbox 发布周期失败");
        }
    }
}
