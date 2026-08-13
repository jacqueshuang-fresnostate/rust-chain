//! 杠杆账户设置与资金划转用例。
//!
//! 包含两类职责：现货钱包与杠杆钱包之间的同资产互转，以及用户在单个产品上的杠杆倍数和保证金模式设置。
//! 划转是本文件唯一动余额的路径，遵循先落划转记录占用幂等键、再由资金层按固定锁序动两侧钱包的顺序，
//! 双向划转都统一先锁现货再锁杠杆，靠稳定锁序而不是加锁时机来避免反向请求交叉等待形成死锁。
//! 设置类用例只写 `margin_user_settings`，会先锁定 active 产品固定校验依据，不涉及任何余额或事件。
//! 两类用例都不发布 WebSocket 事件，划转结果通过响应里的两侧余额快照直接返回给调用方。

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
///
/// 账户名做了兼容处理：`swap` 与 `margin` 都归一为杠杆账户，同名互转直接判为参数非法。
/// 幂等键可以省略，省略时服务端用 UUIDv7 生成一个，这种请求天然不具备重放能力。
/// 资产可用 `asset_id` 或 `asset_symbol` 任一方式指定，解析时要求资产处于 active，
/// 金额还必须满足该资产自身的精度上限，超出小数位在开始动账之前就被拒绝。
/// 幂等重放走独立的只读核对路径，从原划转两侧流水的 after 快照重建响应，
/// 因此返回的是当时的余额而不是当前余额，不会泄漏后续交易造成的变化。
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

/// 按用户和幂等键查既有划转，命中则核对资产、方向和金额后重建原响应，未命中返回 None。
/// 核对用的资产标识走的是不限状态的解析，因此资产事后被停用也能正常重放，只是不能再发起新划转。
/// 四项中任意一项不同即返回 Conflict，杜绝同一个键被复用到另一笔资金意图上。
/// 余额快照取自原划转两侧流水的 after 字段而非钱包当前值，保证重放结果与首次响应完全一致。
/// 本函数只读不动账；开事务仅为在同一视图内读两张流水表，读完即提交，没有资金副作用。
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

/// 读取用户在指定杠杆产品上已保存的保证金模式与倍数，直接走连接池不开事务、不加行锁。
/// 用户从未在该产品上设置过时返回 NotFound，调用方应据此回落到产品自身的默认模式和档位。
/// 两个字段在表里各自可空，因为只改倍数或只改模式的请求不会覆盖对方未提供的值。
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
/// 要求用户设置的默认杠杆精确命中产品的某个配置档位，与开仓时的档位校验口径完全一致。
/// 逐档把存储字符串解析回十进制精确比较，解析失败的档位跳过，一档不中即返回参数错误。
/// 这里用的是设置路径锁定的产品规则快照，因此并发改配不会让校验依据在中途变化。
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

/// 确定要落库的用户保证金模式：请求给出则归一化后使用，缺省时取产品默认模式。
/// 选中的模式必须同时在产品支持列表内且被后端风控实现，否则返回参数错误不写设置。
/// 与开仓路径的同名判定逻辑一致，区别在于这里用的是设置事务锁定的产品规则，且结果只写用户设置表。
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

/// 归一化划转的来源或目标账户名，裁剪空白并折叠大小写后只认现货和杠杆两侧。
/// 历史客户端把杠杆账户称作 `swap`，这里与 `margin` 一并映射为 `margin` 以保持向后兼容。
/// 归一化后的值既用于选择资金搬运方向，也参与幂等重放的方向比对，因此两次请求写法不同但语义相同时仍能命中。
fn normalized_margin_account(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "spot" => Ok("spot".to_owned()),
        "swap" | "margin" => Ok("margin".to_owned()),
        _ => Err(AppError::Validation(
            "margin transfer account must be spot or margin".to_owned(),
        )),
    }
}

/// 归一化划转幂等键：完全不传时由服务端生成 UUIDv7，传了则必须非空且不超过一百二十八个字符。
/// 长度按 Unicode 字符数统计，与开仓幂等键按字节数限制两百五十五的口径不同，两者上限互不通用。
/// 注意「不传」和「传空串」被区别对待：前者放行并自动补键，后者判为参数非法，
/// 这样能挡住客户端把未初始化的空字符串当键提交、导致同一个键被多笔不同划转共用的情况。
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
