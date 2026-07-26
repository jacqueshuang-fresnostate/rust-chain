use super::*;

#[derive(Debug)]
pub(crate) struct AdminTradingPairListFilter {
    pub(crate) symbol: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminTradingPairInsert {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug)]
pub(crate) struct AdminTradingPairUpdate {
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyListFilter {
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyInsert {
    pub(crate) pair_id: u64,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: String,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyUpdate {
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
}

pub(crate) async fn list_admin_trading_pairs(
    pool: &Pool<MySql>,
    filter: AdminTradingPairListFilter,
) -> AppResult<Vec<AdminTradingPairResponse>> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE 1 = 1");
    if let Some(symbol) = filter.symbol {
        builder.push(" AND pairs.symbol = ");
        builder.push_bind(symbol);
    }
    if let Some(status) = filter.status {
        builder.push(" AND pairs.status = ");
        builder.push_bind(status);
    }
    if let Some(market_type) = filter.market_type {
        builder.push(" AND pairs.market_type = ");
        builder.push_bind(market_type);
    }
    builder.push(" ORDER BY pairs.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_admin_trading_pair(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminTradingPairInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, logo_url, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.base_asset_id)
    .bind(input.quote_asset_id)
    .bind(&input.symbol)
    .bind(&input.logo_url)
    .bind(input.price_precision)
    .bind(input.qty_precision)
    .bind(&input.min_order_value)
    .bind(&input.status)
    .bind(&input.market_type)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_trading_pair_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    input: AdminTradingPairUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE trading_pairs
           SET logo_url = ?, price_precision = ?, qty_precision = ?, min_order_value = ?, status = ?, market_type = ?
           WHERE id = ?"#,
    )
    .bind(&input.logo_url)
    .bind(input.price_precision)
    .bind(input.qty_precision)
    .bind(&input.min_order_value)
    .bind(&input.status)
    .bind(&input.market_type)
    .bind(pair_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_trading_pair_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE trading_pairs SET status = ? WHERE id = ?")
        .bind(status)
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM trading_pairs WHERE id = ? FOR UPDATE")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    load_admin_trading_pair_in_tx(tx, pair_id).await
}

pub(crate) async fn ensure_trading_pair_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM assets WHERE id = ? AND status = 'active' LIMIT 1 FOR UPDATE",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

pub(crate) async fn list_admin_market_strategies(
    pool: &Pool<MySql>,
    filter: AdminMarketStrategyListFilter,
) -> AppResult<Vec<AdminMarketStrategyResponse>> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE 1 = 1");
    if let Some(pair_id) = filter.pair_id {
        builder.push(" AND strategies.pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = filter.status {
        builder.push(" AND strategies.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY strategies.created_at DESC, strategies.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn ensure_market_strategy_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT market_type FROM trading_pairs WHERE id = ? AND status = 'active' FOR UPDATE",
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if !matches!(row.0.as_str(), "internal" | "strategy") {
        return Err(AppError::Validation(
            "market strategy can only be bound to internal or strategy pairs".to_owned(),
        ));
    }
    Ok(row.0)
}

pub(crate) async fn insert_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketStrategyInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.pair_id)
    .bind(&input.strategy_type)
    .bind(&input.start_price)
    .bind(&input.target_price)
    .bind(input.start_time)
    .bind(input.end_time)
    .bind(&input.volatility)
    .bind(&input.volume_min)
    .bind(&input.volume_max)
    .bind(&input.status)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn insert_market_strategy_run_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, run_status, current_price, last_generated_at, last_kline_open_time, recovery_status)
           VALUES (?, ?, ?, ?, ?, 'idle')"#,
    )
    .bind(strategy_id)
    .bind(run_status)
    .bind(current_price)
    .bind(start_time)
    .bind(start_time)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn insert_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    version: i32,
    effective_time: DateTime<Utc>,
    config_json: Value,
    seed: String,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_versions (strategy_id, version, effective_time, config_json, seed, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(version)
    .bind(effective_time)
    .bind(SqlxJson(config_json))
    .bind(seed)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    input: AdminMarketStrategyUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE market_strategies
           SET strategy_type = ?, start_price = ?, target_price = ?, start_time = ?, end_time = ?,
               volatility = ?, volume_min = ?, volume_max = ?
           WHERE id = ?"#,
    )
    .bind(&input.strategy_type)
    .bind(&input.start_price)
    .bind(&input.target_price)
    .bind(input.start_time)
    .bind(input.end_time)
    .bind(&input.volatility)
    .bind(&input.volume_min)
    .bind(&input.volume_max)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_market_strategy_run_checkpoint_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs
           SET run_status = ?, current_price = ?, last_generated_at = NULL,
               last_kline_open_time = ?, recovery_status = 'idle', error_message = NULL
           WHERE strategy_id = ?"#,
    )
    .bind(run_status)
    .bind(current_price)
    .bind(start_time)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    ensure_market_strategy_run_updated(result.rows_affected())
}

pub(crate) async fn next_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<i32> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM strategy_versions WHERE strategy_id = ?",
    )
    .bind(strategy_id)
    .fetch_one(&mut **tx)
    .await?)
}

pub(crate) async fn update_market_strategy_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE market_strategies SET status = ? WHERE id = ?")
        .bind(status)
        .bind(strategy_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn update_market_strategy_run_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE strategy_runs SET run_status = ?, recovery_status = 'idle', error_message = NULL WHERE strategy_id = ?",
    )
    .bind(run_status)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    ensure_market_strategy_run_updated(result.rows_affected())
}

pub(crate) async fn load_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_market_strategy_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    action: &str,
    payload_json: Value,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_events (strategy_id, event_type, payload_json)
           VALUES (?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(action)
    .bind(SqlxJson(payload_json))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn ensure_market_strategy_run_updated(rows_affected: u64) -> AppResult<()> {
    if rows_affected != 1 {
        return Err(AppError::Conflict(
            "market strategy run checkpoint is missing".to_owned(),
        ));
    }
    Ok(())
}

fn admin_market_strategy_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT strategies.id,
                  strategies.pair_id,
                  pairs.symbol,
                  pairs.market_type,
                  strategies.strategy_type,
                  strategies.start_price,
                  strategies.target_price,
                  strategies.start_time,
                  strategies.end_time,
                  strategies.volatility,
                  strategies.volume_min,
                  strategies.volume_max,
                  strategies.status,
                  runs.run_status,
                  runs.current_price,
                  runs.last_generated_at,
                  runs.last_kline_open_time,
                  runs.recovery_status,
                  strategies.created_at
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           LEFT JOIN strategy_runs runs ON runs.strategy_id = strategies.id"#,
    )
}

fn admin_trading_pair_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT pairs.id,
                  pairs.base_asset AS base_asset_id,
                  pairs.quote_asset AS quote_asset_id,
                  pairs.symbol,
                  pairs.logo_url,
                  base.symbol AS base_asset,
                  quote.symbol AS quote_asset,
                  pairs.price_precision,
                  pairs.qty_precision,
                  pairs.min_order_value,
                  pairs.status,
                  pairs.market_type,
                  pairs.created_at
           FROM trading_pairs pairs
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset"#,
    )
}

fn map_duplicate_trading_pair_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("trading pair already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}
