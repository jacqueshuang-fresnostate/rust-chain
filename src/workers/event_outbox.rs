use crate::{error::AppResult, modules::events::EventOutboxService, state::AppState};
use chrono::Utc;
use tokio::time::{Duration, interval};
use tracing::{error, info};

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

pub async fn run_loop(state: AppState, interval_seconds: u64) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        if let Err(error) = run_once(&state).await {
            error!(%error, "事件 outbox 发布周期失败");
        }
    }
}
