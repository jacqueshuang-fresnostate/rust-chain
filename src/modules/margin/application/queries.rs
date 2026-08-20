//! 杠杆读模型用例。
//!
//! 汇集用户侧和后台的全部只读查询：仓位列表与详情、钱包与全仓账户汇总、后台仓位历史、
//! 利息汇总报表，以及需要实时行情的仓位/全仓账户风险快照。
//! 除风险快照外，所有数值都直接取自数据库既有落库值，不做重新估值，也不触发计提或强平。
//! 用户侧查询一律带上用户标识作为过滤条件，防止仅凭仓位主键越权读取他人持仓。
//! 后台查询把状态筛选先做白名单归一化，并让行查询与 COUNT 共用同一组谓词，保证明细与总数口径一致。

use super::support::{normalized_position_status, optional_string, route_limit, route_offset};
use crate::{
    error::{AppError, AppResult},
    modules::margin::{
        domain::{
            CrossMarginPositionRisk, CrossMarginReferencePosition, MarginPositionDisplayInput,
            estimate_cross_margin_conditional_price, evaluate_cross_margin,
            margin_position_display_metrics,
        },
        infrastructure::{
            MarginRiskPositionRow, MarginRiskTicker, cached_margin_risk_ticker,
            list_admin_interest_summary, list_admin_margin_positions, list_margin_wallet_accounts,
            list_user_cross_margin_accounts, list_user_cross_margin_risk_positions,
            list_user_margin_positions as list_user_margin_positions_rows,
            load_admin_margin_position_by_id, load_user_cross_margin_wallet_available,
            load_user_position_by_id, load_user_risk_position_by_id,
        },
        presentation::{
            AdminInterestSummaryQuery, AdminInterestSummaryResponse, AdminListPositionsQuery,
            AdminMarginPositionResponse, AdminMarginPositionsResponse, MarginCrossAccountRisk,
            MarginPositionDetailResponse, MarginPositionsResponse, MarginRiskSnapshot,
            MarginRiskSnapshotResponse, MarginWalletsResponse,
        },
    },
    workers::margin_liquidation::{MarginLiquidationRiskState, margin_liquidation_risk_state},
};
use bigdecimal::BigDecimal;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
use std::collections::BTreeMap;
/// 查询指定用户的杠杆仓位列表，按仓位主键倒序返回最近的若干条。
/// 状态参数先裁剪空白折叠为 None，再做四值白名单校验，非法值在拼查询之前就报参数错误。
/// 不传状态表示不加状态条件，因此持仓、已平、已强平和已撤销会混在同一页里返回。
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
/// 三次查询各自独立且不在同一事务内，因此严格来说不是同一时点的一致性快照，
/// 极端并发下可能读到刚平仓的钱包余额配上尚未消失的仓位，展示层需容忍这种短暂偏差。
/// 仓位部分固定只取 opened，全仓账户的权益与保证金率是强平 worker 上次刷新的落库值。
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

/// 按用户和仓位主键联合读取单条仓位详情，用户标识来自 JWT，杜绝凭主键越权读他人持仓。
/// 记录不存在与记录属于他人都统一映射为 NotFound，不区分两者以免探测出主键是否有效。
/// 返回的是仓位行的落库快照，不含实时浮盈，实时风险需另走风险快照接口。
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

/// 后台跨账户检索杠杆仓位历史，支持用户标识、邮箱、交易对和状态四维组合筛选并分页。
/// 邮箱条件以参数化的 EXISTS 子查询关联用户表，不做字符串拼接；状态先过四值白名单。
/// 分页参数在这里才归一化，`limit` 夹到 1 到 100，`offset` 封顶十万以避免深翻页拖垮大表。
/// 返回体带上与列表同筛选口径的总数，明细页与分页器不会出现口径分裂；全程只读不改仓位。
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

