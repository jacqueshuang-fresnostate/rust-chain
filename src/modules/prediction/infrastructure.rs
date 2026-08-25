//! prediction 有限上下文的基础设施层。
//!
//! 负责外部依赖交互、数据库持久化查询、HTTP 调用和订单/市场结算数据组装。
//! 本文件覆盖竞猜预测的四条主链路：Polymarket 市场同步、用户报价、下单冻结资金，
//! 以及市场结算时的批量派奖与无效退款，另含后台配置读写与分页查询。
//! 资金链路只有三个写钱包的入口，分别对应下单、结算与退款，
//! 三者都必须在调用方开启的事务内执行，自身不提交也不回滚。
//! 全局锁序固定为报价行、市场行、订单行、钱包行，由粗到细单向下探；
//! 结算路径按订单主键升序批量加锁，保证并发结算同一市场时不会交叉等待。
//! 幂等分两层：报价靠 `consumed_at` 一次性消费，订单靠用户加幂等键的唯一约束，
//! 市场结算则靠 settled 与 refunded 两个终态短路重放。
//! 金额一律用 `BigDecimal`，落库前按资产 `precision_scale` 截断或校验，本层不四舍五入。
//! 同步链路与资金链路的原子性要求不同：同步逐条市场独立提交，中途失败保留已提交部分，
//! 因此同步失败只影响本轮统计，不会让市场数据处于半更新的不可用状态。
//! 本层不发布任何领域事件，也不主动重试上游，重试交由调度器按同步日志判断。

use super::{
    presentation::{
        CreatePredictionOrderRequest, CreatePredictionQuoteRequest, PredictionMarketResponse,
        PredictionOrderResponse, PredictionQuoteResponse, PredictionSyncResponse,
    },
    repository::{
        PredictionAdminAuditEntry, PredictionAssetConfigRow, PredictionAssetConfigUpdate,
        PredictionAssetMetaRow, PredictionOrderSettlementRow, PredictionQuoteLockRow,
        PredictionSettingsRow, PredictionSettingsUpdate, PredictionStakeAssetRow,
        PredictionSyncLogRow, PredictionWalletRow,
    },
    service,
};
use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_PREDICTION,
        },
        wallet::truncate_amount_to_asset_precision,
    },
    state::AppState,
};
use axum::http::StatusCode;
use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use reqwest::Url;
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::{collections::HashSet, time::Duration};
use uuid::Uuid;

// 预测模块通用 SQL 片段，供管理端资产配置列表复用。
pub(crate) const ADMIN_ASSET_CONFIGS_SQL: &str = r#"SELECT assets.id AS asset_id, assets.symbol AS asset_symbol,
                  COALESCE(configs.enabled, FALSE) AS enabled,
                  COALESCE(configs.max_payout_amount, 0) AS max_payout_amount,
                  COALESCE(configs.revision, CAST(0 AS UNSIGNED)) AS revision,
                  COALESCE(configs.created_at, assets.created_at) AS created_at,
                  COALESCE(configs.updated_at, assets.created_at) AS updated_at
           FROM assets
           LEFT JOIN prediction_asset_configs configs ON configs.asset_id = assets.id
           WHERE assets.status = 'active'"#;

const ADMIN_ASSET_CONFIGS_COUNT_SQL: &str = r#"SELECT COUNT(*)
           FROM assets
           LEFT JOIN prediction_asset_configs configs ON configs.asset_id = assets.id
           WHERE assets.status = 'active'"#;

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
/// 行查询与 COUNT 查询复用同一筛选构建器；任一失败整体返回，避免列表与总数口径分裂。
/// 调用方须把筛选条件都追加完毕再传入，本函数只负责补排序与分页两段尾巴。
/// 排序子句以原样字符串拼接，必须由调用方以字面量提供，不得来自请求参数；
/// 条数与偏移则走占位符绑定，因此这两个值可以安全地来自外部输入。
/// 两条查询分两次往返且不在同一事务，期间的并发写入会让总数与当前页轻微不一致，
/// 这对后台列表可接受，不应据此做资金判定。
pub(crate) async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

type SyncCounts = service::SyncCounts;
type EffectiveMarketConfig = service::EffectiveMarketConfig;

