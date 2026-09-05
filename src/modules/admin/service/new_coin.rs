//! 新币项目生命周期、解锁规则与派发锁仓计算的纯业务规则层。
//!
//! 三条主线：一是生命周期代码在请求、数据库与领域枚举之间的双向解析，其中请求侧未知值算校验错误、
//! 数据库侧未知值算内部数据错误；二是解锁规则与解锁手续费规则的字段形状校验，
//! 两者都采用「按类型确定必填项并禁止其他类型专属字段」的互斥策略；三是把项目解锁规则翻译成领域规则后，
//! 借新币领域的解锁计算得出待写入的锁仓头寸。全部函数不触库、不写钱包、不加锁，
//! 项目当前状态与可派额度等依赖持久化的判定一律由 application 层在持锁后完成。

use super::*;

/// 校验新币派发请求的审计原因以及可选幂等键长度，拒绝空白或超长标识。
/// 赠币数量必须为正；关联申购允许零数量全额退款，原因必填，最终额度由应用事务锁行后确认。
pub(crate) fn validate_distribute_new_coin(request: &DistributeNewCoinRequest) -> AppResult<()> {
    if request.quantity < 0 || (request.quantity == 0 && request.subscription_id.is_none()) {
        return Err(AppError::Validation("quantity must be positive".to_owned()));
    }
    required_admin_audit_reason(request.reason.clone())?;
    if optional_string(Some(request.idempotency_key.clone())).is_none() {
        return Err(AppError::Validation(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    if request.idempotency_key.len() > 128 {
        return Err(AppError::Validation(
            "idempotency_key must not exceed 128 bytes".to_owned(),
        ));
    }
    Ok(())
}

/// 校验新币解锁类型、起始时间、周期和比例组合，防止生成无法执行的锁仓计划。
/// 这里只验证规则形状；已分配仓位如何迁移由更新事务根据项目状态决定。
pub(crate) fn validate_update_new_coin_unlock_rule(
    request: &UpdateNewCoinUnlockRuleRequest,
) -> AppResult<()> {
    validate_unlock_rule_shape(
        &request.unlock_type,
        request.listed_at,
        request.fixed_unlock_at,
        request.relative_unlock_seconds,
    )
}

/// 校验解锁费启用状态、计费基准、费率与固定金额组合，保证启用规则可确定唯一收费口径。
/// 不在此处计算用户费用或修改锁仓；实际收费按项目规则快照在解锁事务中执行。
pub(crate) fn validate_update_new_coin_unlock_fee_rule(
    request: &UpdateNewCoinUnlockFeeRuleRequest,
) -> AppResult<()> {
    validate_unlock_fee_rule_shape(
        request.unlock_fee_enabled,
        request.unlock_fee_rate.as_ref(),
        request.unlock_fee_basis.clone(),
        request.unlock_fee_asset,
    )
}

/// 校验上市后购买开关与交易对绑定关系：启用时必须提供有效的正数 pair_id。
/// 交易对是否 active 以及是否属于该项目由应用事务查询确认。
pub(crate) fn validate_update_new_coin_post_listing_purchase(
    request: &UpdateNewCoinPostListingPurchaseRequest,
) -> AppResult<()> {
    if request.enabled && request.pair_id.unwrap_or(0) == 0 {
        return Err(AppError::Validation(
            "pair_id is required when post-listing purchase is enabled".to_owned(),
        ));
    }
    Ok(())
}

/// 校验新币项目初始生命周期、资产、认购窗口、价格、额度及解锁规则的完整组合。
/// 资产唯一性和并发创建冲突由数据库约束处理；本函数不写项目、钱包或审计。
/// 初始生命周期可以是四态中的任意一个而不强制从预热开始，因此允许直接创建已上市的历史项目。
/// 总量和发行价都要求严格为正，禁止零价配置绕过真实计价；符号去空后不得为空。
/// 解锁规则必填，解锁手续费规则在未显式启用时按关闭处理，两者都按各自的互斥字段形状校验。
pub(crate) fn validate_create_new_coin_project(
    request: &CreateNewCoinProjectRequest,
) -> AppResult<()> {
    let Some(lifecycle_status) = optional_string(Some(request.lifecycle_status.clone())) else {
        return Err(AppError::Validation(
            "lifecycle_status is required".to_owned(),
        ));
    };
    parse_lifecycle_status_from_request(&lifecycle_status)?;
    if request.total_supply <= 0 {
        return Err(AppError::Validation(
            "total_supply must be positive".to_owned(),
        ));
    }
    if request.issue_price <= 0 {
        return Err(AppError::Validation(
            "issue_price must be positive".to_owned(),
        ));
    }
    if request.quote_asset_id == 0 || request.quote_asset_id == request.asset_id {
        return Err(AppError::Validation(
            "quote_asset_id must identify a different active asset".to_owned(),
        ));
    }
    if optional_string(Some(request.symbol.clone())).is_none() {
        return Err(AppError::Validation("symbol is required".to_owned()));
    }
    validate_unlock_rule_shape(
        &request.unlock_type,
        request.listed_at,
        request.fixed_unlock_at,
        request.relative_unlock_seconds,
    )?;
    validate_unlock_fee_rule_shape(
        request.unlock_fee_enabled.unwrap_or(false),
        request.unlock_fee_rate.as_ref(),
        request.unlock_fee_basis.clone(),
        request.unlock_fee_asset,
    )?;
    crate::modules::new_coin::service::ensure_unlock_fee_asset_matches_quote_asset(
        request.unlock_fee_enabled.unwrap_or(false),
        request.unlock_fee_asset,
        Some(request.quote_asset_id),
    )?;

    Ok(())
}

/// 校验新币闪兑规则的汇率来源、固定汇率、费率、限额和启停状态组合。
/// 这里只确定配置是否自洽；目标资产与项目关联、报价和钱包结算由应用/闪兑上下文负责。
/// 当前只支持 fixed 一种汇率来源，浮动配置虽在数据结构中保留但会被本校验拒绝，因此固定汇率必填且必须为正。
/// 状态为可选项：完全不传表示沿用默认启用，但传了却是纯空白会被判为非法，以区分「未指定」与「填错」。
pub(crate) fn validate_new_coin_convert_rule(
    request: &UpsertNewCoinConvertRuleRequest,
) -> AppResult<()> {
    let Some(rate_source) = optional_string(Some(request.rate_source.clone())) else {
        return Err(AppError::Validation("rate_source is required".to_owned()));
    };
    if rate_source != "fixed" {
        return Err(AppError::Validation(
            "only fixed rate_source is supported for new coin convert rules".to_owned(),
        ));
    }
    if request.fixed_rate.is_none() {
        return Err(AppError::Validation(
            "fixed_rate is required for fixed rate_source".to_owned(),
        ));
    }
    if let Some(fixed_rate) = &request.fixed_rate
        && fixed_rate <= 0
    {
        return Err(AppError::Validation(
            "fixed_rate must be positive".to_owned(),
        ));
    }
    if optional_string(request.status.clone()).is_none() && request.status.is_some() {
        return Err(AppError::Validation("status is required".to_owned()));
    }

    Ok(())
}

/// 确认新币项目已完成派发并进入可上市阶段，才允许启停上市后购买功能。
/// 仅接受持久化生命周期为 listed 的项目快照；其他合法状态返回校验错误，未知数据库状态转换为内部错误，调用方负责在持锁后调用。
pub(crate) fn ensure_post_listing_purchase_lifecycle(
    project: &NewCoinProjectResponse,
) -> AppResult<()> {
    if parse_lifecycle_status_from_db(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing purchase can only be configured for listed projects".to_owned(),
        ));
    }
    Ok(())
}

/// 确认新币项目处于待派发状态，阻止对尚未结束认购或已经派发的项目重复分配资产。
/// 仅接受持久化生命周期为 distribution 的项目快照；其他合法状态返回校验错误，未知存储值返回内部错误，本函数本身不加锁或标记已派发。
pub(crate) fn ensure_distribution_lifecycle(project: &NewCoinProjectResponse) -> AppResult<()> {
    if parse_lifecycle_status_from_db(&project.lifecycle_status)? != LifecycleStatus::Distribution {
        return Err(AppError::Validation(
            "new coin project must be in distribution lifecycle before distribution".to_owned(),
        ));
    }
    Ok(())
}

/// 解析请求中的新币生命周期代码，空白或未支持值返回面向调用方的校验错误。
/// 结果只包含领域枚举，不检查当前项目状态或迁移方向，也不读取数据库。
pub(crate) fn parse_lifecycle_status_from_request(value: &str) -> AppResult<LifecycleStatus> {
    let Some(value) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "lifecycle_status is required".to_owned(),
        ));
    };
    parse_lifecycle_status(&value)
}

