use super::*;

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

pub(crate) async fn get_admin_margin_liquidation(
    pool: Option<Pool<MySql>>,
    liquidation_id: u64,
) -> AppResult<AdminMarginLiquidationResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_margin_liquidation_from_store(&pool, liquidation_id).await
}