/// 以数据库时钟在事务内锁定市场并写入预测报价快照；全局设置的前置读取不加锁。
/// 报价固化本金、概率、费率、赔付上限和过期时间，不冻结钱包或写资金流水；插入失败不返回可用报价编号。
/// 校验依次为下注方向合法、本金为正、市场同时处于可见与未结算、资产启用且金额符合其精度，
/// 再确认该资产落在生效配置的允许范围内，任一不满足都在写库之前返回校验错误。
/// 接受价格按下注方向取市场当前的 yes 或 no 概率，并再次要求其严格位于零与一之间。
/// 份额由本金除以接受价格得出后按资产精度截断，理论赔付直接等于份额，
/// 手续费按本金乘生效费率同样截断；两处都向下截断，因此平台侧不会因舍入而多付。
/// 理论赔付超过生效赔付上限时直接拒绝报价，而不是先出价再在结算时封顶，
/// 使用户在下单前就知道该笔投注不被接受。
/// 报价编号取 UUID v7 加 pq 前缀，天然按时间递增；有效期取设置中的秒数并至少为 1 秒。
/// 市场行从校验到 quote 插入一直持有排他锁，因此概率、状态、end_at、
/// last_synced_at 与 market_version 必然来自同一快照；下单事务会再锁市场复核。
pub(crate) async fn create_quote_in_db(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreatePredictionQuoteRequest,
) -> AppResult<PredictionQuoteResponse> {
    let outcome = service::normalize_binary_outcome(&request.outcome)?;
    service::ensure_positive_amount(&request.stake_amount, "stake_amount")?;
    let settings = load_settings(pool).await?;
    ensure_prediction_asset_enabled(pool, request.asset_id).await?;
    let mut tx = pool.begin().await?;
    let market = lock_market(&mut tx, request.market_id).await?;
    let database_now = database_now_in_tx(&mut tx).await?;
    service::validate_market_trading_window(
        &market.display_status,
        &market.settlement_status,
        market.end_at,
        market.last_synced_at,
        database_now,
        service::market_sync_max_age_seconds(settings.sync_interval_seconds),
    )?;
    let market_last_synced_at = market.last_synced_at.ok_or_else(|| {
        AppError::Validation("prediction market has no synchronized snapshot".to_owned())
    })?;
    let asset = load_active_asset_in_tx(&mut tx, request.asset_id).await?;
    service::ensure_amount_precision(&request.stake_amount, asset.precision_scale, "stake_amount")?;
    let effective = effective_market_config(&settings, &market);
    if !effective.allowed_asset_ids.contains(&request.asset_id) {
        return Err(AppError::Validation(
            "asset is not allowed for this prediction market".to_owned(),
        ));
    }

    let accepted_price = if outcome == service::OUTCOME_YES {
        market.yes_price.clone()
    } else {
        market.no_price.clone()
    };
    service::ensure_probability_price(&accepted_price)?;
    let raw_shares = request.stake_amount.clone() / accepted_price.clone();
    let shares = truncate_amount_to_asset_precision(&raw_shares, asset.precision_scale);
    let theoretical_payout = shares.clone();
    let fee_amount = truncate_amount_to_asset_precision(
        &(request.stake_amount.clone() * effective.fee_rate.clone()),
        asset.precision_scale,
    );
    let effective_payout_cap =
        effective_payout_cap_in_tx(&mut tx, request.asset_id, &effective.payout_cap_overrides)
            .await?;
    if effective_payout_cap > 0 && theoretical_payout > effective_payout_cap {
        return Err(AppError::Validation(
            "prediction quote exceeds configured payout cap".to_owned(),
        ));
    }
    let quote_id = format!("pq_{}", Uuid::now_v7().simple());
    let ttl_seconds = i64::from(settings.quote_ttl_seconds.max(1));
    let expires_at = database_now
        .checked_add_signed(TimeDelta::seconds(ttl_seconds))
        .ok_or_else(|| AppError::Validation("quote expiry is outside valid range".to_owned()))?;

    sqlx::query(
        r#"INSERT INTO prediction_quotes
           (quote_id, user_id, market_id, outcome, asset_id, stake_amount, fee_amount,
            accepted_price, shares, theoretical_payout, effective_payout_cap,
            market_version, market_last_synced_at, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&quote_id)
    .bind(user_id)
    .bind(request.market_id)
    .bind(&outcome)
    .bind(request.asset_id)
    .bind(&request.stake_amount)
    .bind(&fee_amount)
    .bind(&accepted_price)
    .bind(&shares)
    .bind(&theoretical_payout)
    .bind(&effective_payout_cap)
    .bind(market.market_version)
    .bind(market_last_synced_at)
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(PredictionQuoteResponse {
        quote_id,
        market_id: request.market_id,
        outcome,
        asset_id: request.asset_id,
        asset_symbol: asset.symbol,
        stake_amount: request.stake_amount,
        fee_amount,
        accepted_price,
        shares,
        theoretical_payout,
        effective_payout_cap,
        market_version: market.market_version,
        market_last_synced_at,
        expires_at,
    })
}

/// 在调用方已锁定设置单例的事务内执行条件更新，并由数据库原子递增 revision。
/// `expected_revision` 同时进入 WHERE 条件；影响零行表示版本已变化，调用方必须返回 409 且不得写审计。
/// 本函数只写配置行，不提交事务、不回读也不写审计，业务变更与审计的原子性由应用层统一编排。
pub(crate) async fn update_admin_settings_if_revision_in_tx(
    tx: &mut Transaction<'_, MySql>,
    update: &PredictionSettingsUpdate,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"UPDATE prediction_settings
           SET sync_enabled = ?, sync_interval_seconds = ?, sync_tags_json = ?,
               allowed_asset_ids_json = ?, default_fee_rate = ?,
               default_settlement_mode = ?, default_invalid_refund_policy = ?,
               quote_ttl_seconds = ?, revision = revision + 1
           WHERE id = 1 AND revision = ?"#,
    )
    .bind(update.sync_enabled)
    .bind(update.sync_interval_seconds)
    .bind(SqlxJson(json!(&update.sync_tags)))
    .bind(SqlxJson(json!(&update.allowed_asset_ids)))
    .bind(&update.default_fee_rate)
    .bind(&update.default_settlement_mode)
    .bind(&update.default_invalid_refund_policy)
    .bind(update.quote_ttl_seconds)
    .bind(update.expected_revision)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// 按稳定顺序返回全部启用资产的预测配置与总数，数据库失败不使用默认配置替代。
/// 以资产表为主表左连配置表，因此尚未配置过的资产也会出现在列表里，
/// 其启用标记与赔付上限回退为假与零，时间字段回退到资产自身的创建时间。
/// 这样后台能看到「所有可配置资产」而不只是「已配置资产」，避免新资产因不在列表而无法开启。
/// 排序先按符号升序再补资产主键，因为符号可能重复，只按符号排序会让分页在边界处漏行或重行。
pub(crate) async fn list_admin_asset_configs(
    pool: &Pool<MySql>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<PredictionAssetConfigRow>, i64)> {
    // 列出所有激活资产的预测配置，缺失配置的资产会回退到资产创建时间作为时间字段。
    // symbol 可能重名，排序补主键保证分页稳定。
    fetch_admin_page(
        pool,
        QueryBuilder::<MySql>::new(ADMIN_ASSET_CONFIGS_SQL),
        QueryBuilder::<MySql>::new(ADMIN_ASSET_CONFIGS_COUNT_SQL),
        " ORDER BY assets.symbol ASC, assets.id ASC",
        limit,
        offset,
    )
    .await
}

/// 返回当前可用于竞猜下注的资产清单，供用户侧渲染下注币种选择与展示各自赔付上限。
/// 与后台配置列表相反，这里以配置表为主表做内连接，并同时要求配置已启用且资产本身处于启用状态，
/// 因此只要任一侧被关停，该资产就立刻从用户可选项中消失。
/// 结果按资产符号升序，不分页也不带总数，因为可下注资产数量有限且需要一次性全量展示。
/// 本查询只用于展示与前置提示，真正的下注资格仍由下单事务内的校验决定。
pub(crate) async fn list_stake_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<PredictionStakeAssetRow>> {
    // 列出可下注资产，用于前端用户配置下拉与展示。
    let rows = sqlx::query_as::<_, PredictionStakeAssetRow>(
        r#"SELECT configs.asset_id, assets.symbol AS asset_symbol, configs.max_payout_amount
           FROM prediction_asset_configs configs
           INNER JOIN assets ON assets.id = configs.asset_id
           WHERE configs.enabled = TRUE AND assets.status = 'active'
           ORDER BY assets.symbol ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 消费后端报价创建竞猜订单；报价必须属于用户、未过期未消费，市场开放且金额符合资产精度。
/// 新请求在单事务中依次锁报价和市场、插入订单占用幂等键，再消费报价并锁钱包完成冻结、扣费和佣金记录。
/// 可用余额减少本金与手续费、冻结余额增加本金，订单、报价、钱包及全部流水必须原子提交。
/// 同用户同键命中时直接返回原订单且 `changed=false`，不比较本次 `quote_id`；并发重复键回滚后重读，提交后仅加载响应，无外部副作用。
/// 幂等键与报价编号先经必填与长度校验，分别限长 128 与 64，超限在开事务前即拒绝。
/// 事务外先做一次无锁幂等回读作为快路径，命中即返回既有订单，完全不接触事务与钱包；
/// 回读到的订单状态为空视为脏数据，返回冲突而不是把它当作可重放的成功结果。
/// 事务内锁序严格为报价行、市场行、订单幂等键、钱包行，由粗到细单向下探，
/// 与结算路径的市场在前、钱包在后方向一致，因此下单与结算并发时不会形成等待环。
/// 报价三项前置缺一不可：归属用户必须相同否则返回 `Forbidden`，`consumed_at` 非空说明已被用掉，
/// 过期时间已过则报价失效；前者是越权，后两者是冲突与校验错误，三种语义刻意分开。
/// 订单先插入再回填订单号，订单号由主键生成因此必须等自增值确定；
/// 报价随后被置上 `consumed_at`，与订单插入同处一个事务，保证一份报价至多换一张订单。
/// 插入撞唯一键说明另一并发请求已抢先占位，此时显式回滚再走无锁重读并以 `changed=false` 返回，
/// 回滚后重读而不是在原事务内重试，是为了避免持锁等待对方提交造成的长事务。
/// 资金动作全部委托给钱包开仓函数：可用余额扣本金与手续费，冻结余额加本金，
/// 金额取报价固化的快照而非重新计算，因此行情在报价后变动不影响本次成交口径。
/// 代理佣金以订单主键为来源标识在同一事务内登记，佣金失败同样回滚整笔下单。
/// 提交成功后仅按主键重读一次订单响应，不发布任何事件，通知由上层自行编排。
pub(crate) async fn create_order_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreatePredictionOrderRequest,
) -> AppResult<(PredictionOrderResponse, bool)> {
    let quote_id = service::required_text(request.quote_id, "quote_id", 64)?;
    let idempotency_key = service::required_text(request.idempotency_key, "idempotency_key", 128)?;
    if let Some(existing) = load_order_by_idempotency(pool, user_id, &idempotency_key).await? {
        if existing.status.is_empty() {
            return Err(AppError::Conflict(
                "prediction order idempotency key is invalid".to_owned(),
            ));
        }
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let quote = lock_quote(&mut tx, &quote_id).await?;
    if quote.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    if quote.consumed_at.is_some() {
        return Err(AppError::Conflict(
            "prediction quote was already used".to_owned(),
        ));
    }
    let market = lock_market(&mut tx, quote.market_id).await?;
    let settings = load_settings_in_tx(&mut tx).await?;
    // 时间必须在取得市场行锁后读取；若锁等待跨过 end_at，旧的锁前时间不得放行订单。
    let database_now = database_now_in_tx(&mut tx).await?;
    if database_now >= quote.expires_at {
        return Err(AppError::Validation("prediction quote expired".to_owned()));
    }
    service::validate_market_trading_window(
        &market.display_status,
        &market.settlement_status,
        market.end_at,
        market.last_synced_at,
        database_now,
        service::market_sync_max_age_seconds(settings.sync_interval_seconds),
    )?;
    if market.market_version != quote.market_version
        || market.last_synced_at != Some(quote.market_last_synced_at)
    {
        return Err(AppError::Conflict(
            "prediction market changed after quote creation".to_owned(),
        ));
    }
    let asset = load_active_asset_in_tx(&mut tx, quote.asset_id).await?;
    service::ensure_amount_precision(&quote.stake_amount, asset.precision_scale, "stake_amount")?;
    service::ensure_amount_precision(&quote.fee_amount, asset.precision_scale, "fee_amount")?;

    let insert = sqlx::query(
        r#"INSERT INTO prediction_orders
           (user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
            stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
            effective_payout_cap, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open')"#,
    )
    .bind(user_id)
    .bind(quote.market_id)
    .bind(&quote.quote_id)
    .bind(&idempotency_key)
    .bind(&quote.outcome)
    .bind(quote.asset_id)
    .bind(&quote.stake_amount)
    .bind(&quote.fee_amount)
    .bind(&quote.accepted_price)
    .bind(&quote.shares)
    .bind(&quote.theoretical_payout)
    .bind(&quote.effective_payout_cap)
    .execute(&mut *tx)
    .await;

    let order_id = match insert {
        Ok(result) => result.last_insert_id(),
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            let order = load_order_by_idempotency(pool, user_id, &idempotency_key)
                .await?
                .ok_or_else(|| {
                    AppError::Conflict("prediction idempotency key is being committed".to_owned())
                })?;
            return Ok((order, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };
    let order_no = service::prediction_order_no(order_id);
    sqlx::query("UPDATE prediction_orders SET order_no = ? WHERE id = ?")
        .bind(&order_no)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    let consumed = sqlx::query(
        r#"UPDATE prediction_quotes
           SET consumed_at = ?
           WHERE quote_id = ? AND consumed_at IS NULL AND expires_at > ?"#,
    )
    .bind(database_now.naive_utc())
    .bind(&quote.quote_id)
    .bind(database_now.naive_utc())
    .execute(&mut *tx)
    .await?;
    if consumed.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "prediction quote could not be consumed exactly once".to_owned(),
        ));
    }
    apply_wallet_prediction_open(
        &mut tx,
        user_id,
        quote.asset_id,
        &quote.stake_amount,
        &quote.fee_amount,
        order_id,
    )
    .await?;
    let commission_source_id = order_id.to_string();
    insert_agent_business_commission_in_tx(
        &mut tx,
        AgentBusinessCommissionWrite {
            user_id,
            product_type: AGENT_COMMISSION_PRODUCT_PREDICTION,
            source_type: "prediction_order",
            source_id: &commission_source_id,
            source_amount: &quote.stake_amount,
            payout_asset_id: quote.asset_id,
        },
    )
    .await?;
    tx.commit().await?;
    Ok((load_order_response(pool, order_id).await?, true))
}

/// 识别数据库唯一约束冲突，供预测订单幂等重放与资产配置首次创建竞争分支使用。
///
/// 本判断仅解释 SQLx 适配器错误；调用方随后按用户和幂等键读取原订单，不比较重放的 `quote_id`。
/// 只认唯一键冲突这一种数据库错误，外键冲突、超时与连接中断都不在此列，
/// 它们会沿正常错误路径回滚，绝不会被误当成可安全重放的幂等命中。
/// 判定不区分具体唯一索引，因此调用点必须确保碰撞字段可被安全解释为同一业务对象的并发写入。
fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.is_unique_violation())
}

/// 批量结算指定竞猜市场；结果与具体无效退款策略须已由应用层规范化，manual 策略不得直接执行。
/// 事务先锁市场，再按订单 ID 顺序锁全部 open 订单；每单随后锁钱包，派奖或退款并更新订单终态。
/// 本金解冻、可用余额、派奖/退款流水、订单结果及市场结算状态必须在同一事务内保持一致。
/// 已 settled/refunded 的市场重放返回零处理且不再动账，不比较传入结果或退款策略；提交后只重读市场响应，不发布外部事件。
/// 终态短路发生在锁定市场之后，因此判重本身也受行锁保护，并发重复结算只有一方真正动账。
/// 短路分支仍执行提交而非回滚，是为了释放已持有的市场行锁；该事务未做任何写入，提交为空操作。
/// 退款策略的取值顺序为：结果是 invalid 时优先用调用方显式传入的策略，未传则回退到全局默认；
/// 结果非 invalid 时该值不参与资金计算，只是被一并读出。
/// 若最终解析出的策略仍是 manual，说明需要人工逐笔处理，此处直接返回校验错误并回滚，
/// 绝不代替运营决定退多少，避免把不确定的退款口径写成既成事实。
/// 订单以主键升序批量加锁，配合市场行锁使任意两个并发结算的加锁顺序完全一致，不会交叉等待。
/// 逐单处理时再判一次状态是否仍为 open，跳过已被其他路径改动的订单。
/// 无效市场走退款：本金全额退回，手续费仅在策略为连费退还时一并退回，否则记为零。
/// 有效市场走派奖：方向命中的订单按理论赔付并受赔付上限封顶，未命中的订单派奖为零但仍需解冻本金。
/// 每单资金动作与订单终态更新成对出现，订单不会出现「已改状态但没动钱」的中间态。
/// 市场最终按结果落到 refunded 或 settled，无效场景额外记录实际使用的退款策略以备审计。
/// 返回值依次是重读的市场响应、本次真正处理的订单数，以及是否为首次结算而非重放。
pub(crate) async fn settle_market_in_tx(
    pool: &Pool<MySql>,
    market_id: u64,
    result: String,
    requested_refund_policy: Option<String>,
) -> AppResult<(PredictionMarketResponse, u32, bool)> {
    let mut tx = pool.begin().await?;
    let market = lock_market(&mut tx, market_id).await?;
    if market.settlement_status == service::SETTLEMENT_SETTLED
        || market.settlement_status == service::SETTLEMENT_REFUNDED
    {
        tx.commit().await?;
        return Ok((load_market_response(pool, market_id).await?, 0, false));
    }
    let settings = load_settings_in_tx(&mut tx).await?;
    let refund_policy = if result == service::OUTCOME_INVALID {
        match requested_refund_policy {
            Some(policy) => policy,
            None => settings.default_invalid_refund_policy.clone(),
        }
    } else {
        settings.default_invalid_refund_policy.clone()
    };
    if result == service::OUTCOME_INVALID && refund_policy == service::REFUND_MANUAL {
        return Err(AppError::Validation(
            "manual invalid refund policy requires an explicit concrete refund policy".to_owned(),
        ));
    }
    let orders = sqlx::query_as::<_, PredictionOrderSettlementRow>(
        r#"SELECT id, user_id, asset_id, outcome, stake_amount, fee_amount,
                  theoretical_payout, effective_payout_cap, status
           FROM prediction_orders
           WHERE market_id = ? AND status = 'open'
           ORDER BY id ASC
           FOR UPDATE"#,
    )
    .bind(market_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut settled_orders = 0u32;
    for order in orders {
        if order.status != service::ORDER_STATUS_OPEN {
            continue;
        }
        if result == service::OUTCOME_INVALID {
            let fee_refund_amount = if refund_policy == service::REFUND_STAKE_AND_FEE {
                order.fee_amount.clone()
            } else {
                BigDecimal::from(0)
            };
            apply_wallet_prediction_refund(
                &mut tx,
                order.user_id,
                order.asset_id,
                &order.stake_amount,
                &fee_refund_amount,
                order.id,
            )
            .await?;
            sqlx::query(
                r#"UPDATE prediction_orders
                   SET status = 'refunded', result = ?, refund_amount = ?,
                       fee_refund_amount = ?, invalid_refund_policy_used = ?,
                       settled_at = CURRENT_TIMESTAMP(6)
                   WHERE id = ?"#,
            )
            .bind(&result)
            .bind(&order.stake_amount)
            .bind(&fee_refund_amount)
            .bind(&refund_policy)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;
        } else {
            let payout_amount = if order.outcome == result {
                service::capped_payout(&order.theoretical_payout, &order.effective_payout_cap)
            } else {
                BigDecimal::from(0)
            };
            apply_wallet_prediction_settlement(
                &mut tx,
                order.user_id,
                order.asset_id,
                &order.stake_amount,
                &payout_amount,
                order.id,
                order.outcome == result,
            )
            .await?;
            sqlx::query(
                r#"UPDATE prediction_orders
                   SET status = 'settled', result = ?, payout_amount = ?,
                       settled_at = CURRENT_TIMESTAMP(6)
                   WHERE id = ?"#,
            )
            .bind(&result)
            .bind(&payout_amount)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;
        }
        settled_orders += 1;
    }

    let settlement_status = if result == service::OUTCOME_INVALID {
        service::SETTLEMENT_REFUNDED
    } else {
        service::SETTLEMENT_SETTLED
    };
    let invalid_policy_used = if result == service::OUTCOME_INVALID {
        Some(refund_policy.clone())
    } else {
        None
    };
    sqlx::query(
        r#"UPDATE prediction_markets
           SET local_resolution = ?, settlement_status = ?,
               invalid_refund_policy_used = ?
           WHERE id = ?"#,
    )
    .bind(&result)
    .bind(settlement_status)
    .bind(invalid_policy_used)
    .bind(market_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        load_market_response(pool, market_id).await?,
        settled_orders,
        true,
    ))
}

/// 创建 running 日志并更新全局同步状态，再以 Polymarket 响应同步市场，最后记录 success 或 failed。
/// 市场更新并非整轮单事务：中途失败会保留此前已提交的市场更新或自动结算，并把本轮日志与设置标为失败。
/// 本函数只负责一轮同步的可观测性外壳，真正的拉取与写库全部委托给内部实现。
/// 开始时先插入一条 running 日志并把设置上的同步状态同步改为 running 且清空上次错误，
/// 因此运维既能从日志表看到每一轮，也能从设置行看到当前是否有同步在跑。
/// 成功时回填导入与更新计数、结束时间，并把设置的最近成功时间一并推进；
/// 失败时把错误文本压缩到单行且截断后写入日志与设置，再原样上抛原始错误。
/// 无论成败都不会删除或回滚已写入的市场数据，同步是逐条提交的最终一致过程。
/// 日志与设置的四次写入各自独立提交，若其中某次写入本身失败，
/// 会掩盖原始业务错误并向上返回该写入错误，此时日志可能停留在 running 状态。
/// 本函数不加锁也不判重，并发触发同一轮同步会产生两条日志并重复拉取，
/// 由调用方按同步间隔自行节流。
pub(crate) async fn sync_polymarket_markets(
    pool: &Pool<MySql>,
    trigger_type: &str,
) -> AppResult<PredictionSyncResponse> {
    let started_at = Utc::now();
    let log_id = sqlx::query(
        r#"INSERT INTO prediction_sync_logs (trigger_type, status, started_at)
           VALUES (?, 'running', ?)"#,
    )
    .bind(trigger_type)
    .bind(started_at)
    .execute(pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"UPDATE prediction_settings
           SET last_sync_status = 'running',
               last_sync_error = NULL,
               last_sync_started_at = ?
           WHERE id = 1"#,
    )
    .bind(started_at)
    .execute(pool)
    .await?;

    let result = sync_polymarket_markets_inner(pool).await;
    let finished_at = Utc::now();
    match result {
        Ok(counts) => {
            sqlx::query(
                r#"UPDATE prediction_sync_logs
                   SET status = 'success', imported_count = ?, updated_count = ?,
                       finished_at = ?
                   WHERE id = ?"#,
            )
            .bind(counts.imported_count)
            .bind(counts.updated_count)
            .bind(finished_at)
            .bind(log_id)
            .execute(pool)
            .await?;
            sqlx::query(
                r#"UPDATE prediction_settings
                   SET last_sync_status = 'success', last_sync_error = NULL,
                       last_sync_finished_at = ?, last_successful_sync_at = ?,
                       last_sync_imported_count = ?, last_sync_updated_count = ?
                   WHERE id = 1"#,
            )
            .bind(finished_at)
            .bind(finished_at)
            .bind(counts.imported_count)
            .bind(counts.updated_count)
            .execute(pool)
            .await?;
            Ok(PredictionSyncResponse {
                imported_count: counts.imported_count,
                updated_count: counts.updated_count,
                status: "success".to_owned(),
                error_message: None,
            })
        }
        Err(error) => {
            let message = service::compact_error_message(&error.to_string());
            sqlx::query(
                r#"UPDATE prediction_sync_logs
                   SET status = 'failed', error_message = ?, finished_at = ?
                   WHERE id = ?"#,
            )
            .bind(&message)
            .bind(finished_at)
            .bind(log_id)
            .execute(pool)
            .await?;
            sqlx::query(
                r#"UPDATE prediction_settings
                   SET last_sync_status = 'failed', last_sync_error = ?,
                       last_sync_finished_at = ?
                   WHERE id = 1"#,
            )
            .bind(&message)
            .bind(finished_at)
            .execute(pool)
            .await?;
            Err(error)
        }
    }
}

/// 拉取并解析 Polymarket 市场，按外部市场编号去重后逐条 upsert 当前快照。
/// 每条 upsert 独立提交；明确终局随后按配置自动结算或标记待确认，后续条目失败不会回滚前序写入。
/// 解析失败的市场被静默跳过而不中断整轮，因为上游偶发的残缺条目不应拖垮全部同步；
/// 去重按外部市场编号取首次出现者，同一编号在多个标签下重复返回时只处理一次。
/// upsert 以来源加外部市场编号为唯一键，命中时覆盖标题、概率、成交量、上游状态等可变字段，
/// 并刷新同步时间；本地的结算状态与人工覆盖配置不在更新列内，因此后台调整不会被同步冲掉。
/// 值得注意的是展示状态与上游状态绑定同一个值，上游关闭市场会连带把它从用户侧隐藏，
/// 这意味着后台手工设置的展示状态会在下一轮同步被上游状态覆盖。
/// 计数依据自增主键是否产生：新插入计入导入数，命中既有行计入更新数。
/// 每条市场写完立即协调其终局状态，可能触发自动结算并真实动账，
/// 因此本函数虽名为同步却存在资金副作用，且这些副作用逐条提交无法整体回滚。
pub(crate) async fn sync_polymarket_markets_inner(pool: &Pool<MySql>) -> AppResult<SyncCounts> {
    let settings = load_settings(pool).await?;
    let tags = service::json_string_array(&settings.sync_tags_json);
    let remote_markets = fetch_polymarket_markets(&tags).await?;
    let mut seen_market_ids = HashSet::new();
    let parsed_markets = remote_markets
        .iter()
        .filter_map(|value| service::parse_polymarket_market(value).ok())
        .filter(|market| seen_market_ids.insert(market.external_market_id.clone()))
        .collect::<Vec<_>>();
    let mut counts = SyncCounts::default();
    for market in parsed_markets {
        let result = sqlx::query(
            r#"INSERT INTO prediction_markets
               (source, external_event_id, external_market_id, slug, title, description,
                image_url, category, tags_json, outcome_yes_label, outcome_no_label,
                yes_price, no_price, volume, liquidity, end_at, source_status,
                display_status, external_resolution, settlement_status, sync_payload_json,
                last_synced_at)
               VALUES ('polymarket', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open', ?, CURRENT_TIMESTAMP(6))
               ON DUPLICATE KEY UPDATE
                   external_event_id = VALUES(external_event_id),
                   slug = VALUES(slug),
                   title = VALUES(title),
                   description = VALUES(description),
                   image_url = VALUES(image_url),
                   category = VALUES(category),
                   tags_json = VALUES(tags_json),
                   outcome_yes_label = VALUES(outcome_yes_label),
                   outcome_no_label = VALUES(outcome_no_label),
                   yes_price = VALUES(yes_price),
                   no_price = VALUES(no_price),
                   volume = VALUES(volume),
                   liquidity = VALUES(liquidity),
                   end_at = VALUES(end_at),
                   source_status = VALUES(source_status),
                   display_status = IF(
                       VALUES(end_at) IS NOT NULL
                       AND VALUES(end_at) <= CURRENT_TIMESTAMP(6),
                       'hidden',
                       VALUES(display_status)
                   ),
                   external_resolution = VALUES(external_resolution),
                   sync_payload_json = VALUES(sync_payload_json),
                   last_synced_at = CURRENT_TIMESTAMP(6),
                   market_version = market_version + 1"#,
        )
        .bind(&market.external_event_id)
        .bind(&market.external_market_id)
        .bind(&market.slug)
        .bind(&market.title)
        .bind(&market.description)
        .bind(&market.image_url)
        .bind(&market.category)
        .bind(SqlxJson(market.tags_json))
        .bind(&market.outcome_yes_label)
        .bind(&market.outcome_no_label)
        .bind(&market.yes_price)
        .bind(&market.no_price)
        .bind(&market.volume)
        .bind(&market.liquidity)
        .bind(market.end_at)
        .bind(&market.source_status)
        .bind(&market.source_status)
        .bind(&market.external_resolution)
        .bind(SqlxJson(market.payload))
        .execute(pool)
        .await?;
        let is_insert = result.last_insert_id() > 0;
        if is_insert {
            counts.imported_count += 1;
        } else {
            counts.updated_count += 1;
        }
        reconcile_synced_resolution(
            pool,
            &settings,
            &market.external_market_id,
            &market.source_status,
            &market.external_resolution,
        )
        .await?;
    }
    Ok(counts)
}

/// 根据已保存的上游终局协调本地状态：自动模式调用完整结算事务，人工模式或手工退款策略只标记待确认。
/// 无明确结果的关闭市场也转为待确认；协调失败返回错误，但不会回滚调用前已经提交的市场快照。
/// 入口先短路三种已定局的情形：本地已有结算结果、市场已 settled、市场已 refunded，
/// 命中任一即直接返回，保证上游结果反复推送也不会二次动账。
/// 上游未给出终局结果时，只有在上游已标记关闭且本地仍为 open 的情况下才推进到待确认，
/// 目的是不让订单永远停在可下注状态；该更新在 `WHERE` 中重复带上原状态作为乐观并发守卫。
/// 有终局结果时，结算模式优先取市场自身的覆盖配置，缺省才回退到全局默认。
/// 只有模式为自动、且不属于「结果无效但全局退款策略为人工」这一组合时，才真正调用结算事务派奖；
/// 该组合被排除是因为人工退款策略无法在无人参与的情况下确定退款口径。
/// 其余情形一律只把市场推进到待确认，等待后台显式确认后再动资金。
/// 自动结算分支会在本函数内部真实扣减与派发资金，其原子性由被调用的结算事务保证；
/// 本函数自身不开事务，若结算成功后本轮同步的后续步骤失败，已派奖的资金不会被撤回。
pub(crate) async fn reconcile_synced_resolution(
    pool: &Pool<MySql>,
    settings: &PredictionSettingsRow,
    external_market_id: &str,
    source_status: &str,
    external_resolution: &Option<String>,
) -> AppResult<()> {
    let market = load_market_by_source_external(pool, "polymarket", external_market_id).await?;
    if market.local_resolution.is_some()
        || market.settlement_status == service::SETTLEMENT_SETTLED
        || market.settlement_status == service::SETTLEMENT_REFUNDED
    {
        return Ok(());
    }

    let Some(result) = external_resolution.as_ref() else {
        // 上游明确关闭但还未给出结果时，停止继续对用户开放并交给后台确认，避免订单永久停在 open。
        if source_status == service::STATUS_HIDDEN
            && market.settlement_status == service::SETTLEMENT_OPEN
        {
            sqlx::query(
                "UPDATE prediction_markets SET settlement_status = ? WHERE id = ? AND settlement_status = ?",
            )
            .bind(service::SETTLEMENT_PENDING_CONFIRMATION)
            .bind(market.id)
            .bind(service::SETTLEMENT_OPEN)
            .execute(pool)
            .await?;
        }
        return Ok(());
    };

    let settlement_mode = market
        .settlement_mode_override
        .clone()
        .unwrap_or_else(|| settings.default_settlement_mode.clone());
    let invalid_requires_manual_policy = result == service::OUTCOME_INVALID
        && settings.default_invalid_refund_policy == service::REFUND_MANUAL;
    if settlement_mode == service::SETTLEMENT_MODE_AUTO && !invalid_requires_manual_policy {
        settle_market_in_tx(pool, market.id, result.clone(), None).await?;
        return Ok(());
    }

    if market.settlement_status == service::SETTLEMENT_OPEN {
        sqlx::query(
            "UPDATE prediction_markets SET settlement_status = ? WHERE id = ? AND settlement_status = ?",
        )
        .bind(service::SETTLEMENT_PENDING_CONFIRMATION)
        .bind(market.id)
        .bind(service::SETTLEMENT_OPEN)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// 从 Polymarket 接口拉取市场分页并保留上游标识、概率与结算字段作为同步权威输入。
/// 网络、状态码或载荷解析失败均返回同步错误，不以空列表覆盖已持久化市场。
/// 请求按标签与开闭状态做笛卡尔展开：每个标签都分别拉一次未关闭与已关闭市场，
/// 拉已关闭的那一轮才能拿到终局结果，否则结算永远等不到上游结论。
/// 标签列表为空时以单个空标签占位，退化为不带标签过滤的两次全量拉取。
/// 标签形态自动判别：纯数字按标签编号传参，其余按标签别名传参，兼容后台两种填写习惯。
/// 客户端超时固定 15 秒并带上可识别的 UA，超时按上游错误处理而非静默返回空。
/// 非 2xx 状态码会连同压缩后的响应体一并报错，便于从同步日志直接看出上游拒绝原因。
/// 所有轮次的结果被拉平后累加进同一个列表，去重交由调用方按外部市场编号完成。
/// 任意一轮失败即整体返回错误，已拉取的部分结果一并丢弃，
/// 这样不会用一份不完整的快照去覆盖数据库中已有的市场数据。
pub(crate) async fn fetch_polymarket_markets(tags: &[String]) -> AppResult<Vec<Value>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("rust-chain-prediction-sync/1.0")
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let tags_to_fetch = if tags.is_empty() {
        vec![String::new()]
    } else {
        tags.to_vec()
    };
    let mut values = Vec::new();
    for tag in tags_to_fetch {
        for closed in [false, true] {
            let mut params = vec![
                ("closed".to_owned(), closed.to_string()),
                ("limit".to_owned(), service::DEFAULT_SYNC_LIMIT.to_owned()),
            ];
            if !closed {
                params.push(("active".to_owned(), "true".to_owned()));
            }
            if !tag.is_empty() {
                if tag.chars().all(|ch| ch.is_ascii_digit()) {
                    params.push(("tag_id".to_owned(), tag.clone()));
                } else {
                    params.push(("tag_slug".to_owned(), tag.clone()));
                }
            }
            let url = Url::parse_with_params(service::POLYMARKET_GAMMA_EVENTS_URL, &params)
                .map_err(|error| AppError::Internal(error.to_string()))?;
            let response = client
                .get(url)
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
                .map_err(|error| upstream_sync_error(error.to_string()))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|error| upstream_sync_error(error.to_string()))?;
            if !status.is_success() {
                return Err(upstream_sync_error(format!(
                    "polymarket returned status {status}: {}",
                    service::compact_error_message(&body)
                )));
            }
            let payload: Value = serde_json::from_str(&body).map_err(|error| {
                upstream_sync_error(format!("polymarket returned invalid json: {error}"))
            })?;
            values.extend(service::extract_market_values(payload));
        }
    }
    Ok(values)
}

/// 构造预测市场读模型的基础查询，只给出 SELECT 与 FROM 两段，不执行任何数据库访问。
/// 列清单固定为市场读模型所需的全部字段，包括上游标识、双向概率、成交量与流动性、
/// 上游状态与展示状态、上游结果与本地结果、结算状态，以及后台的四项覆盖配置。
/// 上游结果与本地结果分列两个字段，前者是同步来的参考值，后者才是决定派奖的权威口径。
/// 表以 markets 为别名，调用方追加条件时必须带上该别名，避免与订单查询的多表连接产生列歧义。
/// 筛选、排序与分页全部由调用方以占位符追加，本函数不拼接任何来自外部输入的片段。
pub(crate) fn prediction_market_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT markets.id, markets.source, markets.external_event_id, markets.external_market_id,
                  markets.slug, markets.title, markets.description, markets.image_url,
                  markets.category, markets.tags_json, markets.outcome_yes_label,
                  markets.outcome_no_label, markets.yes_price, markets.no_price,
                  markets.volume, markets.liquidity, markets.end_at, markets.source_status,
                  markets.display_status, markets.external_resolution, markets.local_resolution,
                  markets.settlement_status, markets.settlement_mode_override,
                  markets.allowed_asset_ids_override_json, markets.payout_cap_overrides_json,
                  markets.fee_rate_override, markets.last_synced_at,
                  markets.market_version, markets.locally_closed_at,
                  markets.created_at, markets.updated_at
           FROM prediction_markets markets"#,
    )
}