/// 解析数据库中的新币生命周期代码并返回领域枚举。
/// 已存未知值被视为内部数据错误并在消息中保留原代码；函数不修复记录或触发状态迁移。
pub(crate) fn parse_lifecycle_status_from_db(value: &str) -> AppResult<LifecycleStatus> {
    parse_lifecycle_status(value).map_err(|_| {
        AppError::Internal(format!(
            "stored new coin lifecycle_status is unsupported: {value}"
        ))
    })
}

/// 将新币生命周期领域枚举转换为数据库和接口共用的稳定状态代码。
/// 映射为穷尽纯函数，不访问持久化或执行状态迁移，调用方负责校验迁移是否合法。
pub(crate) fn lifecycle_status_value(status: LifecycleStatus) -> &'static str {
    match status {
        LifecycleStatus::Preheat => "preheat",
        LifecycleStatus::Subscription => "subscription",
        LifecycleStatus::Distribution => "distribution",
        LifecycleStatus::Listed => "listed",
    }
}

/// 依据新币派发的数量、解锁规则和来源时间计算锁仓头寸，保持各期金额之和等于待锁总额。
/// 计算不写钱包或锁仓表；非法规则或金额返回错误，实际幂等合并与事务提交由应用层负责。
/// 项目上的解锁字段会先被翻译成领域解锁规则，再交由新币领域统一计算，从而与用户侧解锁口径一致。
/// 派发的幂等键被当作解锁来源标识透传，领域据此生成合并键，使同一笔派发重放时能落到同一条锁仓上。
/// 返回空列表是合法结果，表示该规则下资产可立即到账而无需锁仓，调用方据此决定派发状态记为已完成还是锁定中。
pub(crate) fn lock_positions_for_distribution(
    project: &NewCoinProjectResponse,
    user_id: u64,
    asset_id: u64,
    source_id: &str,
    quantity: BigDecimal,
    source_time: DateTime<Utc>,
) -> AppResult<Vec<AdminNewCoinLockPositionWrite>> {
    let unlock_rule = unlock_rule_from_project(project)?;
    let application = apply_unlock_rule(
        &unlock_rule,
        vec![UnlockSource {
            user_id: user_id.to_string(),
            asset_id: asset_id.to_string(),
            source_id: source_id.to_owned(),
            amount: quantity,
            source_time,
        }],
    )
    .map_err(|error| AppError::Validation(format!("invalid new coin unlock rule: {error:?}")))?;

    Ok(application
        .lock_positions
        .into_iter()
        .map(|position| AdminNewCoinLockPositionWrite {
            listing_project_id: (project.unlock_type == "immediate_on_listing"
                && project.lifecycle_status != "listed")
                .then_some(project.id),
            user_id,
            asset_id,
            unlock_type: position.unlock_type,
            unlock_at: position.unlock_at,
            amount: position.remaining_amount,
            merge_key: position.merge_key,
            source_time,
            source_type: "new_coin_distribution".to_owned(),
            source_id: source_id.to_owned(),
        })
        .collect())
}

