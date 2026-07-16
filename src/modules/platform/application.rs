//! platform bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    error::AppResult,
    modules::platform::{
        domain::{DEFAULT_CHART_PROVIDER, validate_platform_brand},
        infrastructure::{
            ensure_default_platform_brand, ensure_default_platform_brand_in_tx,
            load_platform_brand_in_tx, load_platform_brand_row, lock_platform_brand_in_tx,
            upsert_platform_brand_in_tx,
        },
        presentation::{PlatformBrandResponse, SavePlatformBrandRequest},
    },
};
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug)]
pub struct PlatformBrandChange {
    pub before: PlatformBrandResponse,
    pub after: PlatformBrandResponse,
}

impl ApplicationLayer for PlatformBrandChange {}

pub async fn load_platform_brand(pool: &Pool<MySql>) -> AppResult<PlatformBrandResponse> {
    ensure_default_platform_brand(pool).await?;
    load_platform_brand_row(pool).await
}

pub async fn save_platform_brand_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    request: SavePlatformBrandRequest,
) -> AppResult<PlatformBrandChange> {
    // 平台品牌配置变更需要先锁定旧值，审计日志才能记录完整 before/after。
    ensure_default_platform_brand_in_tx(tx).await?;
    let before = lock_platform_brand_in_tx(tx).await?;
    let brand = validate_platform_brand(&request)?;
    // 兼容未升级的管理端：未提交图表引擎时保留已发布配置，不能静默回退到默认实现。
    let chart_provider =
        brand
            .chart_provider
            .as_deref()
            .unwrap_or(if before.chart_provider.is_empty() {
                DEFAULT_CHART_PROVIDER
            } else {
                &before.chart_provider
            });
    upsert_platform_brand_in_tx(
        tx,
        admin_id,
        &brand.platform_name,
        &brand.logo_url,
        chart_provider,
    )
    .await?;
    let after = load_platform_brand_in_tx(tx).await?;
    Ok(PlatformBrandChange { before, after })
}
