//! 杠杆强平记录的只读查询用例层，仅为后台提供事后取证视图。
//!
//! 两个用例都不参与保证金结算事务，不锁仓位与钱包，也不会触发任何重新计算或补偿，
//! 因此可以安全地在强平高峰期查询而不影响清算链路。

use super::*;

/// 按用户、邮箱、交易对和仓位筛选保证金强平记录，并返回分页明细和匹配总数。
/// 邮箱去除空白，分页边界统一裁剪；读取不锁仓位、钱包或强平记录，也不会触发重新结算。
/// 用户编号、交易对与仓位编号按精确值匹配，四项条件之间是并列收窄关系，未提供的项不参与过滤。
/// 记录反映强平发生当时的价格与费用，后续行情变化不会改写这些历史数值。
pub(crate) async fn list_admin_margin_liquidations(
    pool: Option<Pool<MySql>>,
    query: AdminMarginLiquidationQuery,
) -> AppResult<AdminMarginLiquidationsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (liquidations, total) = list_admin_margin_liquidations_from_store(
        &pool,
        AdminMarginLiquidationListFilter {
            user_id: query.user_id,
            email: query.email.and_then(optional_string),
            pair_id: query.pair_id,
            position_id: query.position_id,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminMarginLiquidationsResponse {
        liquidations,
        total,
    })
}

/// 按强平记录 ID 读取用户、仓位、交易对、价格、费用和时间组成的后台详情。
/// 查询不参与保证金事务；记录缺失返回未找到，SQL 或数值解码失败返回错误，不修改仓位或余额。
pub(crate) async fn get_admin_margin_liquidation(
    pool: Option<Pool<MySql>>,
    liquidation_id: u64,
) -> AppResult<AdminMarginLiquidationResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_margin_liquidation_from_store(&pool, liquidation_id).await
}