/// 将新币项目的发行、生命周期、解锁、手续费和上市后购买配置映射为审计快照。
/// 快照不包含认购或钱包明细；应用层在项目配置事务中保存前后值，时间统一为毫秒值。
/// 生命周期推进、解锁规则替换、手续费规则调整和上市后购买开关四类操作共用这一份结构，
/// 因此单看快照即可判断本次改动落在哪一组字段上。
/// 上市后购买同时记录交易对编号与其状态，可据此看出关闭开关时交易对是否仍处于启用。
pub(crate) fn new_coin_project_audit_json(project: &NewCoinProjectResponse) -> Value {
    json!({
        "id": project.id,
        "asset_id": project.asset_id,
        "symbol": project.symbol,
        "lifecycle_status": project.lifecycle_status,
        "total_supply": project.total_supply,
        "issue_price": project.issue_price,
        "quote_asset_id": project.quote_asset_id,
        "reserved_supply": project.reserved_supply,
        "allocated_supply": project.allocated_supply,
        "remaining_supply": project.remaining_supply,
        "listed_at": project.listed_at.map(|value| value.timestamp_millis()),
        "actual_listed_at": project.actual_listed_at.map(|value| value.timestamp_millis()),
        "unlock_type": project.unlock_type,
        "fixed_unlock_at": project.fixed_unlock_at.map(|value| value.timestamp_millis()),
        "relative_unlock_seconds": project.relative_unlock_seconds,
        "unlock_fee_enabled": project.unlock_fee_enabled,
        "unlock_fee_rate": project.unlock_fee_rate,
        "unlock_fee_basis": project.unlock_fee_basis,
        "unlock_fee_asset": project.unlock_fee_asset,
        "status": project.status,
        "post_listing_purchase_enabled": project.post_listing_purchase_enabled,
        "post_listing_pair_id": project.post_listing_pair_id,
        "post_listing_pair_status": project.post_listing_pair_status,
    })
}

