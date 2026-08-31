//! 杠杆账户设置与资金划转用例。
//!
//! 包含两类职责：现货钱包与杠杆钱包之间的同资产互转，以及用户在单个产品上的杠杆倍数和保证金模式设置。
//! 划转是本文件唯一动余额的路径，遵循先落划转记录占用幂等键、再由资金层按固定锁序动两侧钱包的顺序，
//! 双向划转都统一先锁现货再锁杠杆，靠稳定锁序而不是加锁时机来避免反向请求交叉等待形成死锁。
//! 设置类用例只写 `margin_user_settings`，会先锁定 active 产品固定校验依据，不涉及任何余额或事件。
//! 两类用例都不发布 WebSocket 事件，划转结果通过响应里的两侧余额快照直接返回给调用方。

use super::support::{
    decimal_matches_string, ensure_supported_user_margin_mode, is_duplicate_key_error,
    margin_transfer_request_fingerprint, normalized_margin_mode, validate_positive_decimal,
};
use crate::{
    error::{AppError, AppResult},
    modules::{
        margin::{
            domain::{
                MarkedCrossMarginPosition, cross_margin_max_transferable,
                evaluate_marked_cross_margin,
            },
            infrastructure::{
                MarginProductSettingRule, MarginRiskPositionRow, MarginRiskTicker,
                apply_margin_to_spot_transfer, apply_spot_to_margin_transfer,
                cached_margin_risk_ticker, ensure_and_lock_cross_margin_account,
                insert_margin_transfer, list_user_cross_margin_risk_positions,
                load_cross_margin_account, load_margin_transfer_by_idempotency_key,
                load_margin_transfer_wallet_snapshots, load_user_margin_setting,
                load_user_margin_setting_from_pool, lock_active_product_setting_rule,
                lock_cross_margin_risk_positions, lock_margin_transfer_wallets,
                require_active_cross_margin_account, resolve_active_transfer_asset,
                resolve_transfer_asset_id_for_replay, update_locked_cross_margin_risk,
                upsert_user_margin_setting,
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
use chrono::Utc;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
use std::collections::BTreeMap;
use uuid::Uuid;
/// 在现货与杠杆账户间划转同一资产；账户方向、正金额和资产精度必须先通过校验。
/// 事务先插入划转记录占用用户幂等键，再由资金层统一按“现货钱包→杠杆钱包”顺序加锁。
/// 两侧余额一减一增并各写对应流水，必须与划转记录原子提交，禁止出现单边到账或审计缺口。
/// 同键同请求重放原划转编号及余额快照且不再动账；同键异参冲突，提交后无外部副作用。
///
/// 账户名做了兼容处理：`swap` 与 `margin` 都归一为杠杆账户，同名互转直接判为参数非法。
/// 幂等键必须由客户端提供，空白或超界在解析资产和动账前拒绝。
/// 资产可用 `asset_id` 或 `asset_symbol` 任一方式指定，解析时要求资产处于 active，
/// 新的现货转杠杆还要求资产开启 `margin_transfer_enabled`；关闭开关只阻止新增转入，
/// 不影响已有杠杆余额转回现货，也不影响在开关变化前已经成功请求的幂等重放。
/// 金额还必须满足该资产自身的精度上限，超出小数位在开始动账之前就被拒绝。
/// 幂等重放走独立的只读核对路径，从原划转两侧流水的 after 快照重建响应，
/// 因此返回的是当时的余额而不是当前余额，不会泄漏后续交易造成的变化。
pub(crate) async fn transfer_margin_funds(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
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
    let risk_prefetch = if from == "margin" && to == "spot" {
        let mut asset_tx = pool.begin().await?;
        let asset =
            resolve_active_transfer_asset(&mut asset_tx, asset_id, asset_symbol.as_deref()).await?;
        if !amount_fits_asset_precision(&amount, asset.precision_scale) {
            return Err(AppError::Validation(format!(
                "margin transfer amount supports at most {} decimal places for asset {}",
                asset.precision_scale, asset.id
            )));
        }
        asset_tx.commit().await?;
        Some(prefetch_cross_transfer_risk(pool, redis, user_id, asset.id).await?)
    } else {
        None
    };
    let transfer_id = Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;
    let asset = resolve_active_transfer_asset(&mut tx, asset_id, asset_symbol.as_deref()).await?;
    if from == "spot" && to == "margin" && !asset.margin_transfer_enabled {
        return Err(AppError::Validation(
            "asset is not enabled for transfer into margin account".to_owned(),
        ));
    }
    if !amount_fits_asset_precision(&amount, asset.precision_scale) {
        return Err(AppError::Validation(format!(
            "margin transfer amount supports at most {} decimal places for asset {}",
            asset.precision_scale, asset.id
        )));
    }
    let request_fingerprint =
        margin_transfer_request_fingerprint(user_id, asset.id, &from, &to, &amount);
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
        &request_fingerprint,
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
            let wallets = lock_margin_transfer_wallets(&mut tx, user_id, asset.id).await?;
            apply_spot_to_margin_transfer(
                &mut tx,
                user_id,
                asset.id,
                &amount,
                &transfer_id,
                wallets,
            )
            .await?
        }
        ("margin", "spot") => {
            let prefetch = risk_prefetch.as_ref().ok_or_else(|| {
                AppError::Internal("cross transfer risk prefetch is required".to_owned())
            })?;
            if prefetch.margin_asset != asset.id {
                return Err(AppError::Conflict(
                    "margin transfer asset changed during risk prefetch".to_owned(),
                ));
            }
            let account = ensure_and_lock_cross_margin_account(&mut tx, user_id, asset.id).await?;
            require_active_cross_margin_account(&account)?;
            if prefetch.account_version.unwrap_or(0) != account.version {
                return Err(AppError::Conflict(
                    "cross margin account changed during transfer risk prefetch".to_owned(),
                ));
            }
            let positions = lock_cross_margin_risk_positions(&mut tx, user_id, asset.id).await?;
            if !same_cross_risk_positions(&prefetch.positions, &positions) {
                return Err(AppError::Conflict(
                    "cross margin positions changed during transfer risk prefetch".to_owned(),
                ));
            }
            ensure_transfer_marks_fresh(&prefetch.tickers)?;
            let wallets = lock_margin_transfer_wallets(&mut tx, user_id, asset.id).await?;
            // 双钱包锁可能被反向划转长时间占用；取得全部资金锁后必须再校验一次价格时间。
            ensure_transfer_marks_fresh(&prefetch.tickers)?;
            let before = evaluate_cross_transfer_risk(
                wallets.margin_available(),
                &positions,
                &prefetch.tickers,
            )?;
            let max_transferable =
                cross_margin_max_transferable(wallets.margin_available(), &before)
                    .map_err(|message| AppError::Validation(message.to_owned()))?;
            if amount > max_transferable {
                return Err(AppError::Validation(format!(
                    "cross margin transfer exceeds risk maximum: requested {amount}, max_transferable_to_spot {max_transferable}, reason maintenance_margin"
                )));
            }
            let available_after =
                (wallets.margin_available().clone() - amount.clone()).with_scale(18);
            let after =
                evaluate_cross_transfer_risk(&available_after, &positions, &prefetch.tickers)?;
            if after.equity < after.maintenance_margin {
                return Err(AppError::Validation(
                    "cross margin transfer would fall below maintenance margin".to_owned(),
                ));
            }
            let snapshots = apply_margin_to_spot_transfer(
                &mut tx,
                user_id,
                asset.id,
                &amount,
                &transfer_id,
                wallets,
            )
            .await?;
            let observed_at = prefetch
                .tickers
                .values()
                .map(|ticker| ticker.observed_at)
                .min()
                .unwrap_or_else(Utc::now);
            update_locked_cross_margin_risk(
                &mut tx,
                user_id,
                asset.id,
                account.version,
                &after,
                observed_at,
            )
            .await?;
            snapshots
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

struct CrossTransferRiskPrefetch {
    margin_asset: u64,
    account_version: Option<u64>,
    positions: Vec<MarginRiskPositionRow>,
    tickers: BTreeMap<u64, MarginRiskTicker>,
}

/// 在取得写锁前按唯一 pair 预取一批新鲜标记价；事务内会再次核对账户版本、仓位集合和价格年龄。
async fn prefetch_cross_transfer_risk(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<CrossTransferRiskPrefetch> {
    let account = load_cross_margin_account(pool, user_id, margin_asset).await?;
    if let Some(account) = account.as_ref() {
        require_active_cross_margin_account(account)?;
    }
    let positions = list_user_cross_margin_risk_positions(pool, user_id, margin_asset).await?;
    let mut symbols = BTreeMap::<u64, String>::new();
    for position in &positions {
        if let Some(existing) = symbols.insert(position.pair_id, position.symbol.clone())
            && existing != position.symbol
        {
            return Err(AppError::Internal(format!(
                "pair {} has inconsistent symbols in cross transfer risk rows",
                position.pair_id
            )));
        }
    }
    let mut tickers = BTreeMap::new();
    for (pair_id, symbol) in symbols {
        tickers.insert(
            pair_id,
            cached_margin_risk_ticker(redis, pair_id, &symbol).await?,
        );
    }
    Ok(CrossTransferRiskPrefetch {
        margin_asset,
        account_version: account.map(|account| account.version),
        positions,
        tickers,
    })
}

fn same_cross_risk_positions(
    expected: &[MarginRiskPositionRow],
    locked: &[MarginRiskPositionRow],
) -> bool {
    expected.len() == locked.len()
        && expected.iter().all(|left| {
            locked.iter().any(|right| {
                left.id == right.id
                    && left.pair_id == right.pair_id
                    && left.symbol == right.symbol
                    && left.direction == right.direction
                    && left.margin_amount == right.margin_amount
                    && left.notional_amount == right.notional_amount
                    && left.interest_amount == right.interest_amount
                    && left.entry_price == right.entry_price
                    && left.maintenance_margin_rate == right.maintenance_margin_rate
            })
        })
}

fn ensure_transfer_marks_fresh(tickers: &BTreeMap<u64, MarginRiskTicker>) -> AppResult<()> {
    let now = Utc::now();
    if let Some((pair_id, _)) = tickers.iter().find(|(_, ticker)| ticker.observed_at > now) {
        return Err(AppError::Validation(format!(
            "margin transfer risk ticker is from the future for pair {pair_id}"
        )));
    }
    let cutoff = now - chrono::TimeDelta::seconds(60);
    if let Some((pair_id, _)) = tickers
        .iter()
        .find(|(_, ticker)| ticker.observed_at < cutoff)
    {
        return Err(AppError::Validation(format!(
            "margin transfer risk ticker is stale for pair {pair_id}"
        )));
    }
    Ok(())
}

fn evaluate_cross_transfer_risk(
    wallet_available: &BigDecimal,
    positions: &[MarginRiskPositionRow],
    tickers: &BTreeMap<u64, MarginRiskTicker>,
) -> AppResult<crate::modules::margin::domain::CrossMarginRiskState> {
    let mut marked = Vec::with_capacity(positions.len());
    for position in positions {
        let entry_price = position.entry_price.as_ref().ok_or_else(|| {
            AppError::Internal("cross transfer risk position is missing entry price".to_owned())
        })?;
        let ticker = tickers.get(&position.pair_id).ok_or_else(|| {
            AppError::Validation(format!(
                "margin transfer risk price is unavailable for pair {}",
                position.pair_id
            ))
        })?;
        marked.push(MarkedCrossMarginPosition {
            direction: &position.direction,
            margin_amount: &position.margin_amount,
            notional_amount: &position.notional_amount,
            interest_amount: &position.interest_amount,
            entry_price,
            mark_price: &ticker.last_price,
            maintenance_margin_rate: &position.maintenance_margin_rate,
        });
    }
    evaluate_marked_cross_margin(wallet_available, &marked)
        .map(|evaluated| evaluated.account)
        .map_err(|message| AppError::Validation(message.to_owned()))
}

/// 钱包读模型使用的权威转出能力快照；真正提交仍会在事务内重算并校验版本。
pub(super) struct CrossTransferCapacity {
    pub(super) max_transferable: BigDecimal,
    pub(super) block_reason: Option<String>,
    pub(super) account_version: Option<u64>,
    pub(super) equity: Option<BigDecimal>,
    pub(super) maintenance_margin: Option<BigDecimal>,
    pub(super) observed_at: Option<chrono::DateTime<Utc>>,
}

/// 计算全仓账户当前可安全转回现货的钱包额度，并生成供查询接口展示的风险快照。
///
/// 本方法先读取账户状态和所有全仓持仓，再用同一批经过时效校验的标记价计算权益、维持保证金与
/// 安全缓冲；行情缺失、过期或账户正在强平时按 fail-closed 返回零额度。这里的结果只用于读模型，
/// 真正划转仍会在数据库事务取得账户、持仓与钱包锁后重新校验行情及账户版本，避免查询结果被当成授权。
pub(super) async fn calculate_cross_transfer_capacity(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    margin_asset: u64,
    available: &BigDecimal,
) -> AppResult<CrossTransferCapacity> {
    let account = load_cross_margin_account(pool, user_id, margin_asset).await?;
    if let Some(account) = account.as_ref()
        && account.status != "active"
    {
        return Ok(CrossTransferCapacity {
            max_transferable: BigDecimal::from(0).with_scale(18),
            block_reason: Some(format!("account_{}", account.status)),
            account_version: Some(account.version),
            equity: None,
            maintenance_margin: None,
            observed_at: None,
        });
    }
    let prefetch = match prefetch_cross_transfer_risk(pool, redis, user_id, margin_asset).await {
        Ok(prefetch) => prefetch,
        Err(AppError::Database(error)) => return Err(AppError::Database(error)),
        Err(error) => {
            tracing::warn!(user_id, margin_asset, error = %error, "全仓最大转出额因行情不可用而关闭");
            return Ok(CrossTransferCapacity {
                max_transferable: BigDecimal::from(0).with_scale(18),
                block_reason: Some("price_unavailable".to_owned()),
                account_version: account.as_ref().map(|account| account.version),
                equity: None,
                maintenance_margin: None,
                observed_at: None,
            });
        }
    };
    if prefetch.positions.is_empty() {
        return Ok(CrossTransferCapacity {
            max_transferable: available.clone().with_scale(18),
            block_reason: None,
            account_version: prefetch.account_version,
            equity: Some(available.clone().with_scale(18)),
            maintenance_margin: Some(BigDecimal::from(0).with_scale(18)),
            observed_at: None,
        });
    }
    let risk = match evaluate_cross_transfer_risk(available, &prefetch.positions, &prefetch.tickers)
    {
        Ok(risk) => risk,
        Err(error) => {
            tracing::warn!(user_id, margin_asset, error = %error, "全仓最大转出额因风险数据异常而关闭");
            return Ok(CrossTransferCapacity {
                max_transferable: BigDecimal::from(0).with_scale(18),
                block_reason: Some("risk_unavailable".to_owned()),
                account_version: prefetch.account_version,
                equity: None,
                maintenance_margin: None,
                observed_at: None,
            });
        }
    };
    let max_transferable = cross_margin_max_transferable(available, &risk)
        .map_err(|message| AppError::Validation(message.to_owned()))?;
    let block_reason = (max_transferable <= 0).then(|| "maintenance_margin".to_owned());
    Ok(CrossTransferCapacity {
        max_transferable,
        block_reason,
        account_version: prefetch.account_version,
        equity: Some(risk.equity),
        maintenance_margin: Some(risk.maintenance_margin),
        observed_at: prefetch
            .tickers
            .values()
            .map(|ticker| ticker.observed_at)
            .min(),
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
    let request_fingerprint =
        margin_transfer_request_fingerprint(user_id, requested_asset_id, from, to, amount);
    let matches = existing.request_fingerprint.as_deref().map_or_else(
        || {
            existing.asset_id == requested_asset_id
                && existing.from_account == from
                && existing.to_account == to
                && existing.amount == *amount
        },
        |existing_fingerprint| existing_fingerprint == request_fingerprint,
    );
    if !matches {
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

/// 解析旧单倍数或新双方向载荷，在开事务前拒绝混合、部分、null 或非正数形状。
/// 事务内锁定启用产品，要求做多与做空倍数分别精确命中同一份配置档位。
/// 三列在单次 UPSERT 中原子写入，其中兼容 `leverage` 始终取做多值；任一校验失败都不产生设置写入。
pub(crate) async fn update_user_leverage(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    request: UpdateUserLeverageRequest,
) -> AppResult<MarginUserSettingResponse> {
    let (long_leverage, short_leverage) = validated_requested_leverages(request)?;
    let mut tx = pool.begin().await?;
    let product = lock_active_product_setting_rule(&mut tx, product_id).await?;
    validate_product_leverage(&long_leverage, &product)?;
    validate_product_leverage(&short_leverage, &product)?;
    upsert_user_margin_setting(
        &mut tx,
        user_id,
        product_id,
        None,
        Some((&long_leverage, &short_leverage)),
    )
    .await?;
    let setting = load_user_margin_setting(&mut tx, user_id, product_id).await?;
    tx.commit().await?;
    Ok(setting)
}

/// 读取用户在指定杠杆产品上已保存的保证金模式与双方向倍数，直接走连接池不开事务、不加行锁。
/// 用户从未在该产品上设置过时返回 NotFound，调用方应据此回落到产品自身的默认模式和档位。
/// 模式和倍数组各自可空，因为只改倍数或只改模式的请求不会覆盖另一维度。
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

/// 把互斥的 HTTP 载荷归一成“做多、做空”两个完整值，并在任何数据库连接或写入前完成形状与正数校验。
/// 旧格式把单值复制给两个方向；新格式必须同时给出两个非 null 值。
/// 双层 Option 保留了显式 null 的存在性，所以 null 不会被误当成缺省字段而绕过混合格式检查。
fn validated_requested_leverages(
    request: UpdateUserLeverageRequest,
) -> AppResult<(BigDecimal, BigDecimal)> {
    match (
        request.leverage,
        request.long_leverage,
        request.short_leverage,
    ) {
        (Some(Some(leverage)), None, None) => {
            validate_positive_decimal(&leverage, "leverage")?;
            Ok((leverage.clone(), leverage))
        }
        (None, Some(Some(long_leverage)), Some(Some(short_leverage))) => {
            validate_positive_decimal(&long_leverage, "long_leverage")?;
            validate_positive_decimal(&short_leverage, "short_leverage")?;
            Ok((long_leverage, short_leverage))
        }
        _ => Err(AppError::Validation(
            "provide either leverage or both long_leverage and short_leverage".to_owned(),
        )),
    }
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

/// 归一化客户端划转幂等键：裁剪后必须非空且不超过 128 字节。
fn normalize_transfer_idempotency_key(value: String) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(
            "margin transfer idempotency_key must not be empty".to_owned(),
        ));
    }
    if value.len() > 128 {
        return Err(AppError::Validation(
            "margin transfer idempotency_key must not exceed 128 bytes".to_owned(),
        ));
    }
    Ok(value.to_owned())
}
