//! 默认来源的持锁归档，复用既有缓存和广播协议；默认来源不伪造人工策略 ID。

use super::*;

/// 交易对共同互斥证据；归档时同时核对数据库命名锁、持久化代际和 worker 身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairTickerFence {
    pub pair_id: u64,
    pub generation: u64,
    pub lock_name: String,
    pub connection_id: u64,
    pub owner: String,
}

/// 默认 ticker 的来源版本；该版本来自已保存默认配置，不使用人工策略身份占位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultTickerProvenance {
    pub fence: PairTickerFence,
    pub config_version: u32,
}

impl MarketIngestionService {
    /// 默认 ticker 先核对交易对所有者、配置及人工排期优先级，再幂等归档；随后才写缓存并调用已有价格消费者。
    /// 模拟盘口/成交仍只走展示链，不生成真实委托；同事件重试不重复触发价格消费者。
    pub async fn ingest_and_publish_default_ticker(
        &self,
        snapshot: &MarketTickerSnapshot,
        provenance: &DefaultTickerProvenance,
    ) -> AppResult<SyntheticIngestionOutcome> {
        let pool = self.mysql.as_ref().ok_or_else(|| {
            AppError::Internal("mysql is required for default market ingestion".into())
        })?;
        let archived = archive_default_ticker(pool, snapshot, provenance).await?;
        let entry = MarketTickerCacheEntry::from_snapshot(snapshot)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let outcome = self
            .cache
            .save_ticker_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if outcome == MarketCacheWriteOutcome::RejectedStale {
            return Ok(SyntheticIngestionOutcome::RejectedStale);
        }
        if archived == SyntheticTickerArchiveOutcome::Inserted {
            self.trigger_spot_limit_orders(
                snapshot.symbol(),
                snapshot.last_price(),
                "default ticker",
            )
            .await;
            self.trigger_margin_limit_orders(snapshot.symbol(), snapshot.last_price())
                .await;
            self.publish(MarketFeedEvent::from_ticker_snapshot(snapshot)?)?;
        }
        Ok(
            if archived == SyntheticTickerArchiveOutcome::Inserted
                && outcome == MarketCacheWriteOutcome::Accepted
            {
                SyntheticIngestionOutcome::Accepted
            } else {
                SyntheticIngestionOutcome::ReplayedIdentical
            },
        )
    }
}

/// 在已持有交易对行锁的事务中核对命名锁和运行代际；租约与事件时间均使用数据库时钟。
pub(super) async fn verify_pair_fence(
    tx: &mut Transaction<'_, MySql>,
    fence: &PairTickerFence,
    source: &str,
    observed_at: DateTime<Utc>,
) -> AppResult<()> {
    let owner = sqlx::query_scalar::<_, Option<u64>>("SELECT IS_USED_LOCK(?)")
        .bind(&fence.lock_name)
        .fetch_one(&mut **tx)
        .await?;
    if owner != Some(fence.connection_id) {
        return Err(stale_synthetic_provenance_conflict());
    }
    let valid=sqlx::query_scalar::<_,u64>(
        "SELECT pair_id FROM market_pair_generation_runs WHERE pair_id=? AND generation=? AND active_source=? AND lease_owner=? AND lease_expires_at>CURRENT_TIMESTAMP(6) AND ?<=CURRENT_TIMESTAMP(6) AND (last_tick_at IS NULL OR last_tick_at<=?) FOR UPDATE",
    ).bind(fence.pair_id).bind(fence.generation).bind(source).bind(&fence.owner)
        .bind(observed_at.naive_utc()).bind(observed_at.naive_utc()).fetch_optional(&mut **tx).await?;
    if valid.is_none() {
        return Err(stale_synthetic_provenance_conflict());
    }
    Ok(())
}