/// 将单笔新币派发的项目、用户、认购、资产、数量、锁仓和幂等键映射为资金审计快照。
/// 结果不读取钱包流水；派发事务须把快照与余额或锁仓写入一并提交。
/// 幂等键进入快照是关键设计：排查是否重复发币时可直接按该键在审计中检索。
/// 锁仓头寸编号为空表示本次派发已直接入账可用余额，非空则表示资产先进入锁仓等待解禁。
pub(crate) fn new_coin_distribution_audit_json(
    distribution: &NewCoinDistributionResponse,
) -> Value {
    json!({
        "id": distribution.id,
        "project_id": distribution.project_id,
        "user_id": distribution.user_id,
        "subscription_id": distribution.subscription_id,
        "asset_id": distribution.asset_id,
        "quantity": distribution.quantity,
        "lock_position_id": distribution.lock_position_id,
        "status": distribution.status,
        "idempotency_key": distribution.idempotency_key,
        "created_at": distribution.created_at.timestamp_millis(),
    })
}

/// 将新币闪兑规则的交易对、汇率来源、固定/浮动配置、状态和创建人映射为审计快照。
/// JSON 保留浮动配置原值但不执行定价；调用方在规则插入或更新事务中保存前后值。
pub(crate) fn new_coin_convert_rule_audit_json(rule: &NewCoinConvertRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "convert_pair_id": rule.convert_pair_id,
        "rate_source": rule.rate_source,
        "fixed_rate": rule.fixed_rate,
        "floating_rate_json": rule.floating_rate_json.as_ref().map(|value| &value.0),
        "status": rule.status,
        "created_by": rule.created_by,
    })
}

/// 按解锁类型校验三组时间字段的互斥形状；计划上市只作配置，不作为实际成熟依据。
/// 上市即解锁必须给出计划上市时间且不得携带固定解锁时刻或相对周期；
/// 固定时间解锁必须给出解锁时刻且不得携带上市时间或相对周期；
/// 相对周期解锁必须给出正数秒数且不得携带任何绝对时间，秒数为 0 与缺失同等看待。
/// 未知类型和空白类型分别报不支持与必填错误。
/// 采用「必填 + 禁止他类字段」的双向约束，是为了避免残留的旧字段在类型切换后被误用。
fn validate_unlock_rule_shape(
    unlock_type: &str,
    listed_at: Option<DateTime<Utc>>,
    fixed_unlock_at: Option<DateTime<Utc>>,
    relative_unlock_seconds: Option<u64>,
) -> AppResult<()> {
    match optional_string(Some(unlock_type.to_owned())).as_deref() {
        Some("immediate_on_listing") => {
            if listed_at.is_none() {
                return Err(AppError::Validation(
                    "listed_at is required for immediate_on_listing unlock".to_owned(),
                ));
            }
            if fixed_unlock_at.is_some() || relative_unlock_seconds.is_some() {
                return Err(AppError::Validation(
                    "immediate_on_listing unlock cannot include fixed or relative unlock fields"
                        .to_owned(),
                ));
            }
        }
        Some("fixed_time") => {
            if fixed_unlock_at.is_none() {
                return Err(AppError::Validation(
                    "fixed_unlock_at is required for fixed_time unlock".to_owned(),
                ));
            }
            if listed_at.is_some() || relative_unlock_seconds.is_some() {
                return Err(AppError::Validation(
                    "fixed_time unlock cannot include listed_at or relative_unlock_seconds"
                        .to_owned(),
                ));
            }
        }
        Some("relative_period") => {
            if relative_unlock_seconds.unwrap_or(0) == 0 {
                return Err(AppError::Validation(
                    "relative_unlock_seconds is required for relative_period unlock".to_owned(),
                ));
            }
            if listed_at.is_some() || fixed_unlock_at.is_some() {
                return Err(AppError::Validation(
                    "relative_period unlock cannot include listed_at or fixed_unlock_at".to_owned(),
                ));
            }
        }
        Some(_) => {
            return Err(AppError::Validation(
                "unsupported new coin unlock_type".to_owned(),
            ));
        }
        None => return Err(AppError::Validation("unlock_type is required".to_owned())),
    }

    Ok(())
}

