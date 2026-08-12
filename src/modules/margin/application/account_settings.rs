use super::support::{
    decimal_matches_string, ensure_supported_user_margin_mode, is_duplicate_key_error,
    normalized_margin_mode, validate_positive_decimal,
};
use crate::{
    error::{AppError, AppResult},
    modules::{
        margin::{
            infrastructure::{
                MarginProductSettingRule, insert_margin_transfer,
                load_margin_transfer_by_idempotency_key, load_margin_transfer_wallet_snapshots,
                load_user_margin_setting, load_user_margin_setting_from_pool,
                lock_active_product_setting_rule, resolve_active_transfer_asset,
                resolve_transfer_asset_id_for_replay, transfer_margin_to_spot_wallets,
                transfer_spot_to_margin_wallets, upsert_user_margin_setting,
            },
            presentation::{
                MarginUserSettingResponse, TransferMarginFundsRequest, TransferMarginFundsResponse,
                UpdateUserLeverageRequest, UpdateUserMarginModeRequest,
            },
        },
        wallet::amount_fits_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool};
use uuid::Uuid;
/// 在现货与杠杆账户间划转同一资产；账户方向、正金额和资产精度必须先通过校验。
/// 事务先插入划转记录占用用户幂等键，再由资金层统一按“现货钱包→杠杆钱包”顺序加锁。
/// 两侧余额一减一增并各写对应流水，必须与划转记录原子提交，禁止出现单边到账或审计缺口。
/// 同键同请求重放原划转编号及余额快照且不再动账；同键异参冲突，提交后无外部副作用。
pub(crate) async fn transfer_margin_funds(
    pool: &Pool<MySql>,
    user_id: u64,
    request: TransferMarginFundsRequest,
) -> AppResult<TransferMarginFundsResponse> {
    let TransferMarginFundsRequest {
        asset_id,
        asset_symbol,
        from,
        to,
        amount,
        idempotency_key,
    } = request;
    validate_positive_decimal(&amount, "transfer amount")?;
    let from = normalized_margin_account(&from)?;
    let to = normalized_margin_account(&to)?;
    if from == to {
        return Err(AppError::Validation(
            "margin transfer source and target must be different".to_owned(),
        ));
    }
    let idempotency_key = normalize_transfer_idempotency_key(idempotency_key)?;
    if let Some(response) = replay_margin_transfer_if_present(
        pool,
        user_id,
        asset_id,
        asset_symbol.as_deref(),
        &from,
        &to,
        &amount,
        &idempotency_key,
    )
    .await?
    {
        return Ok(response);
    }
    let transfer_id = Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;
    let asset = resolve_active_transfer_asset(&mut tx, asset_id, asset_symbol.as_deref()).await?;
    if !amount_fits_asset_precision(&amount, asset.precision_scale) {
        return Err(AppError::Validation(format!(
            "margin transfer amount supports at most {} decimal places for asset {}",
            asset.precision_scale, asset.id
        )));
    }
    // 先占用用户幂等键，再触碰两侧钱包；任一后续步骤失败时同事务整体回滚。
    match insert_margin_transfer(
        &mut tx,
        &transfer_id,
        user_id,
        asset.id,
        &from,
        &to,
        &amount,
        &idempotency_key,
    )
    .await
    {
        Ok(()) => {}
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            if let Some(response) = replay_margin_transfer_if_present(
                pool,
                user_id,
                asset_id,
                asset_symbol.as_deref(),
                &from,
                &to,
                &amount,
                &idempotency_key,
            )
            .await?
            {
                return Ok(response);
            }
            return Err(AppError::Database(error));
        }
        Err(error) => return Err(AppError::Database(error)),
    }
    // 现货账户和杠杆账户的余额变化、两边流水必须同事务提交，避免出现单边扣款或审计缺口。
    let (spot_wallet, margin_wallet) = match (from.as_str(), to.as_str()) {
        ("spot", "margin") => {
            transfer_spot_to_margin_wallets(&mut tx, user_id, asset.id, &amount, &transfer_id)
                .await?
        }
        ("margin", "spot") => {
            transfer_margin_to_spot_wallets(&mut tx, user_id, asset.id, &amount, &transfer_id)
                .await?
        }
        _ => {
            return Err(AppError::Validation(
                "margin transfer only supports spot and margin accounts".to_owned(),
            ));
        }
    };
    tx.commit().await?;
    Ok(TransferMarginFundsResponse {
        transfer_id,
        spot_wallet,
        margin_wallet,
    })
}