/// 后台按仓位主键读取单条详情，不带用户维度约束，可查看任意账户的持仓。
/// 相比用户侧详情多返回强平时间和强平原因两列，用于事后核查风控处置过程。
/// 记录缺失返回 NotFound；该只读路径不加行锁，也不会顺带触发利息计提或强平判定。
pub(crate) async fn get_admin_margin_position(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<AdminMarginPositionResponse> {
    load_admin_margin_position_by_id(pool, position_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 后台利息汇总报表，按保证金币种和仓位状态分组统计仓位笔数、借款额合计与已计提利息合计。
/// 与仓位历史共用同一套筛选谓词和归一化规则，因此报表数字可以和明细页逐条对上。
/// 总数按分组键去重统计而非按行统计，分页器展示的是分组个数而不是仓位条数。
/// 纯读聚合，数值全部取自仓位行上由利息 worker 写入的 `interest_amount`，本查询不执行任何计提或结算。
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
/// 逐仓模式仅读当前 pair 的服务端新鲜行情，保持既有顶层单仓字段口径不变。
/// 全仓模式按同用户、同保证金资产汇总所有 cross/opened/已成交仓位，
/// 每个唯一 pair 只取一次行情；任一 pair 缺价则整个账户快照失败，不返回混合时点的部分数据。
/// 本函数只读，不修改余额、流水或仓位状态；重复调用按当次行情重新计算，因此不承诺相同结果。
/// 返回后没有事件发布或其他外部副作用，行情缺失、过期或非法时直接失败。
///
/// 风险计算直接复用强平 worker 的 `margin_liquidation_risk_state`，保证接口展示的强平判定
/// 与后台任务实际执行的判定同源，不会出现页面显示安全但下一轮就被强平的口径分歧。
/// 维持保证金率取自仓位关联产品的当前配置，因此管理员改配后本接口会立即反映新的强平线。
/// 响应里带上行情的 `observed_at`，调用方可据此判断快照的新鲜度。
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
    if position.margin_mode == "cross" {
        return get_cross_margin_position_risk_snapshot(
            pool,
            redis,
            user_id,
            position_id,
            position.margin_asset,
        )
        .await;
    }
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
    build_margin_risk_snapshot(position, entry_price, ticker, risk_state, None)
}

/// 为全仓仓位构建权威账户级快照，并保留被查仓位的顶层兼容字段。
/// 行查询已在 SQL 中严格限定同用户、同资产、cross、opened 和非空入场价；
/// 这里先按 pair 去重再串行取价，保证同 pair 多空腿共用完全相同的标记价和观测时间。
async fn get_cross_margin_position_risk_snapshot(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    position_id: u64,
    margin_asset: u64,
) -> AppResult<MarginRiskSnapshotResponse> {
    let positions = list_user_cross_margin_risk_positions(pool, user_id, margin_asset).await?;
    let reference_position = positions
        .iter()
        .find(|position| position.id == position_id)
        .cloned()
        .ok_or_else(|| {
            AppError::Conflict("cross margin account changed during risk snapshot".to_owned())
        })?;

    let mut symbols_by_pair = BTreeMap::<u64, String>::new();
    for position in &positions {
        if let Some(existing_symbol) = symbols_by_pair.get(&position.pair_id) {
            if existing_symbol != &position.symbol {
                return Err(AppError::Internal(format!(
                    "pair {} has inconsistent symbols in cross margin risk rows",
                    position.pair_id
                )));
            }
        } else {
            symbols_by_pair.insert(position.pair_id, position.symbol.clone());
        }
    }

    let mut tickers_by_pair = BTreeMap::<u64, MarginRiskTicker>::new();
    for (pair_id, symbol) in symbols_by_pair {
        let ticker = cached_margin_risk_ticker(redis, pair_id, &symbol).await?;
        tickers_by_pair.insert(pair_id, ticker);
    }

    let wallet_available =
        load_user_cross_margin_wallet_available(pool, user_id, margin_asset).await?;
    let mut position_margin = BigDecimal::from(0);
    let mut account_position_risks = Vec::with_capacity(positions.len());
    let mut reference_positions = Vec::with_capacity(positions.len());
    let mut reference_risk_state = None;
    for position in &positions {
        let entry_price = position.entry_price.as_ref().ok_or_else(|| {
            AppError::Internal("cross margin risk query returned a null entry price".to_owned())
        })?;
        let ticker = tickers_by_pair.get(&position.pair_id).ok_or_else(|| {
            AppError::Internal(
                "cross margin risk ticker missing after complete prefetch".to_owned(),
            )
        })?;
        let risk_state = margin_liquidation_risk_state(
            &position.direction,
            &position.margin_amount,
            &position.notional_amount,
            &position.interest_amount,
            entry_price,
            &ticker.last_price,
            &position.maintenance_margin_rate,
        )?;
        position_margin += position.margin_amount.clone();
        account_position_risks.push(CrossMarginPositionRisk {
            unrealized_pnl: risk_state.realized_pnl.clone(),
            interest_amount: position.interest_amount.clone(),
            maintenance_margin: risk_state.maintenance_margin.clone(),
        });
        reference_positions.push(CrossMarginReferencePosition {
            pair_id: position.pair_id,
            direction: &position.direction,
            notional_amount: &position.notional_amount,
            entry_price,
        });
        if position.id == position_id {
            reference_risk_state = Some(risk_state);
        }
    }
    let account_risk = evaluate_cross_margin(
        &wallet_available,
        &position_margin.with_scale(18),
        &account_position_risks,
    );
    let liquidation_buffer =
        (account_risk.equity.clone() - account_risk.maintenance_margin.clone()).with_scale(18);
    let reference_ticker = tickers_by_pair
        .get(&reference_position.pair_id)
        .cloned()
        .ok_or_else(|| {
            AppError::Internal(
                "reference ticker missing after complete cross margin prefetch".to_owned(),
            )
        })?;
    let conditional_price = estimate_cross_margin_conditional_price(
        reference_position.pair_id,
        &reference_ticker.last_price,
        &liquidation_buffer,
        reference_position.price_precision,
        &reference_positions,
    )
    .map_err(|message| AppError::Validation(message.to_owned()))?;
    let marks_observed_at_min = tickers_by_pair
        .values()
        .map(|ticker| ticker.observed_at)
        .min()
        .ok_or_else(|| AppError::Internal("cross margin account has no marks".to_owned()))?;
    let marks_observed_at_max = tickers_by_pair
        .values()
        .map(|ticker| ticker.observed_at)
        .max()
        .ok_or_else(|| AppError::Internal("cross margin account has no marks".to_owned()))?;
    let cross_account_risk = MarginCrossAccountRisk {
        margin_asset,
        reference_pair_id: reference_position.pair_id,
        price_assumption: "reference_pair_only_other_marks_static",
        equity: account_risk.equity,
        maintenance_margin: account_risk.maintenance_margin,
        liquidation_buffer,
        margin_ratio: account_risk.margin_ratio,
        unrealized_pnl: account_risk.unrealized_pnl,
        interest_amount: account_risk.interest_amount,
        should_liquidate: account_risk.should_liquidate,
        net_quantity: conditional_price.net_quantity,
        gross_quantity: conditional_price.gross_quantity,
        estimate_status: conditional_price.status.as_str(),
        conditional_liquidation_price: conditional_price.price,
        conditional_liquidation_distance_rate: conditional_price.distance_rate,
        marks_observed_at_min,
        marks_observed_at_max,
    };
    let reference_entry_price = reference_position.entry_price.clone().ok_or_else(|| {
        AppError::Internal("cross margin reference position lost its entry price".to_owned())
    })?;
    let reference_risk_state = reference_risk_state.ok_or_else(|| {
        AppError::Internal("cross margin reference risk state was not calculated".to_owned())
    })?;
    build_margin_risk_snapshot(
        reference_position,
        reference_entry_price,
        reference_ticker,
        reference_risk_state,
        Some(cross_account_risk),
    )
}

/// 把一笔仓位的实时风险、行情与可选全仓账户风险组装成稳定 HTTP 结构。
/// 全仓仍然调用公共展示算法，因此旧顶层的单仓估算强平价继续返回 null；
/// 逐仓传 None 时不序列化 `cross_account_risk`，避免破坏已有客户端合同。
fn build_margin_risk_snapshot(
    position: MarginRiskPositionRow,
    entry_price: BigDecimal,
    ticker: MarginRiskTicker,
    risk_state: MarginLiquidationRiskState,
    cross_account_risk: Option<MarginCrossAccountRisk>,
) -> AppResult<MarginRiskSnapshotResponse> {
    let display_metrics = margin_position_display_metrics(MarginPositionDisplayInput {
        margin_mode: &position.margin_mode,
        direction: &position.direction,
        margin_amount: &position.margin_amount,
        notional_amount: &position.notional_amount,
        interest_amount: &position.interest_amount,
        entry_price: &entry_price,
        mark_price: &ticker.last_price,
        unrealized_pnl: &risk_state.realized_pnl,
        equity: &risk_state.equity,
        maintenance_margin: &risk_state.maintenance_margin,
    })
    .map_err(|message| AppError::Validation(message.to_owned()))?;
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
            unrealized_pnl: risk_state.realized_pnl.clone(),
            realized_pnl: risk_state.realized_pnl,
            equity: risk_state.equity,
            maintenance_margin: risk_state.maintenance_margin,
            position_quantity: display_metrics.position_quantity,
            return_rate: display_metrics.return_rate,
            margin_ratio: display_metrics.margin_ratio,
            estimated_liquidation_price: display_metrics.estimated_liquidation_price,
            liquidation_distance_rate: display_metrics.liquidation_distance_rate,
            should_liquidate: risk_state.should_liquidate,
            observed_at: ticker.observed_at,
            cross_account_risk,
        },
    })
}