/// 构造与市场行查询同源的计数语句，调用方必须追加与行查询完全一致的过滤条件。
/// 表与别名刻意与行查询保持一致，使同一段筛选代码能原样作用于两个构建器，
/// 否则分页总数会与实际可翻页的行数对不上。
/// 只统计不取列，因此不受行查询列清单变化影响，两者唯一的耦合点就是筛选条件。
pub(crate) fn prediction_market_count_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM prediction_markets markets"#,
    )
}

/// 构造预测订单计数基础语句，避免分页总数与行筛选条件漂移。
/// 三个内连接与行查询完全相同且必须保留：因为是内连接，
/// 用户、市场或资产任一缺失都会让订单不被计入，去掉连接会让总数大于实际可见行数。
/// 别名同样与行查询对齐，使按用户邮箱、市场标题或资产符号的筛选能直接复用。
pub(crate) fn prediction_order_count_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM prediction_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN prediction_markets markets ON markets.id = orders.market_id
           INNER JOIN assets ON assets.id = orders.asset_id"#,
    )
}

/// 构造预测订单读模型的基础查询，连带取出用户邮箱、市场标题与资产符号三个展示字段。
/// 三张关联表都用内连接，因此订单只有在其用户、市场与资产均存在时才可见。
/// 金额类字段既包含下单时固化的本金、手续费、接受价格、份额、理论赔付与赔付上限，
/// 也包含结算后回填的派奖额、退款额、手续费退款额与实际使用的无效退款策略。
/// 未结算订单的这些结算字段为空，调用方应据 `status` 判断而非据金额是否为零。
/// 本函数只拼语句不执行查询，也绝不修改任何订单状态。
pub(crate) fn prediction_order_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.order_no, orders.user_id, users.email AS user_email,
                  orders.market_id, markets.title AS market_title, orders.outcome,
                  orders.asset_id, assets.symbol AS asset_symbol, orders.stake_amount,
                  orders.fee_amount, orders.accepted_price, orders.shares,
                  orders.theoretical_payout, orders.effective_payout_cap,
                  orders.status, orders.result, orders.payout_amount, orders.refund_amount,
                  orders.fee_refund_amount, orders.invalid_refund_policy_used,
                  orders.settled_at, orders.created_at
           FROM prediction_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN prediction_markets markets ON markets.id = orders.market_id
           INNER JOIN assets ON assets.id = orders.asset_id"#,
    )
}