/// 校验解禁手续费规则：关闭时直接放行且不检查任何其他字段，开启时三项配置缺一不可。
/// 开启后要求费率严格为正、计费依据是 market_value 或 profit 之一、且必须指定收费资产。
/// 与解锁规则不同，这里不禁止关闭状态下残留旧字段，因为写入前应用层会主动把它们清空。
/// 费率为正意味着无法通过开启收费再配零费率来表达免费，免费应直接关闭该开关。
fn validate_unlock_fee_rule_shape(
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<&BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
) -> AppResult<()> {
    if !unlock_fee_enabled {
        return Ok(());
    }
    let Some(unlock_fee_rate) = unlock_fee_rate else {
        return Err(AppError::Validation(
            "unlock_fee_rate must be positive when unlock fee is enabled".to_owned(),
        ));
    };
    if unlock_fee_rate <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "unlock_fee_rate must be positive when unlock fee is enabled".to_owned(),
        ));
    }
    crate::modules::new_coin::service::ensure_new_coin_amount_precision(
        unlock_fee_rate,
        8,
        "unlock_fee_rate",
    )?;
    match optional_string(unlock_fee_basis).as_deref() {
        Some("market_value" | "profit") => {}
        Some(_) => {
            return Err(AppError::Validation(
                "unsupported unlock_fee_basis".to_owned(),
            ));
        }
        None => {
            return Err(AppError::Validation(
                "unlock_fee_basis is required when unlock fee is enabled".to_owned(),
            ));
        }
    }
    if unlock_fee_asset.is_none() {
        return Err(AppError::Validation(
            "unlock_fee_asset is required when unlock fee is enabled".to_owned(),
        ));
    }

    Ok(())
}

/// 把生命周期字符串代码映射为领域枚举，是请求侧与数据库侧两个解析入口共用的底层实现。
/// 严格按字面量匹配 preheat、subscription、distribution、listed 四值，不去空白也不做大小写归一，
/// 因此调用方需自行完成预处理；未知值统一返回校验错误，由上层决定是否改判为内部数据错误。
fn parse_lifecycle_status(value: &str) -> AppResult<LifecycleStatus> {
    match value {
        "preheat" => Ok(LifecycleStatus::Preheat),
        "subscription" => Ok(LifecycleStatus::Subscription),
        "distribution" => Ok(LifecycleStatus::Distribution),
        "listed" => Ok(LifecycleStatus::Listed),
        _ => Err(AppError::Validation(
            "unsupported new coin lifecycle_status".to_owned(),
        )),
    }
}

/// 把项目上扁平存放的解锁类型与时间字段还原成新币领域的解锁规则枚举。
/// 三种类型分别取上市时间、固定解锁时刻和相对秒数，缺失对应字段一律返回校验错误，
/// 因此历史脏数据会在派发时被拦下而不是产生错误的锁仓计划。
/// 相对秒数需从无符号整数转换为领域所需的有符号类型，超范围时报周期过大而非静默溢出。
fn unlock_rule_from_project(project: &NewCoinProjectResponse) -> AppResult<UnlockRule> {
    match project.unlock_type.as_str() {
        "immediate_on_listing" => Ok(UnlockRule::OnActualListing {
            project_id: project.id.to_string(),
            listed: project.lifecycle_status == "listed",
        }),
        "fixed_time" => Ok(UnlockRule::FixedTime {
            unlock_at: project.fixed_unlock_at.ok_or_else(|| {
                AppError::Validation("fixed_unlock_at is required for fixed unlock".to_owned())
            })?,
        }),
        "relative_period" => Ok(UnlockRule::RelativePeriod {
            seconds_after_source: project
                .relative_unlock_seconds
                .ok_or_else(|| {
                    AppError::Validation(
                        "relative_unlock_seconds is required for relative unlock".to_owned(),
                    )
                })?
                .try_into()
                .map_err(|_| {
                    AppError::Validation("relative unlock period is too large".to_owned())
                })?,
        }),
        _ => Err(AppError::Validation(
            "unsupported new coin unlock_type".to_owned(),
        )),
    }
}