/// 人工来源先锁交易对再锁共同运行行，避免与后台的交易对→配置锁序反转。
/// 已被新版 worker 接管的交易对要求额外所有权证据；尚无运行行的历史调用保留原策略租约合同。
pub(super) async fn verify_manual_pair_fence(
    tx: &mut Transaction<'_, MySql>,
    provenance: &SyntheticTickerProvenance,
    observed_at: DateTime<Utc>,
) -> AppResult<()> {
    let pair_id = sqlx::query_scalar::<_, u64>("SELECT pair_id FROM market_strategies WHERE id=?")
        .bind(provenance.strategy_id())
        .fetch_one(&mut **tx)
        .await?;
    sqlx::query("SELECT id FROM trading_pairs WHERE id=? FOR UPDATE")
        .bind(pair_id)
        .fetch_one(&mut **tx)
        .await?;
    let paused = sqlx::query_scalar::<_, bool>(
        "SELECT all_market_paused FROM market_default_generators WHERE pair_id=? FOR UPDATE",
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(false);
    if paused {
        return Err(stale_synthetic_provenance_conflict());
    }
    let has_runtime = sqlx::query_scalar::<_, u64>(
        "SELECT pair_id FROM market_pair_generation_runs WHERE pair_id=?",
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .is_some();
    if let Some(fence) = &provenance.pair_fence {
        if fence.pair_id != pair_id {
            return Err(stale_synthetic_provenance_conflict());
        }
        verify_pair_fence(tx, fence, "strategy", observed_at).await?;
        let identity = sqlx::query_scalar::<_, u64>(
            "SELECT pair_id FROM market_pair_generation_runs WHERE pair_id=? AND strategy_id=? AND strategy_version=?",
        ).bind(pair_id).bind(provenance.strategy_id()).bind(provenance.active_version())
            .fetch_optional(&mut **tx).await?;
        if identity.is_none() {
            return Err(stale_synthetic_provenance_conflict());
        }
    } else if has_runtime {
        return Err(stale_synthetic_provenance_conflict());
    }
    Ok(())
}

async fn archive_default_ticker(
    pool: &Pool<MySql>,
    snapshot: &MarketTickerSnapshot,
    provenance: &DefaultTickerProvenance,
) -> AppResult<SyntheticTickerArchiveOutcome> {
    if snapshot.provider() != MarketDataProvider::Strategy
        || snapshot.last_price() <= &BigDecimal::from(0)
        || provenance.config_version == 0
    {
        return Err(AppError::Validation("invalid default market ticker".into()));
    }
    let fence = &provenance.fence;
    let observed_at = DateTime::from_timestamp_millis(snapshot.observed_at().timestamp_millis())
        .ok_or_else(|| AppError::Validation("invalid default ticker time".into()))?;
    let symbol = ValidatedMarketSymbol::from_raw(snapshot.symbol())
        .map_err(|e| AppError::Validation(e.to_string()))?
        .as_str()
        .to_owned();
    let source_version = format!(
        "default:{}:g{}:v{}",
        fence.pair_id, fence.generation, provenance.config_version
    );
    let key = hex::encode(sha2::Sha256::digest(format!(
        "default|{symbol}|{}|{}|{source_version}",
        observed_at.timestamp_millis(),
        snapshot.last_price().normalized()
    )));
    let expected = SyntheticTickerArchiveRow {
        event_key: key.clone(),
        symbol: symbol.clone(),
        price: snapshot.last_price().clone(),
        source: "default".into(),
        observed_at,
        generation: fence.generation,
        source_version: source_version.clone(),
        strategy_id: None,
        strategy_version: None,
    };
    let mut tx = pool.begin().await?;
    let pair = sqlx::query_as::<_, (String, String, String)>(
        "SELECT symbol,status,market_type FROM trading_pairs WHERE id=? FOR UPDATE",
    )
    .bind(fence.pair_id)
    .fetch_one(&mut *tx)
    .await?;
    if crate::modules::market::sanitize_symbol(&pair.0) != symbol
        || pair.1 != "active"
        || !matches!(pair.2.as_str(), "strategy" | "internal")
    {
        return Err(stale_synthetic_provenance_conflict());
    }
    let control=sqlx::query_as::<_,(bool,bool)>("SELECT enabled,all_market_paused FROM market_default_generators WHERE pair_id=? FOR UPDATE").bind(fence.pair_id).fetch_optional(&mut *tx).await?;
    if control != Some((true, false)) {
        return Err(stale_synthetic_provenance_conflict());
    }
    verify_pair_fence(&mut tx, fence, "default", observed_at).await?;
    let valid=sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM market_pair_generation_runs runtime INNER JOIN market_default_generator_versions versions ON versions.pair_id=runtime.pair_id AND versions.version=runtime.default_version WHERE runtime.pair_id=? AND runtime.default_version=? AND NOT EXISTS (SELECT 1 FROM market_strategies strategies WHERE strategies.pair_id=runtime.pair_id AND strategies.status='active' AND strategies.start_time<=CURRENT_TIMESTAMP(6) AND strategies.end_time>CURRENT_TIMESTAMP(6))")
        .bind(fence.pair_id).bind(provenance.config_version).fetch_one(&mut *tx).await?;
    if valid != 1 {
        return Err(stale_synthetic_provenance_conflict());
    }
    // 交易对行锁串行化该符号归档；同时间不同来源或载荷是冲突，不产生第二个候选结算价格。
    let latest=sqlx::query_as::<_,SyntheticTickerArchiveRow>("SELECT event_key,symbol,price,source,observed_at,generation,source_version,strategy_id,strategy_version FROM market_price_ticks WHERE symbol=? ORDER BY observed_at DESC,id DESC LIMIT 1 FOR UPDATE")
        .bind(&symbol).fetch_optional(&mut *tx).await?;
    if let Some(latest) = latest {
        if latest.observed_at > observed_at {
            return Err(stale_synthetic_provenance_conflict());
        }
        if latest.observed_at == observed_at {
            if synthetic_ticker_archive_matches(&latest, &expected) {
                tx.commit().await?;
                return Ok(SyntheticTickerArchiveOutcome::AlreadyArchived);
            }
            return Err(AppError::Conflict(
                "default ticker conflicts with archived event".into(),
            ));
        }
    }
    sqlx::query("INSERT INTO market_price_ticks(event_key,symbol,price,source,observed_at,generation,source_version) VALUES(?,?,?,'default',?,?,?)")
        .bind(key).bind(symbol).bind(snapshot.last_price()).bind(observed_at.naive_utc()).bind(fence.generation).bind(source_version).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(SyntheticTickerArchiveOutcome::Inserted)
}
