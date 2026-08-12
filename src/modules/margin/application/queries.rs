use super::support::{normalized_position_status, optional_string, route_limit, route_offset};
use crate::{
    error::{AppError, AppResult},
    modules::margin::{
        infrastructure::{
            cached_margin_risk_ticker, list_admin_interest_summary, list_admin_margin_positions,
            list_margin_wallet_accounts, list_user_cross_margin_accounts,
            list_user_margin_positions as list_user_margin_positions_rows,
            load_admin_margin_position_by_id, load_user_position_by_id,
            load_user_risk_position_by_id,
        },
        presentation::{
            AdminInterestSummaryQuery, AdminInterestSummaryResponse, AdminListPositionsQuery,
            AdminMarginPositionResponse, AdminMarginPositionsResponse,
            MarginPositionDetailResponse, MarginPositionsResponse, MarginRiskSnapshot,
            MarginRiskSnapshotResponse, MarginWalletsResponse,
        },
    },
    workers::margin_liquidation::margin_liquidation_risk_state,
};
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
/// 按用户、状态和上限查询保证金仓位读模型；只读失败不返回部分结果。
pub(crate) async fn list_user_margin_positions(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<String>,
    limit: u32,
) -> AppResult<MarginPositionsResponse> {
    let status = optional_string(status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let positions =
        list_user_margin_positions_rows(pool, user_id, status.as_deref(), limit).await?;
    Ok(MarginPositionsResponse { positions })
}

/// 汇总用户保证金钱包、已开仓仓位与全仓账户快照；该读模型不重算或写入资金。
/// 任一数据源失败返回错误而非混用不完整快照，重复读取只随最新持久化状态变化。
pub(crate) async fn list_user_margin_wallets(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<MarginWalletsResponse> {
    let wallets = list_margin_wallet_accounts(pool, user_id).await?;
    let positions = list_user_margin_positions_rows(pool, user_id, Some("opened"), limit).await?;
    let cross_accounts = list_user_cross_margin_accounts(pool, user_id).await?;
    Ok(MarginWalletsResponse {
        wallets,
        positions,
        cross_accounts,
    })
}

/// 按用户和仓位标识读取详情，防止仅凭仓位主键越权读取其他账户数据。
pub(crate) async fn get_user_margin_position(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<MarginPositionDetailResponse> {
    let position = load_user_position_by_id(pool, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(MarginPositionDetailResponse { position })
}

/// 按后台用户、邮箱、交易对和状态条件查询仓位历史及一致总数；不修改仓位。
pub(crate) async fn list_admin_margin_position_history(
    pool: &Pool<MySql>,
    query: AdminListPositionsQuery,
) -> AppResult<AdminMarginPositionsResponse> {
    let status = optional_string(query.status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let (positions, total) = list_admin_margin_positions(
        pool,
        query.user_id,
        query.email,
        query.pair_id,
        status.as_deref(),
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminMarginPositionsResponse { positions, total })
}

/// 读取后台保证金仓位详情；记录缺失返回 NotFound，不获取钱包写锁。
pub(crate) async fn get_admin_margin_position(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<AdminMarginPositionResponse> {
    load_admin_margin_position_by_id(pool, position_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按后台筛选聚合保证金利息并返回一致总数；该查询不执行计提或结算。
pub(crate) async fn list_admin_margin_interest_summary(
    pool: &Pool<MySql>,
    query: AdminInterestSummaryQuery,
) -> AppResult<AdminInterestSummaryResponse> {
    let status = optional_string(query.status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let (summaries, total) = list_admin_interest_summary(
        pool,
        query.user_id,
        query.email,
        query.pair_id,
        status.as_deref(),
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminInterestSummaryResponse { summaries, total })
}

/// 读取用户已开仓仓位的即时风险快照；仓位必须存在、状态为 opened 且保留有效入场价。
/// 使用服务端新鲜行情计算盈亏、权益、维持保证金和强平条件，不开启事务也不锁定持久化行。
/// 本函数只读，不修改余额、流水或仓位状态；重复调用按当次行情重新计算，因此不承诺相同结果。
/// 返回后没有事件发布或其他外部副作用，行情缺失、过期或非法时直接失败。
pub(crate) async fn get_margin_position_risk_snapshot(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    position_id: u64,
) -> AppResult<MarginRiskSnapshotResponse> {
    let position = load_user_risk_position_by_id(pool, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if position.status != "opened" {
        return Err(AppError::Validation(
            "margin risk snapshot requires an opened position".to_owned(),
        ));
    }
    let Some(entry_price) = position.entry_price.clone() else {
        return Err(AppError::Validation(
            "margin entry price is required for risk snapshot".to_owned(),
        ));
    };
    let ticker = cached_margin_risk_ticker(redis, position.pair_id, &position.symbol).await?;
    let risk_state = margin_liquidation_risk_state(
        &position.direction,
        &position.margin_amount,
        &position.notional_amount,
        &position.interest_amount,
        &entry_price,
        &ticker.last_price,
        &position.maintenance_margin_rate,
    )?;
    Ok(MarginRiskSnapshotResponse {
        risk: MarginRiskSnapshot {
            position_id: position.id,
            pair_id: position.pair_id,
            symbol: position.symbol,
            margin_asset: position.margin_asset,
            direction: position.direction,
            margin_amount: position.margin_amount,
            notional_amount: position.notional_amount,
            interest_amount: position.interest_amount,
            entry_price,
            mark_price: ticker.last_price,
            maintenance_margin_rate: position.maintenance_margin_rate,
            realized_pnl: risk_state.realized_pnl,
            equity: risk_state.equity,
            maintenance_margin: risk_state.maintenance_margin,
            should_liquidate: risk_state.should_liquidate,
            observed_at: ticker.observed_at,
        },
    })
}
