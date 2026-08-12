//! platform bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    modules::platform::{
        domain::{
            DEFAULT_CHART_PROVIDER, PlatformBrand, PlatformBrandCommand, validate_platform_brand,
        },
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

/// 读取当前平台品牌配置，并在首次部署时幂等补齐默认记录。
/// 默认记录初始化和后续查询分别执行；数据库失败时不构造虚假的默认响应。
pub async fn load_platform_brand(pool: &Pool<MySql>) -> AppResult<PlatformBrandResponse> {
    ensure_default_platform_brand(pool).await?;
    Ok(platform_brand_response(
        load_platform_brand_row(pool).await?,
    ))
}

/// 在调用方事务中校验并保存平台品牌配置，同时返回完整变更前后快照。
/// 本函数不提交事务；图表提供方缺省时保留旧值，失败由调用方统一回滚并停止审计落库。
pub async fn save_platform_brand_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    request: SavePlatformBrandRequest,
) -> AppResult<PlatformBrandChange> {
    // 平台品牌配置变更需要先锁定旧值，审计日志才能记录完整 before/after。
    ensure_default_platform_brand_in_tx(tx).await?;
    let before = lock_platform_brand_in_tx(tx).await?;
    let brand = validate_platform_brand(platform_brand_command(request))
        .map_err(|error| AppError::Validation(error.into_message()))?;
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
    Ok(PlatformBrandChange {
        before: platform_brand_response(before),
        after: platform_brand_response(after),
    })
}

fn platform_brand_command(request: SavePlatformBrandRequest) -> PlatformBrandCommand {
    PlatformBrandCommand {
        platform_name: request.platform_name,
        logo_url: request.logo_url,
        chart_provider: request.chart_provider,
    }
}

fn platform_brand_response(brand: PlatformBrand) -> PlatformBrandResponse {
    PlatformBrandResponse {
        id: brand.id,
        name: brand.name,
        platform_name: brand.platform_name,
        logo_url: brand.logo_url,
        chart_provider: brand.chart_provider,
        updated_by: brand.updated_by,
        created_at: brand.created_at,
        updated_at: brand.updated_at,
    }
}
