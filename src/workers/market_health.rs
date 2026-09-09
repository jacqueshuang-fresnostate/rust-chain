//! 行情健康巡检与 K 线断线补偿 worker。
//!
//! 该 worker 使用已有的 `KLINE_RECOVERY_*` 配置和 `kline_recovery::run_once`
//! 扫描 active strategy/internal 交易对：每轮只处理有限数量的策略，单项失败由底层
//! 记录并继续，不因为一次数据库短暂不可用而让巡检任务退出。它不写 Redis、WebSocket
//! 或资金账本，恢复结果仍由现有 Mongo 幂等 upsert 与 MySQL 检查点保护。

use crate::{
    error::AppResult,
    state::AppState,
    workers::kline_recovery::{self, KlineRecoverySummary},
};
use chrono::Utc;
use tokio::time::{Duration, Instant, MissedTickBehavior, interval_at};
use tracing::{info, warn};

/// 约束健康巡检间隔，避免错误配置造成忙循环或长时间不补偿。
/// 现有部署变量仍以秒为单位；上限只影响 worker 调度，不改变配置读取结果。
const MAX_RECOVERY_INTERVAL_SECONDS: u64 = 3_600;

fn recovery_interval_seconds(configured: u64) -> u64 {
    configured.clamp(1, MAX_RECOVERY_INTERVAL_SECONDS)
}

/// 执行一轮 K 线断线扫描，作为循环和测试共用的薄封装。
///
/// `run_once` 本身负责 MySQL/Mongo 依赖检查、策略筛选、幂等写入和检查点乐观更新；
/// 这里只注入当前 UTC 时刻，不读取或修改任何全局状态。
pub async fn run_recovery_once(
    state: &AppState,
    batch_limit: u32,
) -> AppResult<KlineRecoverySummary> {
    kline_recovery::run_once(state, Utc::now(), batch_limit).await
}

/// 以固定间隔持续执行 K 线缺口补偿。
///
/// 首次 tick 延迟一个完整周期，让实时合成 worker 先发布当前分钟；之后使用 `Skip`
/// 策略避免进程暂停恢复时瞬间追赶大量旧轮次。单轮错误只记日志并等待下一周期，
/// 因此连接池短暂重启不会静默丢失后续补偿机会。函数在进程生命周期内持续运行，
/// 正常只会因任务被取消而返回 `Ok(())`。
pub async fn run_recovery_loop(
    state: AppState,
    interval_seconds: u64,
    batch_limit: u32,
) -> AppResult<()> {
    let interval_seconds = recovery_interval_seconds(interval_seconds);
    let period = Duration::from_secs(interval_seconds);
    let mut ticker = interval_at(Instant::now() + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!(
        interval_seconds,
        batch_limit, "K 线健康巡检与断线补偿循环已启动"
    );

    loop {
        ticker.tick().await;
        match run_recovery_once(&state, batch_limit).await {
            Ok(summary) => {
                if summary.scanned > 0 || summary.recovered_candles > 0 || summary.failed > 0 {
                    info!(
                        scanned = summary.scanned,
                        recovered_candles = summary.recovered_candles,
                        skipped = summary.skipped,
                        failed = summary.failed,
                        "K 线健康巡检完成"
                    );
                }
            }
            Err(error) => {
                warn!(%error, "K 线健康巡检暂时失败，将在下一周期重试");
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_market_health_tests.rs"]
mod tests;
