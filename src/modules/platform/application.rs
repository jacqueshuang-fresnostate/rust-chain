//! platform bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 平台品牌配置只有两个用例：面向前端的读取，以及后台在事务内的保存。
//! 读取会先幂等补齐默认行，保证首次部署也能拿到可用配置；保存则先锁旧值再写入并回读，
//! 产出的前后快照供调用方写审计日志，本层自身不提交事务也不记录审计。

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

/// 一次品牌配置保存的前后快照，前值取自加锁读取，后值取自同事务内的回读。
/// 供调用方写入审计日志并对外返回，事务尚未提交时该结构即已生成。
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
/// 执行顺序固定为幂等补齐默认行、加锁读取旧值、校验入参、写入、再同事务回读新值，锁一直持有到调用方结束事务。
/// 旧值为空串这种历史脏数据会回落到默认提供方，避免把空取值继续传播下去。
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

/// 把后台保存请求原样搬成领域命令，只做字段搬运不做任何归一或校验。
/// 之所以保留这层转换，是让领域校验只依赖领域自身的入参类型，不必反向依赖表现层的请求结构。
fn platform_brand_command(request: SavePlatformBrandRequest) -> PlatformBrandCommand {
    PlatformBrandCommand {
        platform_name: request.platform_name,
        logo_url: request.logo_url,
        chart_provider: request.chart_provider,
    }
}

/// 把品牌配置的领域快照逐字段转成对外响应，保存路径会分别对变更前后各调用一次。
/// 全部字段直接透出，不隐藏最后修改人也不脱敏，因此该结构只用于后台接口与审计留痕。
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
