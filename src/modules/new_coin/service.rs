//! new_coin bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 本文件承担两类职责：一是 `NewCoinService` 这套面向旧同步仓储的用例编排，
//! 负责把上市后购买、解禁费报价与缴费、解禁释放的领域判定与持久化串起来；
//! 二是一组被 routes 与 infrastructure 共用的纯校验与换算函数，
//! 包括会话主体解析、条数上限、幂等键与金额校验、生命周期与解禁规则解析，
//! 以及解禁费金额和初始缴费状态的计算。
//! 本层不持有数据库连接、不开启事务、不加锁，也不发布事件，
//! 所有资金落地与原子性保证都由具体仓储实现承担；
//! 金额一律以 `BigDecimal` 传递，比较前统一 `normalized`，不做定点舍入。

use crate::{
    error::{AppError, AppResult},
    modules::new_coin::{
        LifecycleStatus, NewCoinDomainError, NewCoinOrderKind, UnlockFeeInput, UnlockFeeQuote,
        UnlockFeeRule, UnlockRule, UnlockSource, apply_unlock_rule, calculate_unlock_fee,
        ensure_unlock_release_allowed, plan_post_listing_purchase,
        repository::{NewCoinLockPositionWrite, NewCoinProjectRuleRead, UnlockFeeExpectation},
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use std::cmp::{Ordering, max};

use super::repository::{
    NewCoinPurchaseRepository, NewCoinRepositoryError, PostListingPurchaseRecord,
    UnlockFeePaymentRecord, UnlockFeeRepository, UnlockReleaseRecord, WalletLockCommandOutput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinServiceError {
    Domain(NewCoinDomainError),
    Repository(NewCoinRepositoryError),
}

impl From<NewCoinDomainError> for NewCoinServiceError {
    /// 把领域规则拒绝原样包进 `Domain` 变体，保留生命周期不符、金额非正等具体判定原因。
    /// 分类保留而不压平成字符串，使调用方能区分「业务规则不允许」与「存储访问失败」，
    /// 前者通常映射为参数校验类响应，后者才需要重试或告警。
    fn from(error: NewCoinDomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<NewCoinRepositoryError> for NewCoinServiceError {
    /// 把仓储侧的存储失败或状态解析失败包进 `Repository` 变体，与领域拒绝彻底分开。
    /// 转换只做归类不做重试，因此收到此变体时资金动作是否已部分落地取决于仓储实现，
    /// 服务层不会据此推断已提交或已回滚。
    fn from(error: NewCoinRepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseCommand {
    pub project_id: String,
    pub order_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quantity: BigDecimal,
    pub purchased_at: DateTime<Utc>,
    pub lifecycle_status: LifecycleStatus,
    pub post_listing_purchase_enabled: bool,
    pub unlock_rule: UnlockRule,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseResult {
    pub order_kind: NewCoinOrderKind,
    pub wallet_lock: WalletLockCommandOutput,
}

#[derive(Debug, Clone)]
pub struct UnlockFeeQuoteCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub unlock_quantity: BigDecimal,
    pub unlock_price: BigDecimal,
    pub purchase_cost: BigDecimal,
    pub fee_rule: UnlockFeeRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeQuoteResult {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quote: UnlockFeeQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayUnlockFeeCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub payment_asset: String,
    pub amount: BigDecimal,
    pub paid_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseUnlockCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub fee_quote: UnlockFeeQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseResult {
    pub unlock_id: String,
    pub released: bool,
}

#[derive(Debug, Clone)]
pub struct NewCoinService<R> {
    repository: R,
}

impl<R> NewCoinService<R> {
    /// 注入领域仓储实现；构造时不调用仓储，事务与持久化语义由具体方法和实现方负责。
    /// 仓储以值持有，服务本身无内部可变状态，克隆服务等同于克隆仓储句柄。
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// 借出只读仓储引用，供调用方在用例之外直接发起查询类操作。
    /// 通过此引用发出的读取不受本服务的领域校验保护，返回值也不参与用例的一致性判定。
    pub fn repository(&self) -> &R {
        &self.repository
    }

    /// 借出可变仓储引用，主要供测试装配预期或调用方执行本服务未封装的写入。
    /// 取得引用本身不触发任何持久化，但绕过此服务直接写入会跳过领域放行判定，
    /// 涉及资金的写入应优先走本服务提供的用例方法。
    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }
}

impl<R> NewCoinService<R>
where
    R: NewCoinPurchaseRepository,
{
    /// 校验上市后购买并生成钱包可用/锁仓增量，再通过仓储保存购买记录与资金计划。
    /// 先把命令折成单条 `UnlockSource`，来源编号取订单号，来源时间取成交时刻，
    /// 因此相对周期类解禁以 `purchased_at` 而非当前时钟为起点，重放同一命令得到同样的解禁时点。
    /// 领域侧要求项目已上市且开启上市后购买，否则直接返回 `Domain` 错误且不触碰仓储。
    /// 计划把购买数量拆成 `available_delta` 与 `locked_delta` 两部分，两者之和恒等于购买数量。
    /// 本服务不自行开启数据库事务；实现方必须保证订单、钱包锁仓和记录原子提交。
    /// 仓储保存失败时错误直接上抛，不返回成功计划，调用方据此判定本次购买整体未生效。
    pub fn create_post_listing_purchase(
        &mut self,
        command: PostListingPurchaseCommand,
    ) -> Result<PostListingPurchaseResult, NewCoinServiceError> {
        let unlock_source = UnlockSource {
            user_id: command.user_id.clone(),
            asset_id: command.asset_id.clone(),
            source_id: command.order_id.clone(),
            amount: command.quantity.clone(),
            source_time: command.purchased_at,
        };
        let plan = plan_post_listing_purchase(
            command.lifecycle_status,
            command.post_listing_purchase_enabled,
            &command.unlock_rule,
            unlock_source,
        )?;
        let wallet_lock = WalletLockCommandOutput {
            user_id: command.user_id.clone(),
            asset_id: command.asset_id.clone(),
            available_delta: plan.unlock.available_amount,
            locked_delta: plan.unlock.locked_amount,
            lock_positions: plan.unlock.lock_positions,
        };
        let record = PostListingPurchaseRecord {
            project_id: command.project_id,
            order_id: command.order_id,
            user_id: command.user_id,
            asset_id: command.asset_id,
            quantity: command.quantity,
            order_kind: plan.order_kind,
            purchased_at: command.purchased_at,
            wallet_lock: wallet_lock.clone(),
        };

        self.repository.save_post_listing_purchase(record)?;

        Ok(PostListingPurchaseResult {
            order_kind: plan.order_kind,
            wallet_lock,
        })
    }
}

impl<R> NewCoinService<R>
where
    R: UnlockFeeRepository,
{
    /// 按市值或非负利润口径计算解锁费报价，并把解锁、用户与资产标识一并回带便于调用方对齐上下文。
    /// 计算完全委托给领域函数：市值口径按数量乘解锁价，利润口径只对不低于零的收益计费，亏损不产生负费用。
    /// 数量非正、价格或成本为负、费率为负、启用收费却缺支付资产等情形都会返回 `Domain` 错误。
    /// 只返回快照，全程不访问仓储，因此既不查询当前缴费状态也不扣任何余额；
    /// 相同规则与输入可安全重复调用，得到完全一致的结果。
    pub fn quote_unlock_fee(
        &self,
        command: UnlockFeeQuoteCommand,
    ) -> Result<UnlockFeeQuoteResult, NewCoinServiceError> {
        let quote = calculate_unlock_fee(
            &command.fee_rule,
            UnlockFeeInput {
                unlock_quantity: command.unlock_quantity,
                unlock_price: command.unlock_price,
                purchase_cost: command.purchase_cost,
            },
        )?;

        Ok(UnlockFeeQuoteResult {
            unlock_id: command.unlock_id,
            user_id: command.user_id,
            asset_id: command.asset_id,
            quote,
        })
    }

    /// 把已由调用方校验的解锁费支付记录交给仓储保存，并把落库用的同一份记录回吐给调用方。
    /// 本方法自身不做任何校验：既不比对支付资产与金额是否符合报价，也不检查是否已经缴过费，
    /// 这些判定必须在调用之前由 `ensure_unlock_fee_payment_matches` 之类的守卫完成。
    /// 缴费时间取命令携带的 `paid_at` 而非当前时钟，便于补录历史缴费。
    /// 扣款、资金流水与 paid 状态置位须由仓储实现在同一事务内原子完成并自行防重复收费。
    pub fn pay_unlock_fee(
        &mut self,
        command: PayUnlockFeeCommand,
    ) -> Result<UnlockFeePaymentRecord, NewCoinServiceError> {
        let record = UnlockFeePaymentRecord {
            unlock_id: command.unlock_id,
            user_id: command.user_id,
            payment_asset: command.payment_asset,
            amount: command.amount,
            paid_at: command.paid_at,
        };

        self.repository.save_unlock_fee_payment(record.clone())?;
        Ok(record)
    }

    /// 有费用时先读取支付状态并执行领域放行，再请求仓储标记释放；仓储错误不返回 `released=true`。
    /// 报价 `required` 为假时跳过查询并直接以未缴费参与判定，因为免费解禁本就不看缴费状态，
    /// 这样可省掉一次无意义的存储访问；为真时必须查到已缴费才放行，否则返回缴费要求错误。
    /// 释放时间取调用时刻的 `Utc::now()`，因此重放会写入新的时间戳，
    /// 真正的重复释放防护依赖仓储实现的幂等键，本层不做去重。
    /// 到期校验、锁仓扣减、钱包入账与账本写入均由仓储实现在同一事务内原子完成，
    /// 本方法只在仓储成功返回后才把结果置为已释放。
    pub fn release_unlock(
        &mut self,
        command: ReleaseUnlockCommand,
    ) -> Result<UnlockReleaseResult, NewCoinServiceError> {
        let fee_paid = if command.fee_quote.required {
            self.repository
                .unlock_fee_paid(&command.unlock_id, &command.user_id)?
        } else {
            false
        };
        ensure_unlock_release_allowed(&command.fee_quote, fee_paid)?;

        let record = UnlockReleaseRecord {
            unlock_id: command.unlock_id.clone(),
            user_id: command.user_id,
            asset_id: command.asset_id,
            released_at: Utc::now(),
        };
        self.repository.mark_unlock_released(record)?;

        Ok(UnlockReleaseResult {
            unlock_id: command.unlock_id,
            released: true,
        })
    }
}

/// 从 `user:{id}` 会话 subject 解析用户编号，格式不符返回 Unauthorized。
/// 前缀缺失或数字部分溢出 `u64` 都归为鉴权失败而非参数错误，
/// 因为这类 subject 只可能来自伪造或过期令牌，不应向调用方暴露解析细节。
/// 解析结果是后续所有新币查询与下单的租户隔离依据，不得由请求体覆盖。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 将新币列表条数默认设为 50，并夹取到 1 到 100 的闭区间。
/// 采用夹取而非报错，使超范围的查询参数退化为边界值而不是让整个请求失败。
/// 传 0 会被抬到 1，因此调用方无法用零条数探测接口，也不会出现空 `LIMIT` 拖垮查询。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 支付资产和金额必须与解禁记录固化的手续费快照完全一致，异参请求不允许推进 paid 状态。
/// 依次拦截四种情形：该解禁本就不收费、支付资产与要求资产不符、记录未配置应收金额、金额不匹配。
/// 前三种都返回 `Validation`，避免调用方通过换资产或对免费解禁缴费来伪造已缴费状态。
/// 金额必须严格为正，且与期望值按 `normalized` 比较，
/// 使 `1.0` 与 `1.000000` 这类仅 scale 不同的等值支付不会被误拒。
/// 本函数是纯校验，不读写存储也不扣款，通过后仍需由缴费路径自身保证不重复收费。
pub(crate) fn ensure_unlock_fee_payment_matches(
    expectation: &UnlockFeeExpectation,
    payment_asset_id: u64,
    amount: &BigDecimal,
) -> AppResult<()> {
    if !expectation.unlock_fee_enabled {
        return Err(AppError::Validation(
            "unlock fee payment is not required for this unlock".to_owned(),
        ));
    }
    if expectation.unlock_fee_asset != Some(payment_asset_id) {
        return Err(AppError::Validation(
            "unlock fee payment asset does not match required asset".to_owned(),
        ));
    }
    let Some(expected_amount) = &expectation.unlock_fee_amount else {
        return Err(AppError::Validation(
            "unlock fee amount is not configured".to_owned(),
        ));
    };
    // 金额比较使用 normalized，避免同一数值因 scale 不同导致合法支付被拒。
    if amount <= &BigDecimal::default()
        || amount.normalized().cmp(&expected_amount.normalized()) != Ordering::Equal
    {
        return Err(AppError::Validation(
            "unlock fee payment amount does not match required amount".to_owned(),
        ));
    }
    Ok(())
}

/// 金额必须严格为正，零和负数一并拒绝，失败时不得创建报价、订单、钱包变更或流水。
/// `field` 只参与错误文案拼接，用于让调用方知道是数量、价格还是金额越界，不影响判定本身。
/// 这是资金入口的第一道守卫，必须在开启事务之前调用，避免为无效请求占用行锁。
pub(crate) fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::default() {
        Err(AppError::Validation(format!("{field} must be positive")))
    } else {
        Ok(())
    }
}

/// 新币申购、购买与解禁共用的幂等键守卫，去掉首尾空白后必须仍然非空。
/// 空键会让存储层的唯一约束失去意义，使同一请求重试写出多张订单，因此必须在开启事务之前拒绝。
/// 注意本函数只校验非空，不做长度、字符集或归一化处理，
/// 也不改写入参，因此含首尾空白的键会原样进入数据库并与去空后的键视为不同键。
pub(crate) fn ensure_idempotency_key(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        Err(AppError::Validation(
            "idempotency_key must not be empty".to_owned(),
        ))
    } else {
        Ok(())
    }
}

/// 把数据库中的生命周期字符串解析为枚举，取值依次为预热、申购、分发、已上市四个阶段。
/// 未知取值一律返回 `Validation` 而不降级为预热或已上市，
/// 因为任何一侧的臆测都可能让本不该开放的申购或买入被放行。
/// 该函数是判定「能否申购」「能否上市后买入」的前置步骤，本身不做阶段迁移也不读写存储。
pub(crate) fn lifecycle_status(value: &str) -> AppResult<LifecycleStatus> {
    match value {
        "preheat" => Ok(LifecycleStatus::Preheat),
        "subscription" => Ok(LifecycleStatus::Subscription),
        "distribution" => Ok(LifecycleStatus::Distribution),
        "listed" => Ok(LifecycleStatus::Listed),
        _ => Err(AppError::Validation(
            "unsupported lifecycle_status".to_owned(),
        )),
    }
}

/// 要求项目已启用上市后购买，且请求的交易对恰好等于后台批准的那一个，两个条件缺一不可。
/// 未配置交易对时 `post_listing_pair_id` 为空，与任何请求值都不相等，因此同样被拒。
/// 两种失败共用一条错误文案，不向调用方区分是开关未开还是交易对不符，避免暴露后台配置细节。
/// 生命周期是否为已上市由调用方另行校验，本函数不看阶段，也不读写存储。
pub(crate) fn ensure_post_listing_purchase_enabled(
    project: &NewCoinProjectRuleRead,
    requested_pair_id: u64,
) -> AppResult<()> {
    if !project.post_listing_purchase_enabled
        || project.post_listing_pair_id != Some(requested_pair_id)
    {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    Ok(())
}

/// 按项目解禁规则把一次分发数量拆成待写库的锁仓明细，是下单事务准备锁仓数据的唯一入口。
/// 先从项目快照还原解禁规则，再以单条来源调用领域拆分，来源时间决定相对周期类解禁的起算点。
/// 返回明细的 `amount` 取领域算出的剩余量而非原始来源金额，因此立即解禁的部分不会出现在结果里，
/// 上市时点之后买入且规则为上市即解禁时，结果为空表示全额直接可用。
/// `merge_key` 由领域按用户、资产与解禁时点生成，存储层据此把同批次多次分配合并到一个锁仓位置。
/// 领域拒绝时统一折叠为 `Validation` 并带上原始错误的调试信息，
/// 此时不返回任何部分计划；本函数是纯计算，不读写存储也不改钱包。
pub(crate) fn lock_positions_for_project(
    project: &NewCoinProjectRuleRead,
    user_id: u64,
    asset_id: u64,
    source_id: &str,
    quantity: BigDecimal,
    source_time: chrono::DateTime<Utc>,
    source_type: &str,
) -> AppResult<Vec<NewCoinLockPositionWrite>> {
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
        .map(|position| NewCoinLockPositionWrite {
            user_id,
            asset_id,
            unlock_type: position.unlock_type,
            unlock_at: position.unlock_at,
            amount: position.remaining_amount,
            merge_key: position.merge_key,
            source_type: source_type.to_owned(),
            source_id: source_id.to_owned(),
        })
        .collect())
}

/// 从项目快照还原解禁规则，把存储中「类型字符串加多个可空列」的表示收敛为有效的领域枚举。
/// 三种类型各自要求一个必填列：上市即解禁需要 `listed_at`，固定时点需要 `fixed_unlock_at`，
/// 相对周期需要 `relative_unlock_seconds`，缺失时分别返回带列名的 `Validation` 错误。
/// 相对周期从无符号秒数转成有符号秒数，超出范围同样拒绝，避免溢出后算出过去的解禁时点。
/// 未知解禁类型不做兜底，直接报错以防按错误规则锁仓；本函数不读写存储也不校验项目阶段。
pub(crate) fn unlock_rule_from_project(project: &NewCoinProjectRuleRead) -> AppResult<UnlockRule> {
    match project.unlock_type.as_str() {
        "immediate_on_listing" => Ok(UnlockRule::ImmediateOnListing {
            listed_at: project.listed_at.ok_or_else(|| {
                AppError::Validation("listed_at is required for immediate unlock".to_owned())
            })?,
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

/// 按项目费率计算某个锁仓批次的解禁费金额与初始缴费状态，结果会被固化进解禁记录。
/// 返回的状态字符串直接对应存储枚举：not_required 表示无需缴费，pending 表示应收未付。
/// 依次短路三种无需收费的情形：项目未开启收费返回空金额；费率为零或为负返回零金额；
/// 两者的区别在于前者连金额列都不写，后者写入显式的零。
/// 开启收费且费率为正时必须已配置支付资产，否则返回 `Validation`，防止算出应收却无处可付。
/// 计费基准默认按解禁市值，即数量乘解禁价；配置为 profit 时改按解禁收益，
/// 收益取市值减购买成本并与零取大，因此亏损批次的费用为零而非负数。
/// 基准取值超出这两种时直接报错，不静默回退到市值口径。
/// 金额为零时状态回落到 not_required，避免生成一条永远无需支付的 pending 记录。
/// 本函数是纯计算：不扣款、不写库、不做资产精度量化，落库精度由数据库列定义决定。
pub(crate) fn unlock_fee_fields(
    project: &NewCoinProjectRuleRead,
    quantity: &BigDecimal,
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
) -> AppResult<(&'static str, Option<BigDecimal>)> {
    if !project.unlock_fee_enabled {
        return Ok(("not_required", None));
    }
    let fee_rate = project.unlock_fee_rate.clone().unwrap_or_default();
    if fee_rate <= BigDecimal::default() {
        return Ok(("not_required", Some(BigDecimal::default())));
    }
    if project.unlock_fee_asset.is_none() {
        return Err(AppError::Validation(
            "unlock_fee_asset is required when unlock fee is enabled".to_owned(),
        ));
    }
    let market_value = quantity.clone() * unlock_price.clone();
    let basis_amount = match project
        .unlock_fee_basis
        .as_deref()
        .unwrap_or("market_value")
    {
        "market_value" => market_value,
        "profit" => max(market_value - purchase_cost.clone(), BigDecimal::default()),
        _ => {
            return Err(AppError::Validation(
                "unsupported unlock_fee_basis".to_owned(),
            ));
        }
    };
    let fee_amount = basis_amount * fee_rate;
    let fee_paid_status = if fee_amount > BigDecimal::default() {
        "pending"
    } else {
        "not_required"
    };
    Ok((fee_paid_status, Some(fee_amount)))
}