#[allow(clippy::too_many_arguments)]
async fn replay_margin_transfer_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    request_asset_id: Option<u64>,
    request_asset_symbol: Option<&str>,
    from: &str,
    to: &str,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<TransferMarginFundsResponse>> {
    let Some(existing) =
        load_margin_transfer_by_idempotency_key(pool, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    let requested_asset_id =
        resolve_transfer_asset_id_for_replay(pool, request_asset_id, request_asset_symbol).await?;
    if existing.asset_id != requested_asset_id
        || existing.from_account != from
        || existing.to_account != to
        || existing.amount != *amount
    {
        return Err(AppError::Conflict(
            "margin transfer idempotency_key was already used with different parameters".to_owned(),
        ));
    }
    let mut tx = pool.begin().await?;
    let (spot_wallet, margin_wallet) = load_margin_transfer_wallet_snapshots(
        &mut tx,
        user_id,
        existing.asset_id,
        &existing.transfer_id,
    )
    .await?;
    tx.commit().await?;
    Ok(Some(TransferMarginFundsResponse {
        transfer_id: existing.transfer_id,
        spot_wallet,
        margin_wallet,
    }))
}

/// 在事务内锁定启用产品，验证杠杆属于配置档位后保存用户设置并回读结果。
/// 产品锁定、设置写入和回读同事务提交；失败回滚且不涉及钱包余额或外部事件。
pub(crate) async fn update_user_leverage(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    request: UpdateUserLeverageRequest,
) -> AppResult<MarginUserSettingResponse> {
    validate_positive_decimal(&request.leverage, "leverage")?;
    let mut tx = pool.begin().await?;
    let product = lock_active_product_setting_rule(&mut tx, product_id).await?;
    validate_product_leverage(&request.leverage, &product)?;
    upsert_user_margin_setting(&mut tx, user_id, product_id, None, Some(&request.leverage)).await?;
    let setting = load_user_margin_setting(&mut tx, user_id, product_id).await?;
    tx.commit().await?;
    Ok(setting)
}

/// 读取用户在指定保证金产品上的杠杆与模式设置；该只读路径不持有行锁或修改资金。
pub(crate) async fn get_user_margin_setting(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
) -> AppResult<MarginUserSettingResponse> {
    load_user_margin_setting_from_pool(pool, user_id, product_id).await
}

/// 在事务内锁定启用产品，确认目标模式受产品支持后保存用户保证金模式。
/// 设置写入与回读同事务提交；非法模式或数据库失败不改变钱包、仓位和事件状态。
pub(crate) async fn update_user_margin_mode(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    request: UpdateUserMarginModeRequest,
) -> AppResult<MarginUserSettingResponse> {
    let mut tx = pool.begin().await?;
    let product = lock_active_product_setting_rule(&mut tx, product_id).await?;
    let mode = selected_margin_mode(&product, Some(&request.margin_mode))?;
    upsert_user_margin_setting(&mut tx, user_id, product_id, Some(&mode), None).await?;
    let setting = load_user_margin_setting(&mut tx, user_id, product_id).await?;
    tx.commit().await?;
    Ok(setting)
}
fn validate_product_leverage(
    leverage: &BigDecimal,
    product: &MarginProductSettingRule,
) -> AppResult<()> {
    if !product
        .leverage_levels
        .0
        .iter()
        .any(|level| decimal_matches_string(leverage, level))
    {
        return Err(AppError::Validation(
            "margin leverage must match a configured product level".to_owned(),
        ));
    }
    Ok(())
}

fn selected_margin_mode(
    product: &MarginProductSettingRule,
    requested_mode: Option<&str>,
) -> AppResult<String> {
    let mode = match requested_mode {
        Some(value) => normalized_margin_mode(value)?,
        None => product.margin_mode.clone(),
    };
    if !product
        .margin_modes
        .0
        .iter()
        .any(|supported| supported == &mode)
    {
        return Err(AppError::Validation(
            "margin_mode is not supported by this margin product".to_owned(),
        ));
    }
    ensure_supported_user_margin_mode(&mode)?;
    Ok(mode)
}

fn normalized_margin_account(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "spot" => Ok("spot".to_owned()),
        "swap" | "margin" => Ok("margin".to_owned()),
        _ => Err(AppError::Validation(
            "margin transfer account must be spot or margin".to_owned(),
        )),
    }
}

fn normalize_transfer_idempotency_key(value: Option<String>) -> AppResult<String> {
    let Some(value) = value else {
        return Ok(Uuid::now_v7().to_string());
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(
            "margin transfer idempotency_key must not be empty".to_owned(),
        ));
    }
    if value.chars().count() > 128 {
        return Err(AppError::Validation(
            "margin transfer idempotency_key must not exceed 128 characters".to_owned(),
        ));
    }
    Ok(value.to_owned())
}
