use crate::{error::AppResult, modules::events::EventOutboxService, state::AppState};
use chrono::Utc;
use tokio::time::{Duration, interval};
use tracing::{error, info};

/// 单轮发布已提交的 outbox 事件；服务按幂等状态区分成功、重试与死信，业务事务不在本 worker 中重放。
/// 发布失败保持持久化记录供后续周期处理，本入口只输出汇总，不伪造成功确认。
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

/// 按固定间隔轮询 outbox；单周期故障只记录并继续，数据库中的 next-attempt/终态承担崩溃恢复。
pub async fn run_loop(state: AppState, interval_seconds: u64) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        if let Err(error) = run_once(&state).await {
            error!(%error, "事件 outbox 发布周期失败");
        }
    }
}
