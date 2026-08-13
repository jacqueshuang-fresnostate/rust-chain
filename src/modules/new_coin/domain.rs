//! new_coin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 本文件定义新币发行的四个核心概念：项目生命周期状态机、解禁规则、解禁费计费规则，
//! 以及把一次分配拆成「立即可用额」与「待解禁锁仓」的算法。
//! 生命周期只允许预热、申购、分发、已上市这条单向链路推进，不支持跳级或回退。
//! 解禁规则分为上市即解禁、固定时点解禁和自来源时刻起算的相对周期解禁三种。
//! 解禁费支持按解禁市值或按解禁收益两种计费基准，收益口径对亏损批次不产生负费用。
//! 全部函数都是无 I/O 的纯函数：不访问数据库、不开事务、不写钱包、不发事件，
//! 返回的锁仓计划与费用报价只是资金计划，必须由仓储在自己的事务中落地。
//! 金额一律以 `BigDecimal` 计算且不做舍入，落库精度由数据库列定义决定。

use crate::modules::wallet::{
    LockPosition, LockPositionSource, LockSchedule, WalletDomainError, create_lock_positions,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStatus {
    Preheat,
    Subscription,
    Distribution,
    Listed,
}

impl LifecycleStatus {
    /// 只允许 `preheat → subscription → distribution → listed` 单向推进；跳级、回退和同状态重放均返回迁移错误。
    /// 三条合法迁移逐一枚举，未列出的组合一律拒绝并回带来源与目标状态，便于调用方定位非法操作。
    /// 同状态重放同样被拒，因此本方法不具备幂等性，重复推进需由调用方自行判重。
    /// 迁移只产生新的状态值，不写库、不校验业务前置条件，也不影响已有订单与锁仓。
    pub fn transition_to(self, to: LifecycleStatus) -> Result<LifecycleStatus, NewCoinDomainError> {
        match (self, to) {
            (LifecycleStatus::Preheat, LifecycleStatus::Subscription)
            | (LifecycleStatus::Subscription, LifecycleStatus::Distribution)
            | (LifecycleStatus::Distribution, LifecycleStatus::Listed) => Ok(to),
            (from, to) => Err(NewCoinDomainError::InvalidLifecycleTransition { from, to }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockType {
    ImmediateOnListing,
    FixedTime,
    RelativePeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockFeeBasis {
    MarketValue,
    Profit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinDomainError {
    InvalidLifecycleTransition {
        from: LifecycleStatus,
        to: LifecycleStatus,
    },
    SubscriptionNotOpen {
        status: LifecycleStatus,
    },
    PostListingPurchaseNotOpen {
        status: LifecycleStatus,
    },
    PostListingPurchaseDisabled,
    NonPositiveUnlockAmount,
    NonPositiveRelativePeriod,
    NegativeUnlockFeeRate,
    NegativeUnlockPrice,
    NegativePurchaseCost,
    MissingUnlockFeePaymentAsset,
    WalletLock(WalletDomainError),
    UnlockFeePaymentRequired {
        payment_asset: String,
        amount: BigDecimal,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCoinOrderKind {
    Subscription,
    Purchase,
}

impl NewCoinOrderKind {
    /// 返回订单种类对应的中文展示名，供面向用户的文案与通知直接使用。
    /// 注意此处的中文名与枚举名并非字面对应：`Subscription` 显示为「申购」，
    /// `Purchase` 显示为「认购」，这是业务侧沿用的既有叫法，改动会影响用户可见文案。
    pub fn chinese_name(self) -> &'static str {
        match self {
            NewCoinOrderKind::Subscription => "申购",
            NewCoinOrderKind::Purchase => "认购",
        }
    }

    /// 返回订单种类对应的稳定 API 动作代码，用于接口字段与埋点标识。
    /// 与展示名不同，这两个代码属于对外契约的一部分，一经发布不得随文案调整而变更，
    /// 否则会破坏既有客户端与历史数据的匹配。
    pub fn api_action(self) -> &'static str {
        match self {
            NewCoinOrderKind::Subscription => "subscription",
            NewCoinOrderKind::Purchase => "purchase",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockRule {
    ImmediateOnListing { listed_at: DateTime<Utc> },
    FixedTime { unlock_at: DateTime<Utc> },
    RelativePeriod { seconds_after_source: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockSource {
    pub user_id: String,
    pub asset_id: String,
    pub source_id: String,
    pub amount: BigDecimal,
    pub source_time: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct PendingLockSource {
    user_id: String,
    asset_id: String,
    wallet_source: LockPositionSource,
}

#[derive(Debug, Clone)]
pub struct UnlockApplication {
    pub available_amount: BigDecimal,
    pub locked_amount: BigDecimal,
    pub lock_positions: Vec<LockPosition>,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchasePlan {
    pub order_kind: NewCoinOrderKind,
    pub unlock: UnlockApplication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeRule {
    pub enabled: bool,
    pub rate: BigDecimal,
    pub basis: UnlockFeeBasis,
    pub payment_asset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeInput {
    pub unlock_quantity: BigDecimal,
    pub unlock_price: BigDecimal,
    pub purchase_cost: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeQuote {
    pub required: bool,
    pub amount: BigDecimal,
    pub basis: UnlockFeeBasis,
    pub payment_asset: Option<String>,
}

/// 仅在项目处于 `subscription` 生命周期时允许申购；本规则不读取项目或修改钱包。
/// 其余三个阶段一律拒绝并回带当前状态，使调用方能区分「尚未开始」与「已经结束」。
/// 这是申购路径的阶段守卫，必须在扣款之前调用；它只看阶段，
/// 不检查额度、白名单或用户资格，这些约束由各自的规则另行判定。
pub fn ensure_subscription_allowed(status: LifecycleStatus) -> Result<(), NewCoinDomainError> {
    if status == LifecycleStatus::Subscription {
        Ok(())
    } else {
        Err(NewCoinDomainError::SubscriptionNotOpen { status })
    }
}

/// 要求项目已上市且开启上市后购买，再按解锁规则把购买数量拆成可用额和锁仓额。
/// 两道守卫顺序固定：先看生命周期是否为 `listed`，再看后台购买开关，
/// 两者返回不同的错误变体，便于调用方区分「还没上市」与「上市了但未开放买入」。
/// 通过后把单条来源交给解禁规则拆分，来源时间即成交时刻，
/// 因此相对周期类解禁从本次买入起算，而非从项目上市起算。
/// 订单种类固定标记为 `Purchase`，用于区别于申购来源的分配。
/// 返回值只是资金计划；订单、余额、锁仓和流水仍须由仓储在同一事务中落地。
pub fn plan_post_listing_purchase(
    status: LifecycleStatus,
    enabled: bool,
    unlock_rule: &UnlockRule,
    source: UnlockSource,
) -> Result<PostListingPurchasePlan, NewCoinDomainError> {
    if status != LifecycleStatus::Listed {
        return Err(NewCoinDomainError::PostListingPurchaseNotOpen { status });
    }

    if !enabled {
        return Err(NewCoinDomainError::PostListingPurchaseDisabled);
    }

    Ok(PostListingPurchasePlan {
        order_kind: NewCoinOrderKind::Purchase,
        unlock: apply_unlock_rule(unlock_rule, vec![source])?,
    })
}

/// 将一组新币分配来源按项目解禁规则拆分为立即可用量和待解禁锁仓计划。
/// 调用方须传入同一用户、同一资产且金额均为正数的来源；相对周期必须大于零。
/// 上市前来源可合并到上市时解锁仓位，固定时点统一合并，相对周期按来源分别计算解锁时点。
/// 返回的可用量与各锁仓剩余量之和必须等于输入总量；本函数不写钱包、不启事务或审计。
/// 合并键和来源编号保持确定性，便于持久化层去重；非法规则或锁仓构造失败时无副作用。
pub fn apply_unlock_rule(
    unlock_rule: &UnlockRule,
    sources: Vec<UnlockSource>,
) -> Result<UnlockApplication, NewCoinDomainError> {
    ensure_positive_sources(&sources)?;

    match unlock_rule {
        UnlockRule::ImmediateOnListing { listed_at } => {
            let mut available_amount = BigDecimal::from(0);
            let mut locked_sources = Vec::new();

            for source in sources {
                if source.source_time >= *listed_at {
                    available_amount += source.amount;
                } else {
                    locked_sources.push(to_lock_source(source, *listed_at));
                }
            }

            let lock_positions = if locked_sources.is_empty() {
                Vec::new()
            } else {
                let user_id = locked_sources[0].user_id.clone();
                let asset_id = locked_sources[0].asset_id.clone();
                let wallet_sources = to_wallet_sources(locked_sources);
                create_lock_positions(
                    &user_id,
                    &asset_id,
                    LockSchedule::ImmediateOnListing {
                        listed_at: *listed_at,
                    },
                    wallet_sources,
                )
                .map_err(NewCoinDomainError::WalletLock)?
            };

            let locked_amount = sum_remaining(&lock_positions);
            Ok(UnlockApplication {
                available_amount,
                locked_amount,
                lock_positions,
            })
        }
        UnlockRule::FixedTime { unlock_at } => {
            let lock_sources = sources
                .into_iter()
                .map(|source| to_lock_source(source, *unlock_at))
                .collect::<Vec<_>>();
            let user_id = lock_sources[0].user_id.clone();
            let asset_id = lock_sources[0].asset_id.clone();
            let wallet_sources = to_wallet_sources(lock_sources);
            let lock_positions = create_lock_positions(
                &user_id,
                &asset_id,
                LockSchedule::FixedTime {
                    unlock_at: *unlock_at,
                },
                wallet_sources,
            )
            .map_err(NewCoinDomainError::WalletLock)?;
            let locked_amount = sum_remaining(&lock_positions);

            Ok(UnlockApplication {
                available_amount: BigDecimal::from(0),
                locked_amount,
                lock_positions,
            })
        }
        UnlockRule::RelativePeriod {
            seconds_after_source,
        } => {
            if *seconds_after_source <= 0 {
                return Err(NewCoinDomainError::NonPositiveRelativePeriod);
            }

            let lock_sources = sources
                .into_iter()
                .map(|source| {
                    let unlock_at = source.source_time + Duration::seconds(*seconds_after_source);
                    to_lock_source(source, unlock_at)
                })
                .collect::<Vec<_>>();
            let user_id = lock_sources[0].user_id.clone();
            let asset_id = lock_sources[0].asset_id.clone();
            let wallet_sources = to_wallet_sources(lock_sources);
            let lock_positions = create_lock_positions(
                &user_id,
                &asset_id,
                LockSchedule::RelativePeriod,
                wallet_sources,
            )
            .map_err(NewCoinDomainError::WalletLock)?;
            let locked_amount = sum_remaining(&lock_positions);

            Ok(UnlockApplication {
                available_amount: BigDecimal::from(0),
                locked_amount,
                lock_positions,
            })
        }
    }
}

/// 根据解禁数量、价格和购买成本计算应付解禁费用，不执行实际扣款。
/// 解禁数量须为正，价格、成本和费率不得为负；启用收费时必须配置非空支付资产。
/// 市值口径按数量乘价格计费，利润口径只对不低于零的收益计费，亏损不会产生负费用。
/// 该纯函数不处理资产精度、事务或审计，调用方须在扣款前按支付资产精度量化结果。
/// 相同规则和输入可安全重放；关闭规则返回零费用，校验失败不产生任何资金副作用。
pub fn calculate_unlock_fee(
    rule: &UnlockFeeRule,
    input: UnlockFeeInput,
) -> Result<UnlockFeeQuote, NewCoinDomainError> {
    let zero = BigDecimal::from(0);

    if input.unlock_quantity <= zero {
        return Err(NewCoinDomainError::NonPositiveUnlockAmount);
    }
    if input.unlock_price < zero {
        return Err(NewCoinDomainError::NegativeUnlockPrice);
    }
    if input.purchase_cost < zero {
        return Err(NewCoinDomainError::NegativePurchaseCost);
    }
    if rule.rate < zero {
        return Err(NewCoinDomainError::NegativeUnlockFeeRate);
    }

    if !rule.enabled {
        return Ok(UnlockFeeQuote {
            required: false,
            amount: BigDecimal::from(0),
            basis: rule.basis,
            payment_asset: rule.payment_asset.clone(),
        });
    }

    let payment_asset = rule
        .payment_asset
        .as_ref()
        .filter(|asset| !asset.trim().is_empty())
        .cloned()
        .ok_or(NewCoinDomainError::MissingUnlockFeePaymentAsset)?;
    let market_value = input.unlock_quantity * input.unlock_price;
    let basis_amount = match rule.basis {
        UnlockFeeBasis::MarketValue => market_value,
        UnlockFeeBasis::Profit => {
            max_decimal(market_value - input.purchase_cost, BigDecimal::from(0))
        }
    };
    let amount = basis_amount * rule.rate.clone();
    let required = amount > 0;

    Ok(UnlockFeeQuote {
        required,
        amount,
        basis: rule.basis,
        payment_asset: Some(payment_asset),
    })
}

/// 当报价要求收费时必须已有支付记录；免手续费报价可直接放行，函数本身不释放锁仓或入账。
/// 拒绝时回带支付资产与应付金额，让调用方能直接引导用户去缴费而不必重新报价。
/// 支付资产缺失时以空串占位，因为「要求收费却未配置资产」属于上游配置错误，
/// 应当在报价阶段就被拦下，此处不再重复校验。
/// 本函数只看报价与缴费标记两个入参，不看解禁时点是否已到，到期判定由持久化层负责。
pub fn ensure_unlock_release_allowed(
    fee: &UnlockFeeQuote,
    fee_paid: bool,
) -> Result<(), NewCoinDomainError> {
    if fee.required && !fee_paid {
        Err(NewCoinDomainError::UnlockFeePaymentRequired {
            payment_asset: fee.payment_asset.clone().unwrap_or_default(),
            amount: fee.amount.clone(),
        })
    } else {
        Ok(())
    }
}

/// 校验解禁来源集合非空且每条金额都严格为正，是拆分锁仓计划前的统一前置守卫。
/// 空集合与含非正金额的集合共用同一个错误变体，因为两者对下游都意味着「没有可锁定的有效数量」。
/// 该守卫同时保证后续按下标取首条来源的操作不会越界，
/// 因此三条解禁分支都可以放心地用首条来源的用户与资产作为整批的归属。
fn ensure_positive_sources(sources: &[UnlockSource]) -> Result<(), NewCoinDomainError> {
    if sources.is_empty() || sources.iter().any(|source| source.amount <= 0) {
        Err(NewCoinDomainError::NonPositiveUnlockAmount)
    } else {
        Ok(())
    }
}

/// 把一条解禁来源连同算好的解禁时点折成待建锁仓的中间结构，用户与资产被提到外层便于整批归组。
/// 来源编号、金额与解禁时点一起下沉到钱包侧的来源结构，成为存储层去重与金额累加的依据。
/// 解禁时点由调用分支各自计算：上市即解禁取上市时刻，固定时点取配置时刻，
/// 相对周期取来源时刻加上配置的秒数，本函数只做搬运不参与计算。
fn to_lock_source(source: UnlockSource, unlock_at: DateTime<Utc>) -> PendingLockSource {
    PendingLockSource {
        user_id: source.user_id,
        asset_id: source.asset_id,
        wallet_source: LockPositionSource {
            source_id: source.source_id,
            amount: source.amount,
            unlock_at,
        },
    }
}

/// 剥掉中间结构上的用户与资产标识，只留下钱包侧建仓所需的来源列表。
/// 之所以能安全丢弃这两个字段，是因为调用方已确认整批来源同属一个用户和一个资产，
/// 并已把它们单独提取出来作为建仓参数。
/// 转换保持原有顺序，使生成的锁仓来源编号具有确定性，便于持久化层按序去重。
fn to_wallet_sources(sources: Vec<PendingLockSource>) -> Vec<LockPositionSource> {
    sources
        .into_iter()
        .map(|source| source.wallet_source)
        .collect()
}

/// 汇总各锁仓位置的剩余待解禁量，得到本次分配真正被锁住的总额。
/// 取剩余量而非锁定总量，是因为合并到既有位置时该位置可能已释放过一部分，
/// 只有剩余量才对应当前仍需锁定的额度。
/// 空列表返回零，配合可用额即可满足「可用额与锁仓额之和等于输入总量」这一不变式。
fn sum_remaining(lock_positions: &[LockPosition]) -> BigDecimal {
    lock_positions
        .iter()
        .fold(BigDecimal::from(0), |sum, position| {
            sum + position.remaining_amount.clone()
        })
}

/// 返回两个十进制数中较大的一个，相等时返回左值，用于把解禁收益下限截到零。
/// 之所以自行实现而不用标准库的 `max`，是为了按值消费入参避免额外克隆，
/// `BigDecimal` 的克隆开销随位数增长，在计费路径上值得规避。
/// 比较基于数值大小而非 scale，因此 `0.0` 与 `0` 视为相等并返回左值。
fn max_decimal(left: BigDecimal, right: BigDecimal) -> BigDecimal {
    if left >= right { left } else { right }
}