/// 读取主键为 1 的预测设置单例行，返回同步开关、标签、允许资产、默认费率与结算退款策略，
/// 以及报价有效期和最近一次同步的状态、错误与计数等可观测字段。
/// 设置行缺失视为部署未完成初始化，直接返回内部错误而不是构造一份内存默认值，
/// 因为默认费率与退款策略直接影响资金，凭空捏造会让线上按未经确认的口径动账。
/// 该读取不加锁，适用于报价与同步等只需近似一致的场景；
/// 结算等需要与写入同处一致快照的路径必须改用事务内的加锁版本。
pub(crate) async fn load_settings(pool: &Pool<MySql>) -> AppResult<PredictionSettingsRow> {
    sqlx::query_as::<_, PredictionSettingsRow>(
        r#"SELECT sync_enabled, sync_interval_seconds, sync_tags_json, allowed_asset_ids_json,
                  default_fee_rate, default_settlement_mode, default_invalid_refund_policy,
                  quote_ttl_seconds, revision, last_sync_status, last_sync_error,
                  last_sync_started_at, last_sync_finished_at, last_successful_sync_at,
                  last_sync_imported_count, last_sync_updated_count
           FROM prediction_settings
           WHERE id = 1"#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Internal("prediction settings are missing".to_owned()))
}

/// 在调用方事务内以 `FOR UPDATE` 读取预测设置单例行，列清单与无锁版本完全一致。
/// 行锁持有到事务结束，使结算全程看到同一份退款策略与默认配置，
/// 后台在结算途中保存新设置会被阻塞，不会出现前半批订单按旧策略、后半批按新策略退款。
/// 该锁排在市场行锁之后获取，结算路径必须保持这一顺序，否则与其他持设置锁的写入可能成环。
/// 设置行缺失同样返回内部错误并使整个事务回滚，绝不以默认值继续结算。
pub(crate) async fn load_settings_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<PredictionSettingsRow> {
    sqlx::query_as::<_, PredictionSettingsRow>(
        r#"SELECT sync_enabled, sync_interval_seconds, sync_tags_json, allowed_asset_ids_json,
                  default_fee_rate, default_settlement_mode, default_invalid_refund_policy,
                  quote_ttl_seconds, revision, last_sync_status, last_sync_error,
                  last_sync_started_at, last_sync_finished_at, last_successful_sync_at,
                  last_sync_imported_count, last_sync_updated_count
           FROM prediction_settings
           WHERE id = 1
           FOR UPDATE"#,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("prediction settings are missing".to_owned()))
}

/// 在预测资产配置事务中锁定权威资产行与现有配置，并返回可审计的完整前镜像。
/// 配置尚未创建时以 enabled=false、上限=0、revision=0 表示逻辑初始态；锁住 assets 行可串行化两个首次创建请求。
/// 资产不存在或已停用返回 NotFound，锁持续到调用方提交或回滚，不在本函数内产生任何写入。
pub(crate) async fn lock_admin_asset_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<PredictionAssetConfigRow> {
    sqlx::query_as::<_, PredictionAssetConfigRow>(
        r#"SELECT assets.id AS asset_id, assets.symbol AS asset_symbol,
                  COALESCE(configs.enabled, FALSE) AS enabled,
                  COALESCE(configs.max_payout_amount, 0) AS max_payout_amount,
                  COALESCE(configs.revision, CAST(0 AS UNSIGNED)) AS revision,
                  COALESCE(configs.created_at, assets.created_at) AS created_at,
                  COALESCE(configs.updated_at, assets.created_at) AS updated_at
           FROM assets
           LEFT JOIN prediction_asset_configs configs ON configs.asset_id = assets.id
           WHERE assets.id = ? AND assets.status = 'active'
           FOR UPDATE"#,
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在已持有资产行锁的事务中创建或条件更新预测资产配置，成功时 revision 恰好递增一次。
/// revision=0 只走 INSERT 并创建版本 1；既有配置使用 `WHERE revision = ?` 条件更新，影响零行由调用方映射为 409。
/// 唯一键竞争同样折叠为未更新，函数不提交事务、不写审计，也不会改写任何历史报价或订单快照。
pub(crate) async fn save_admin_asset_config_if_revision_in_tx(
    tx: &mut Transaction<'_, MySql>,
    update: &PredictionAssetConfigUpdate,
) -> AppResult<bool> {
    if update.expected_revision == 0 {
        let inserted = sqlx::query(
            r#"INSERT INTO prediction_asset_configs
               (asset_id, enabled, max_payout_amount, revision)
               VALUES (?, ?, ?, 1)"#,
        )
        .bind(update.asset_id)
        .bind(update.enabled)
        .bind(&update.max_payout_amount)
        .execute(&mut **tx)
        .await;
        return match inserted {
            Ok(result) => Ok(result.rows_affected() == 1),
            Err(error) if is_duplicate_key_error(&error) => Ok(false),
            Err(error) => Err(error.into()),
        };
    }

    let result = sqlx::query(
        r#"UPDATE prediction_asset_configs
           SET enabled = ?, max_payout_amount = ?, revision = revision + 1
           WHERE asset_id = ? AND revision = ?"#,
    )
    .bind(update.enabled)
    .bind(&update.max_payout_amount)
    .bind(update.asset_id)
    .bind(update.expected_revision)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// 在当前事务内回读刚保存的预测资产配置，响应 revision 与审计 after 必须共同取自这份已提交候选状态。
/// 查询要求配置行和资产行同时存在；缺失返回 NotFound 并促使调用方回滚，不会用左连接默认值伪造成功响应。
pub(crate) async fn load_admin_asset_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<PredictionAssetConfigRow> {
    sqlx::query_as::<_, PredictionAssetConfigRow>(
        r#"SELECT configs.asset_id, assets.symbol AS asset_symbol, configs.enabled,
                  configs.max_payout_amount, configs.revision,
                  configs.created_at, configs.updated_at
           FROM prediction_asset_configs configs
           INNER JOIN assets ON assets.id = configs.asset_id
           WHERE configs.asset_id = ?"#,
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在预测配置业务事务内追加管理员审计，并补充当前 HTTP 请求的来源 IP 与 request ID。
/// actor 只取已验证会话解析出的 admin_id；前后 JSON 已由应用层白名单化且包含 revision，本函数不接触原始请求体。
/// 审计插入失败会让配置写入一并回滚，HTTP 之外没有 task-local 上下文时两项传输元数据按 NULL 落库。
pub(crate) async fn insert_prediction_admin_audit_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    entry: PredictionAdminAuditEntry,
) -> AppResult<()> {
    let request_context = crate::infra::admin_request_context::current_admin_request_context();
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, request_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id.to_string())
    .bind(SqlxJson(entry.before_json))
    .bind(SqlxJson(entry.after_json))
    .bind(entry.reason)
    .bind(
        request_context
            .as_ref()
            .and_then(|context| context.source_ip.as_deref()),
    )
    .bind(
        request_context
            .as_ref()
            .map(|context| context.request_id.as_str()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 更新后台可控的市场展示状态与四项覆盖配置；上游标识与历史订单快照保持不变，也不触发结算。
/// 五个字段整体覆盖而非增量合并，其中四项覆盖配置传空即清除覆盖并回退到全局默认，
/// 因此调用方必须回填全部当前值，遗漏会被当作显式取消覆盖。
/// 允许资产与赔付上限覆盖以 JSON 落库，费率覆盖与结算模式覆盖则为标量列。
/// 展示状态只影响用户侧可见性，不冻结在途订单也不阻止到期结算；
/// 需要注意它会在下一轮同步被上游状态覆盖，所以不适合作为长期下架手段。
/// 返回是否命中到市场行，未命中说明市场编号不存在，由调用方决定报 `NotFound` 还是忽略。
/// UPDATE 会由 MySQL 取得市场行锁并递增 market_version；已生成但未消费的旧 quote
/// 会在下单事务的版本复核中失效，防止按旧费率或资产范围成交。
pub(crate) async fn update_admin_market(
    pool: &Pool<MySql>,
    market_id: u64,
    display_status: &str,
    settlement_mode_override: Option<&str>,
    allowed_asset_ids_override: Option<&[u64]>,
    payout_cap_overrides: Option<&Value>,
    fee_rate_override: Option<&BigDecimal>,
) -> AppResult<bool> {
    // 管理端更新市场展示和结算策略，返回是否成功命中到对应市场。
    let result = sqlx::query(
        r#"UPDATE prediction_markets
           SET display_status = ?, settlement_mode_override = ?,
               allowed_asset_ids_override_json = ?, payout_cap_overrides_json = ?,
               fee_rate_override = ?, market_version = market_version + 1
           WHERE id = ?"#,
    )
    .bind(display_status)
    .bind(settlement_mode_override)
    .bind(allowed_asset_ids_override.map(|ids| SqlxJson(json!(ids))))
    .bind(payout_cap_overrides.cloned().map(SqlxJson))
    .bind(fee_rate_override)
    .bind(market_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// 按日志 ID 倒序分页读取触发类型、状态、导入/更新计数、错误和起止时间，并返回全表总数。
/// 该查询不访问 Polymarket、不重试同步，也不修改市场或资金状态。
/// 主键倒序即时间倒序，最近一轮同步恒在首页首行，运维排障无需再传排序参数。
/// 总数是全表计数而非按状态筛选，因为本查询不追加任何过滤条件。
/// 状态为 running 且结束时间为空的记录表示同步仍在进行，或进程在同步途中崩溃未能回填；
/// 本查询不区分这两种情况，也不会把超时的 running 记录自动改判为失败。
pub(crate) async fn list_admin_sync_logs(
    pool: &Pool<MySql>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<PredictionSyncLogRow>, i64)> {
    // 后台查询同步日志，按 ID 倒序分页返回。
    fetch_admin_page(
        pool,
        QueryBuilder::<MySql>::new(
            r#"SELECT id, trigger_type, status, imported_count, updated_count,
                  error_message, started_at, finished_at
           FROM prediction_sync_logs"#,
        ),
        QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM prediction_sync_logs"),
        " ORDER BY id DESC",
        limit,
        offset,
    )
    .await
}

/// 按市场主键读取完整市场响应，用于详情查询以及下单、结算提交后的结果回读。
/// 复用共享的行查询构建器，因此返回的列与后台列表完全一致，不存在字段裁剪差异。
/// 记录缺失返回 `NotFound`，不返回部分字段也不构造占位市场。
/// 该读取走连接池不加锁，在结算提交后调用时读到的已是提交后的状态；
/// 若需要在事务内取到加锁快照，必须改用 `lock_market`。
pub(crate) async fn load_market_response(
    pool: &Pool<MySql>,
    market_id: u64,
) -> AppResult<PredictionMarketResponse> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE markets.id = ");
    builder.push_bind(market_id);
    builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按上游来源与外部市场编号这一组唯一键定位本地市场，是同步链路把上游条目映射到本地行的入口。
/// 之所以不用本地主键，是因为同步侧只掌握上游标识，本地主键要等 upsert 之后才能知道。
/// 来源一并参与条件，为将来接入其他预测源留出隔离，避免不同源的编号相撞。
/// 市场不存在返回 `NotFound`；同步流程中这通常意味着紧邻的 upsert 未生效，属于异常而非常态。
/// 查询不加锁，返回后市场状态仍可能被并发结算改写，调用方须自行处理该竞态。
pub(crate) async fn load_market_by_source_external(
    pool: &Pool<MySql>,
    source: &str,
    external_market_id: &str,
) -> AppResult<PredictionMarketResponse> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE markets.source = ");
    builder.push_bind(source.to_owned());
    builder.push(" AND markets.external_market_id = ");
    builder.push_bind(external_market_id.to_owned());
    builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按订单主键读取单笔预测订单，连带用户邮箱、市场标题与资产符号等展示字段。
/// 主要用于下单事务提交之后回读最终结果，因此读到的必定是已提交的完整订单。
/// 不带用户维度条件，调用方须自行确保只用于已鉴权的场景，否则会造成越权读取。
/// 订单不存在返回 `NotFound`；本函数不触发结算，也不会推进任何订单状态。
pub(crate) async fn load_order_response(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<PredictionOrderResponse> {
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按用户加幂等键读取既有预测订单，是下单幂等的回读入口，未命中返回 `None` 而非错误。
/// 用户维度进入查询条件，因此幂等键只在各自账户内唯一，不同用户可使用相同的键互不干扰。
/// 命中后调用方直接重放该响应，不再次扣款，也不核对本次请求携带的报价编号，
/// 这意味着同一幂等键配不同报价重发时会拿到首次那张订单，而非报错提示参数不一致。
/// 查询不加锁，被下单路径用在两处：事务前的快路径判重，以及唯一键冲突回滚后的重读。
/// 后者存在读到并发事务尚未提交订单的可能，此时返回 `None`，由调用方转成冲突错误让客户端重试。
pub(crate) async fn load_order_by_idempotency(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<PredictionOrderResponse>> {
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND orders.idempotency_key = ");
    builder.push_bind(idempotency_key.to_owned());
    Ok(builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_optional(pool)
        .await?)
}

/// 在调用方事务内以 `FOR UPDATE` 锁定并读取一份报价，固定下单校验所依据的并发快照。
/// 这是下单事务的第一把锁，排在市场行与钱包行之前，全局锁序由此起始。
/// 一次取回归属用户、市场、方向、资产、本金、手续费、接受价格、份额、理论赔付、
/// 赔付上限、过期时间与消费时间，使后续校验全部基于同一快照而不必二次查询。
/// 行锁保证「判断未消费」与「置上消费时间」之间不会被并发插队，
/// 因此一份报价至多兑换出一张订单。
/// 报价不存在返回 `NotFound`；归属、过期与已消费三项判定由调用方在拿到快照后自行完成。
pub(crate) async fn lock_quote(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
) -> AppResult<PredictionQuoteLockRow> {
    sqlx::query_as::<_, PredictionQuoteLockRow>(
        r#"SELECT quote_id, user_id, market_id, outcome, asset_id, stake_amount,
                  fee_amount, accepted_price, shares, theoretical_payout,
                  effective_payout_cap, market_version, market_last_synced_at,
                  expires_at, consumed_at
           FROM prediction_quotes
           WHERE quote_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(quote_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 返回当前事务连接看到的 MySQL 时间，所有关盘和 quote 过期边界统一以它为准。
pub(crate) async fn database_now_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<DateTime<Utc>> {
    let value = sqlx::query_scalar::<_, chrono::NaiveDateTime>("SELECT CURRENT_TIMESTAMP(6)")
        .fetch_one(&mut **tx)
        .await?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
}

/// 在调用方事务内以 `FOR UPDATE` 锁定并读取一个市场，返回与只读查询完全相同的完整字段。
/// 下单与结算两条链路都经过这里，且都把它排在钱包行之前，这是两者不会死锁的关键。
/// 下单路径靠该锁确认市场在扣款瞬间仍处于可见且未结算；
/// 结算路径靠它把终态判重、订单批量加锁与市场状态改写压进同一临界区，
/// 使并发重复结算中只有一方真正动账，另一方读到终态后空转返回。
/// 市场不存在返回 `NotFound` 并使调用方事务回滚，此时不得继续任何资金或状态写入。
pub(crate) async fn lock_market(
    tx: &mut Transaction<'_, MySql>,
    market_id: u64,
) -> AppResult<PredictionMarketResponse> {
    let market = sqlx::query_as::<_, PredictionMarketResponse>(
        r#"SELECT id, source, external_event_id, external_market_id, slug, title, description,
                  image_url, category, tags_json, outcome_yes_label, outcome_no_label,
                  yes_price, no_price, volume, liquidity, end_at, source_status,
                  display_status, external_resolution, local_resolution, settlement_status,
                  settlement_mode_override, allowed_asset_ids_override_json,
                  payout_cap_overrides_json, fee_rate_override, last_synced_at,
                  market_version, locally_closed_at, created_at, updated_at
           FROM prediction_markets
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(market_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(market)
}

/// 读取资产的符号、精度位数与状态，并要求其处于启用状态，供报价与后台配置校验使用。
/// 资产不存在返回 `NotFound`，存在但已停用返回校验错误，两种失败刻意区分，
/// 前者是引用了不存在的资源，后者是资源存在但当前不可用。
/// 返回的精度位数是本模块所有金额校验与截断的唯一依据，
/// 报价的份额与手续费都按它向下截断，下单前也按它校验本金小数位。
/// 该读取走连接池不加锁，只适合事务外的预校验；
/// 需要与钱包写入处于同一快照时必须改用事务内版本。
pub(crate) async fn load_active_asset(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<PredictionAssetMetaRow> {
    let asset = sqlx::query_as::<_, PredictionAssetMetaRow>(
        "SELECT symbol, precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != service::STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

/// 在下单事务内读取资产的符号、精度与状态，使精度校验与随后的钱包写入落在同一事务快照里。
/// 语句与判定和无锁版本完全一致，区别只在于走事务连接而非连接池；
/// 这里不加 `FOR UPDATE`，因为资产元数据几乎不变，为它取行锁的代价大于收益。
/// 资产在报价之后被停用时，本次读取会返回校验错误并让整笔下单回滚，
/// 因此报价有效期内资产被下架的情形能在扣款前被拦住。
/// 校验通过后的精度用于复核报价固化的本金与手续费，防止历史报价带着超精度金额进入钱包。
pub(crate) async fn load_active_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<PredictionAssetMetaRow> {
    let asset = sqlx::query_as::<_, PredictionAssetMetaRow>(
        "SELECT symbol, precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != service::STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

/// 确认该资产已在预测模块的配置里显式启用，未配置或已停用一律返回校验错误。
/// 未配置与配置为停用被折成同一结果：读不到行时按未启用处理而不是报 `NotFound`，
/// 因为对用户而言两者都表示这个币种不能用来下注。
/// 该校验与资产自身的启用状态是两道独立开关，资产在平台可用不代表可用于竞猜，
/// 因此报价路径两者都要过。
/// 必须发生在报价、下单和任何钱包事务之前；本查询不加锁，
/// 通过后仍可能被后台并发停用，该窗口由下单事务的其余校验兜底。
pub(crate) async fn ensure_prediction_asset_enabled(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let enabled = sqlx::query_as::<_, (bool,)>(
        "SELECT enabled FROM prediction_asset_configs WHERE asset_id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .map(|row| row.0)
    .unwrap_or(false);
    if !enabled {
        return Err(AppError::Validation(
            "asset is not enabled for prediction betting".to_owned(),
        ));
    }
    Ok(())
}

/// 把全局默认设置与单个市场的覆盖配置合并成本次报价实际生效的口径，是纯计算不访问数据库。
/// 允许资产范围的回退条件比其他项更严：覆盖字段不仅要存在，解析出的列表还必须非空，
/// 空数组会被视为未配置并回退到全局范围，避免误存空列表导致该市场彻底无法下注。
/// 费率只要覆盖字段存在就采用，因此可以用零覆盖出一个免手续费的市场。
/// 赔付上限覆盖原样透出为可选 JSON，不在此解析，因为它按资产分别取值，需配合具体资产才能求解。
/// 合并结果只作用于本次报价，不写回数据库，也不影响已有订单固化的口径。
pub(crate) fn effective_market_config(
    settings: &PredictionSettingsRow,
    market: &PredictionMarketResponse,
) -> EffectiveMarketConfig {
    let allowed_asset_ids = market
        .allowed_asset_ids_override_json
        .as_ref()
        .map(|value| service::json_u64_array(&value.0))
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| service::json_u64_array(&settings.allowed_asset_ids_json));
    let fee_rate = market
        .fee_rate_override
        .clone()
        .unwrap_or_else(|| settings.default_fee_rate.clone());
    let payout_cap_overrides = market
        .payout_cap_overrides_json
        .as_ref()
        .map(|value| value.0.clone());
    EffectiveMarketConfig {
        allowed_asset_ids,
        fee_rate,
        payout_cap_overrides,
    }
}

/// 在报价事务内读取赔付上限，避免持有市场锁时再借用第二条连接造成池耗尽或跨快照。
async fn effective_payout_cap_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
    overrides: &Option<Value>,
) -> AppResult<BigDecimal> {
    let asset_key = asset_id.to_string();
    if let Some(value) = overrides
        && let Some(cap) = value
            .get(asset_key.as_str())
            .and_then(service::decimal_from_json)
    {
        return Ok(cap);
    }
    Ok(sqlx::query_as::<_, (BigDecimal,)>(
        "SELECT max_payout_amount FROM prediction_asset_configs WHERE asset_id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|row| row.0)
    .unwrap_or_else(|| BigDecimal::from(0)))
}

/// 逐个校验后台提交的资产范围里每个编号都存在且处于启用状态，任一不满足即整体报错。
/// 校验前先去重并剔除零值，因此重复填写不会产生多余查询，零也不会被当成有效资产。
/// 采用逐条串行查询而非一次批量查询，代价是随列表长度线性增加往返次数，
/// 但换来错误能精确指向首个非法资产。
/// 全有或全无：不返回部分校验结果，禁止保存出一份混有无效编号的资产范围。
pub(crate) async fn validate_asset_ids_exist(pool: &Pool<MySql>, ids: &[u64]) -> AppResult<()> {
    for id in service::unique_u64_list(ids.to_vec()) {
        load_active_asset(pool, id).await?;
    }
    Ok(())
}

/// 在竞猜下单事务内冻结本金并扣除手续费；调用前订单、报价归属及资产精度均应已验证。
/// 锁定或初始化钱包后校验可用余额足以覆盖本金与手续费，再一次性更新 available 与 frozen。
/// 本金必须成对写入 available 扣减和 frozen 增加流水，正手续费另写 available 扣减流水，快照须与余额一致。
/// 本函数不提交也不独立去重；调用方以已插入的订单幂等键保证只执行一次，且无提交后副作用。
/// 余额判定按本金加手续费的总额一次性比较，不允许只够本金就先冻结再欠费，
/// 因此要么两笔都成功，要么整笔下单以校验错误回滚。
/// 资金流向是本金由 available 转入 frozen、手续费从 available 直接离场：
/// 前者仍属于用户只是被冻结，后者已不再属于用户，两者性质不同故分开记账。
/// 钱包只更新一次，落库的是扣完手续费后的终值；但两条本金流水的快照刻意记为扣费前的中间值，
/// 使 available 腿的 `balance_after` 与其变动量严格对应，账本可按流水顺序逐笔复现。
/// 手续费为零时不写流水，避免账本里出现大量零额记录；负手续费不会发生，因为报价阶段已保证其非负。
/// 全部三笔流水的 change_type 分为冻结与手续费两类，ref 统一指向订单编号，便于按单追溯。
/// 本函数只写钱包与流水，订单状态、报价消费与佣金登记由调用方在同一事务内各自完成。
pub(crate) async fn apply_wallet_prediction_open(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    fee_amount: &BigDecimal,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let total_required = stake_amount.clone() + fee_amount.clone();
    if wallet.available < total_required {
        return Err(AppError::Validation(format!(
            "insufficient available balance for prediction order: requested {}, available {}",
            stake_amount.clone() + fee_amount.clone(),
            wallet.available
        )));
    }
    let available_after_stake = wallet.available.clone() - stake_amount.clone();
    let frozen_after = wallet.frozen.clone() + stake_amount.clone();
    let available_after_fee = available_after_stake.clone() - fee_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after_fee)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "available",
        &available_after_stake,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_freeze",
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_freeze",
        order_id,
    )
    .await?;
    if fee_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            -fee_amount.clone(),
            "available",
            &available_after_fee,
            &available_after_fee,
            &frozen_after,
            &wallet.locked,
            "prediction_fee",
            order_id,
        )
        .await?;
    }
    Ok(())
}

/// 在竞猜结算事务内释放订单冻结本金并按胜负写结算流水；调用前订单必须已锁定且仍为 open。
/// 锁定或初始化钱包后要求 frozen 足以覆盖本金，再同步扣减 frozen 并把已计算派奖加入 available。
/// 每单必写一条胜/负本金结算流水，正派奖另写 available 流水；余额、快照和金额必须一致。
/// 本函数不提交也不独立防重；调用方依靠订单终态锁与市场结算幂等性阻止重复派奖。
/// 与开仓相反，本金是单向离开 frozen 而不回到 available：押中与否都不退本金，
/// 用户的收益完全体现在派奖金额上，因此败方的 available 不发生任何变化。
/// 派奖额由调用方按理论赔付并经赔付上限封顶后算出，本函数原样入账不再校验或截断，
/// 传入负值会写出反向变动，调用方必须保证其非负。
/// `won` 只影响本金流水的 change_type，用于把胜负两类结算在账本里分开统计，不参与金额计算；
/// 因此该标记必须与派奖额的算法保持一致，否则会出现标记为负却有派奖的矛盾记录。
/// 冻结余额不足说明本金已被其他路径动过，返回校验错误并让整批结算回滚，
/// 宁可整个市场结算失败也不允许出现负冻结。
/// 派奖为零时不写第二条流水，因此败方订单在账本上只留一条解冻记录。
pub(crate) async fn apply_wallet_prediction_settlement(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    payout_amount: &BigDecimal,
    order_id: u64,
    won: bool,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for prediction settlement: requested {}, frozen {}",
            stake_amount, wallet.frozen
        )));
    }
    let frozen_after = wallet.frozen.clone() - stake_amount.clone();
    let available_after = wallet.available.clone() + payout_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        if won {
            "prediction_settle_win"
        } else {
            "prediction_settle_loss"
        },
        order_id,
    )
    .await?;
    if payout_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            payout_amount.clone(),
            "available",
            &available_after,
            &available_after,
            &frozen_after,
            &wallet.locked,
            "prediction_payout",
            order_id,
        )
        .await?;
    }
    Ok(())
}

/// 在无效竞猜退款事务内解冻并退回本金，可按已选退款策略额外退还正手续费。
/// 调用前订单必须已锁定且 frozen 足以覆盖本金；本函数锁钱包后同步更新 available 与 frozen。
/// 本金退款必须写 available 增加和 frozen 减少两条流水，手续费退款另写流水，所有余额快照保持一致。
/// 本函数不提交也不独立防重；调用方以订单终态和市场锁保证退款仅执行一次，且无提交后副作用。
/// 本金走的是开仓冻结的逆向路径：从 frozen 扣减并等额加回 available，用户资产净额恢复原状。
/// 手续费退款则是额外的一笔 available 入账，是否退、退多少完全由调用方按退款策略决定；
/// 策略为只退本金时传零，本函数据此跳过该笔流水而不做策略判断。
/// 钱包只更新一次并落入含手续费退款的终值，但本金两条流水的快照记为退费前的中间值，
/// 与开仓路径的记账方式对称，保证账本可按顺序逐笔复现余额演进。
/// 冻结余额不足说明本金已被他处动过，返回校验错误并让整批退款回滚，不做部分退款。
/// 本函数不校验退款金额是否等于原始手续费，多退少退都会被如实执行，
/// 因此策略解析的正确性必须由调用方保证。
pub(crate) async fn apply_wallet_prediction_refund(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    fee_refund_amount: &BigDecimal,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for prediction refund: requested {}, frozen {}",
            stake_amount, wallet.frozen
        )));
    }
    let available_after_stake = wallet.available.clone() + stake_amount.clone();
    let frozen_after = wallet.frozen.clone() - stake_amount.clone();
    let available_after_fee = available_after_stake.clone() + fee_refund_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after_fee)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        stake_amount.clone(),
        "available",
        &available_after_stake,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_refund",
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_refund",
        order_id,
    )
    .await?;
    if fee_refund_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            fee_refund_amount.clone(),
            "available",
            &available_after_fee,
            &available_after_fee,
            &frozen_after,
            &wallet.locked,
            "prediction_fee_refund",
            order_id,
        )
        .await?;
    }
    Ok(())
}

/// 确保钱包账户行存在后在调用方事务内加锁读取三态余额，是三个资金函数共同的取锁入口。
/// 先执行 `INSERT IGNORE` 以三态全零建号：行已存在时是空操作，绝不会把余额清零；
/// 行不存在时自动开户，使用户首次持有某资产也能顺利完成结算派奖或退款。
/// 随后以 `FOR UPDATE` 回读，因此并发首次开户由唯一键收敛，最终至多一行。
/// 该锁是全局锁序中最细的一层，必须在报价与市场行锁之后获取；
/// 结算路径逐单加锁时按订单主键升序推进，因此同一批钱包的加锁顺序在并发结算间保持一致。
/// 建号后仍读不到行属于异常情形，返回校验错误并让整个事务回滚。
pub(crate) async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<PredictionWalletRow> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query_as::<_, PredictionWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required".to_owned()))
}

#[allow(clippy::too_many_arguments)]
/// 在预测订单事务内写一条钱包流水，是本模块所有余额变动的统一审计出口。
/// `amount` 为带符号的本次变动量，`balance_type` 标注变动落在 available、frozen 还是 locked 哪条腿，
/// `balance_after` 是该腿变动后的值，随后三个 after 参数是变动后完整的三态快照。
/// 金额与快照必须与同次余额更新严格一致，本函数不回读钱包核对，传错会写出对不上的账本且不报错。
/// ref_type 固定为预测订单，ref_id 取订单主键的字符串形式，因此按订单可反查其全部资金腿。
/// change_type 由调用方按冻结、手续费、胜负结算、派奖、本金退款、手续费退款等场景分别传入，
/// 是账本上区分资金性质的唯一依据。
/// 一次调用只写一条记录，同时影响两条腿的动作需由调用方分别写入两次。
/// 本函数不提交事务；写入失败会让调用方回滚整笔下单或整批结算。
pub(crate) async fn insert_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(service::REF_TYPE_PREDICTION_ORDER)
    .bind(order_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 从应用状态中取出 MySQL 连接池，未配置时返回内部错误而不尝试同步、下单或资金写入。
/// 归为内部错误而非参数校验错误，因为连接池缺失属于部署配置问题，
/// 不应引导客户端重试，也便于在监控中与业务拒绝区分开。
/// 返回的是池句柄的克隆，克隆廉价且不新建物理连接。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state
        .mysql
        .clone()
        .ok_or_else(|| AppError::Internal("mysql pool is not configured".to_owned()))
}

/// 把 Polymarket 的网络、状态码或载荷解析失败统一包装成对外的同步失败错误。
/// 固定使用 502 状态与稳定的错误码，让调用方能据码判定是上游问题而非本服务缺陷。
/// 错误文本先经压缩与截断，避免上游返回的大段 HTML 或堆栈直接进入响应与日志。
/// 返回错误意味着放弃本轮拉取，已持久化的旧市场快照原样保留，
/// 重试由调度器按同步间隔发起，本函数不做任何自动重试。
pub(crate) fn upstream_sync_error(message: String) -> AppError {
    AppError::Api {
        status: StatusCode::BAD_GATEWAY,
        code: "POLYMARKET_SYNC_FAILED",
        message: service::compact_error_message(&message),
    }
}
