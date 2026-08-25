//! wallet bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 覆盖充值地址分配、链上充值观测与冲正、提现申请与审核放行、钱包账户与流水查询、已实现收益与历史收益六条用例。
//! 入参归一化、状态取值校验和分页边界统一在本层完成，SQL 与事务细节全部下沉到基础设施子模块。
//! 收益口径固定为 UTC 自然日、USDT 计价、18 位定点向零截断；缺价时如实返回 partial，绝不用旧价或零价补足。
//! 本层不直接广播链上交易，也不发布领域事件；提现只落申请与冻结，实际上链由链网关 worker 消费已提交状态推进。

use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        risk::{RiskGuardInput, RiskScope, enforce_risk_control},
        security::{SecurityAction, SecurityVerificationInput, verify_user_security_action},
        wallet::{
            infrastructure,
            infrastructure::{
                ReturnHistoryAssetActivityRow, TodayReturnAssetActivityRow, WalletLedgerCategory,
                WalletLedgerFilter,
            },
            presentation::{
                AdminWalletListQuery, AdminWalletWithdrawalsResponse, BroadcastWithdrawalRequest,
                ConfirmWithdrawalRequest, CreateWithdrawalQuoteRequest, CreateWithdrawalRequest,
                DepositAddressRequest, DepositAddressResponse, DepositAssetResponse,
                DepositNetworkResponse, DepositNetworksQuery, FailWithdrawalRequest,
                ObserveDepositRequest, ReturnHistoryMissingPrice, ReturnHistoryPoint,
                ReturnHistoryResponse, ReturnHistorySummary, ReverseDepositRequest,
                ReviewWithdrawalRequest, TodayReturnResponse, TodayReturnStatus,
                WalletAccountResponse, WalletDepositEventResponse, WalletDepositsResponse,
                WalletLedgerQuery, WalletLedgerResponse, WalletWithdrawalQuery,
                WalletWithdrawalResponse, WithdrawalQuoteResponse, WithdrawalRequestResponse,
            },
            truncate_amount_to_asset_precision,
        },
    },
    state::AppState,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use mongodb::Database;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

const TODAY_RETURN_REPORTING_ASSET: &str = "USDT";
const TODAY_RETURN_REPORTING_SCALE: i32 = 18;
const REALIZED_RETURN_ZERO: &str = "0.000000000000000000";
const WITHDRAWAL_QUOTE_TTL_SECONDS: i64 = 300;

/// 列出当前启用且允许充值的资产配置，供充值入口的币种选择列表使用。
/// 该用例无入参可校验，直接透传基础设施查询；返回的精度与最小充值额是后续链上入账的判定依据。
/// 资产可充值不代表一定有网络和地址库存，真正能否拿到地址要到分配用例才能确定。
pub(crate) async fn list_deposit_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    infrastructure::list_deposit_assets(pool).await
}

/// 列出当前启用且允许提现的资产及其固定费用、阶梯费率与精度配置，供提现表单预估费用。
/// 这里返回的费率仅供前端展示，真实扣费在创建申请时由服务端按同一套规则重新计算并以服务端结果为准。
/// 因此前端据此估算的费用与最终冻结额可能因配置变更或阶梯边界而不同，不得据此校验用户输入。
pub(crate) async fn list_withdraw_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    infrastructure::list_withdraw_assets(pool).await
}

/// 按可选资产代码列出启用网络及地址组配置，资产为空时返回全部启用网络。
/// 入参必须是调用方已归一化的大写资产代码，本函数不再做格式校验，直接进入基础设施查询。
/// 返回的地址组代码决定后续分配从哪个库存池取地址，同组网络之间地址可复用。
pub(crate) async fn list_deposit_networks(
    pool: &Pool<MySql>,
    asset_symbol: Option<&str>,
) -> AppResult<Vec<DepositNetworkResponse>> {
    infrastructure::list_active_deposit_networks(pool, asset_symbol).await
}

/// 面向查询 DTO 的充值网络列表入口，路由层只传原始参数，规范化与校验统一收敛到应用层。
/// 资产代码缺省时按不过滤处理；格式非法则在触达数据库前返回校验错误，不会退化成全量网络列表。
/// 归一化后转交无 DTO 依赖的列表函数执行，本函数自身不访问数据库，也不改变任何地址池状态。
pub(crate) async fn list_deposit_networks_by_query(
    pool: &Pool<MySql>,
    query: &DepositNetworksQuery,
) -> AppResult<Vec<DepositNetworkResponse>> {
    let asset_symbol = normalize_deposit_networks_query_asset(query)?;

    list_deposit_networks(pool, asset_symbol.as_deref()).await
}

/// 把充值网络查询里的可选资产代码规范为大写形式，缺省时保持缺省，非法格式返回校验错误。
/// 该函数供路由在进入用例前先行拦截坏参数，使参数错误以校验错误呈现而不是变成数据库空结果。
/// 只做纯字符串处理，不触达数据库，也不判断该资产是否真实存在或是否开放充值。
pub(crate) fn normalize_deposit_networks_query_asset(
    query: &DepositNetworksQuery,
) -> AppResult<Option<String>> {
    query
        .asset_symbol
        .as_deref()
        .map(normalize_asset_symbol)
        .transpose()
}

/// 获取用户在指定资产、网络与地址组中的充值地址；已有分配直接复用，不轮换地址。
/// 请求先完成资产/网络规范化并确认充值已启用；新分配会在单事务内锁定一条可用地址、读取用户邮箱并绑定用户。
/// 地址池行锁保证并发请求不会把同一地址分配给多个用户；事务失败不改变库存，调用方可安全重试并读取既有分配。
/// 本函数只更新本地地址池，不调用链网关，也不产生充值入账或外部消息。
pub(crate) async fn get_or_assign_deposit_address(
    pool: &Pool<MySql>,
    user_id: u64,
    request: DepositAddressRequest,
) -> AppResult<DepositAddressResponse> {
    let request = normalize_deposit_address_request(request)?;
    let network_config = infrastructure::load_active_deposit_network_config(
        pool,
        &request.network,
        &request.asset_symbol,
    )
    .await?;
    infrastructure::ensure_deposit_enabled_asset(pool, &request.asset_symbol).await?;

    if let Some(mut address) = infrastructure::load_user_deposit_address(
        pool,
        user_id,
        &request.asset_symbol,
        &network_config.address_group_code,
        &request.network,
    )
    .await?
    {
        address.network = request.network;
        return Ok(address);
    }

    // 地址池库存锁定、用户邮箱读取和分配写入必须在同一个事务中完成，避免同一地址被并发分配。
    let mut tx = pool.begin().await?;
    let candidate_id = infrastructure::lock_available_deposit_address(
        &mut tx,
        &request.asset_symbol,
        &network_config.address_group_code,
        &request.network,
    )
    .await?;
    let assigned_user_email = infrastructure::load_user_email_in_tx(&mut tx, user_id).await?;
    infrastructure::assign_deposit_address_in_tx(
        &mut tx,
        candidate_id,
        user_id,
        assigned_user_email,
        &request.asset_symbol,
    )
    .await?;
    let mut address = infrastructure::load_deposit_address_in_tx(&mut tx, candidate_id).await?;
    tx.commit().await?;
    address.network = request.network;
    Ok(address)
}

/// 读取用户全部资产账户的 available/frozen/locked 当前快照，按资产代码升序返回并带出符号与图标。
/// 结果包含余额为零的已开通资产，也会遗漏尚未初始化账户行的资产，因此不能据此判断某资产是否受支持。
/// 查询不加资金行锁，返回值只用于展示，扣款前必须在事务内重新锁行读取最新余额。
pub(crate) async fn list_wallet_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<WalletAccountResponse>> {
    infrastructure::list_wallet_accounts(pool, user_id).await
}

/// 以当前 UTC 时间计算用户当日已实现收益，缺价时返回部分状态而非估算值。
/// 该用例只读结算活动和行情，不锁钱包三桶，也不追加任何资金流水。
pub(crate) async fn get_today_return(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
) -> AppResult<TodayReturnResponse> {
    get_today_return_at(pool, redis, user_id, Utc::now()).await
}

/// 聚合指定 UTC 日内可审计收益并加载时效内行情后完成 USDT 估值。
/// 统计区间为计算时刻所在 UTC 自然日的零点到该时刻，跨时区用户看到的当日口径因此统一为 UTC。
/// 只为收益或本金非零且非稳定币的资产请求报价，去重后按资产代码有序请求，避免为零活动资产浪费行情查询。
/// Redis 句柄缺省时报价集合为空，除稳定币外的资产都会缺价，结果整体退化为 partial 而非报错。
/// 该只读用例不锁钱包或写流水；缺失、过期价格保持 partial，避免错误价格影响资产展示。
pub(crate) async fn get_today_return_at(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    calculated_at: DateTime<Utc>,
) -> AppResult<TodayReturnResponse> {
    let period_start_at = utc_day_start(&calculated_at);
    let activity = infrastructure::load_today_return_asset_activity(
        pool,
        user_id,
        period_start_at,
        calculated_at,
    )
    .await?;
    let priced_assets = activity
        .iter()
        .filter(|row| {
            (row.amount != 0 || row.basis_amount != 0) && !is_stablecoin(&row.asset_symbol)
        })
        .map(|row| row.asset_symbol.trim().to_ascii_uppercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let prices = match redis {
        Some(redis) => {
            infrastructure::load_current_usdt_prices(redis, &priced_assets, calculated_at).await?
        }
        None => BTreeMap::new(),
    };

    Ok(calculate_today_return(
        activity,
        &prices,
        period_start_at,
        calculated_at,
    ))
}

/// 只接受一、七、三十或一百八十天这四个收益历史窗口，其余取值和缺省一律返回校验错误。
/// 窗口固定成枚举而非任意区间，是为了限制历史行情回查的规模并让前端切换项与后端口径完全对齐。
/// 缺省不会退化成默认天数，调用方必须显式指定窗口，避免前端漏传时静默返回非预期区间。
pub(crate) fn validate_return_history_days(days: Option<u16>) -> AppResult<u16> {
    match days {
        Some(days @ (1 | 7 | 30 | 180)) => Ok(days),
        _ => Err(AppError::Validation(
            "days must be one of 1, 7, 30, or 180".to_owned(),
        )),
    }
}

/// 以当前 UTC 时间读取并计算用户指定天数的已实现收益历史。
/// 历史估值只读 Mongo、Redis 与结算数据，不改变账户余额或行情缓存。
pub(crate) async fn get_return_history(
    pool: &Pool<MySql>,
    mongo: Option<&Database>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    period_days: u16,
) -> AppResult<ReturnHistoryResponse> {
    get_return_history_at(pool, mongo, redis, user_id, period_days, Utc::now()).await
}

/// 按 UTC 日聚合收益活动，历史日使用 Mongo 收盘价，今日使用 Redis 当前价。
/// 窗口含当日在内向前推算，因此一天窗口等价于今日收益，一百八十天窗口的起点是当日零点减一百七十九天。
/// 先扫描活动行把待估值需求拆成两份：历史日按资产收集所需日期集合，当日资产另收一份，稳定币和零活动资产都不参与。
/// 历史价与当前价分别只在对应需求非空且依赖可用时才发起查询，Mongo 或 Redis 缺省时该侧价格集合为空。
/// 两份报价与活动行一起交给纯计算函数产出逐日曲线，本函数自身不做任何金额换算。
/// 该只读用例不改变钱包、不写流水；任一所需报价缺失时保留对应日期并明确标记 partial。
pub(crate) async fn get_return_history_at(
    pool: &Pool<MySql>,
    mongo: Option<&Database>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    period_days: u16,
    calculated_at: DateTime<Utc>,
) -> AppResult<ReturnHistoryResponse> {
    let period_days = validate_return_history_days(Some(period_days))?;
    let today_start_at = utc_day_start(&calculated_at);
    let period_start_at =
        today_start_at - TimeDelta::days(i64::from(period_days.saturating_sub(1)));
    let activity = infrastructure::load_return_history_asset_activity(
        pool,
        user_id,
        period_start_at,
        calculated_at,
    )
    .await?;
    let today = today_start_at.date_naive();
    let mut historical_price_days = BTreeMap::<String, BTreeSet<NaiveDate>>::new();
    let mut current_price_assets = BTreeSet::new();
    for row in &activity {
        let asset_symbol = row.asset_symbol.trim().to_ascii_uppercase();
        let has_value = row.amount != 0 || row.basis_amount != 0;
        if !has_value || is_stablecoin(&asset_symbol) {
            continue;
        }
        if row.activity_day == today {
            current_price_assets.insert(asset_symbol);
        } else if row.activity_day < today {
            historical_price_days
                .entry(asset_symbol)
                .or_default()
                .insert(row.activity_day);
        }
    }

    let historical_prices = match mongo {
        Some(database) if !historical_price_days.is_empty() => {
            infrastructure::load_historical_usdt_daily_closes(database, &historical_price_days)
                .await?
        }
        _ => BTreeMap::new(),
    };
    let current_price_assets = current_price_assets.into_iter().collect::<Vec<_>>();
    let current_prices = match redis {
        Some(redis) if !current_price_assets.is_empty() => {
            infrastructure::load_current_usdt_prices(redis, &current_price_assets, calculated_at)
                .await?
        }
        _ => BTreeMap::new(),
    };

    Ok(calculate_return_history(
        activity,
        &historical_prices,
        &current_prices,
        period_days,
        period_start_at,
        calculated_at,
    ))
}

/// 逐日将可审计终态业务的已实现收益与本金基数换算为 USDT，并生成固定天数的累计曲线。
/// 输出点数恒等于窗口天数，无活动的日期同样补出金额与基数为零的完整点位，前端无需自行补齐日历。
/// USDT/USDC/USD 按一比一；历史日取精确 UTC 日线，今日取时效内 Redis 价，已知金额向零截断至 18 位。
/// 每个点的估值时刻区分对待：历史日记为次日零点表示该日已收敛，当日记为本次计算时刻表示仍在变动。
/// 日收益率以当日金额除以当日基数得出，基数不为正时直接取零而非报错，避免无本金日出现除零。
/// 任一活动资产缺价时该日金额、基数与收益率整体置空，并从该日起停止累计，其后所有点的累计值都保持未知。
/// 只要出现过一次缺价，整体状态即为 partial 且总摘要三项全部置空，绝不返回只算了一半的合计。
/// 纯计算函数不读取数据库或行情缓存，也不修改钱包余额与流水。
pub(crate) fn calculate_return_history(
    activity: Vec<ReturnHistoryAssetActivityRow>,
    historical_prices: &BTreeMap<(NaiveDate, String), BigDecimal>,
    current_prices: &BTreeMap<String, BigDecimal>,
    period_days: u16,
    period_start_at: DateTime<Utc>,
    calculated_at: DateTime<Utc>,
) -> ReturnHistoryResponse {
    let zero = BigDecimal::from(0);
    let scaled_zero = realized_return_zero();
    let today = utc_day_start(&calculated_at).date_naive();
    let mut activity_by_day = BTreeMap::<NaiveDate, Vec<ReturnHistoryAssetActivityRow>>::new();
    for row in activity {
        activity_by_day
            .entry(row.activity_day)
            .or_default()
            .push(row);
    }

    let mut points = Vec::with_capacity(usize::from(period_days));
    let mut missing_prices = Vec::new();
    let mut cumulative_amount = scaled_zero.clone();
    let mut summary_basis_amount = scaled_zero.clone();
    let mut cumulative_known = true;
    let mut response_status = TodayReturnStatus::Complete;

    for offset in 0..period_days {
        let day_start_at = period_start_at + TimeDelta::days(i64::from(offset));
        let day = day_start_at.date_naive();
        let mut daily_amount = zero.clone();
        let mut daily_basis_amount = zero.clone();
        let mut missing_price_assets = BTreeSet::new();

        for row in activity_by_day.remove(&day).unwrap_or_default() {
            let asset_symbol = row.asset_symbol.trim().to_ascii_uppercase();
            let has_value = row.amount != zero || row.basis_amount != zero;
            if !has_value {
                continue;
            }
            let price = if is_stablecoin(&asset_symbol) {
                Some(BigDecimal::from(1))
            } else if day == today {
                current_prices.get(&asset_symbol).cloned()
            } else {
                historical_prices.get(&(day, asset_symbol.clone())).cloned()
            };
            let Some(price) = price else {
                missing_price_assets.insert(asset_symbol);
                continue;
            };
            daily_amount += row.amount * price.clone();
            daily_basis_amount += row.basis_amount * price;
        }

        let valued_at = if day == today {
            calculated_at
        } else {
            day_start_at + TimeDelta::days(1)
        };
        if missing_price_assets.is_empty() {
            let daily_amount = quantize_realized_return(&daily_amount);
            let daily_basis_amount = quantize_realized_return(&daily_basis_amount);
            let daily_rate = realized_return_rate(&daily_amount, &daily_basis_amount);
            let point_cumulative = if cumulative_known {
                cumulative_amount =
                    quantize_realized_return(&(cumulative_amount + daily_amount.clone()));
                Some(cumulative_amount.clone())
            } else {
                None
            };
            summary_basis_amount =
                quantize_realized_return(&(summary_basis_amount + daily_basis_amount.clone()));
            points.push(ReturnHistoryPoint {
                day_start_at,
                valued_at,
                amount: Some(daily_amount),
                basis_amount: Some(daily_basis_amount),
                rate: Some(daily_rate),
                cumulative_amount: point_cumulative,
                status: TodayReturnStatus::Complete,
                missing_price_assets: Vec::new(),
            });
        } else {
            response_status = TodayReturnStatus::Partial;
            cumulative_known = false;
            for asset_symbol in &missing_price_assets {
                missing_prices.push(ReturnHistoryMissingPrice {
                    day_start_at,
                    asset_symbol: asset_symbol.clone(),
                });
            }
            points.push(ReturnHistoryPoint {
                day_start_at,
                valued_at,
                amount: None,
                basis_amount: None,
                rate: None,
                cumulative_amount: None,
                status: TodayReturnStatus::Partial,
                missing_price_assets: missing_price_assets.into_iter().collect(),
            });
        }
    }

    let summary = if response_status == TodayReturnStatus::Complete {
        ReturnHistorySummary {
            amount: Some(cumulative_amount.clone()),
            basis_amount: Some(summary_basis_amount.clone()),
            rate: Some(realized_return_rate(
                &cumulative_amount,
                &summary_basis_amount,
            )),
        }
    } else {
        ReturnHistorySummary {
            amount: None,
            basis_amount: None,
            rate: None,
        }
    };

    ReturnHistoryResponse {
        scope: "realized",
        reporting_asset: TODAY_RETURN_REPORTING_ASSET,
        period_days,
        period_start_at,
        calculated_at,
        status: response_status,
        summary,
        missing_prices,
        points,
    }
}

/// 将当日各资产已实现收益与本金基数按服务端价格换算为 USDT，并以 amount/basis 计算收益率。
/// 资产代码在比对前统一裁剪并转大写，收益与基数同时为零的活动行直接跳过，不会因此触发缺价判定。
/// 稳定币按一比一，已知值向零截断到 18 位；截断后为零的结果会归一成带 18 位小数的正零，避免输出负零。
/// 与历史曲线不同，这里对缺价资产采取继续累加其余资产的策略：合计保留已知部分，同时标记 partial 并列出缺价资产。
/// 因此 partial 状态下的金额是不完整的下界而非最终值，调用方不得把它当作确定收益展示。
/// 收益率取合计金额除以合计基数，基数不为正时直接返回零；该纯计算不读取或修改 available/frozen/locked，也不追加钱包流水。
pub(crate) fn calculate_today_return(
    activity: Vec<TodayReturnAssetActivityRow>,
    prices: &BTreeMap<String, BigDecimal>,
    period_start_at: DateTime<Utc>,
    calculated_at: DateTime<Utc>,
) -> TodayReturnResponse {
    let zero = BigDecimal::from(0);
    let mut amount = zero.clone();
    let mut basis_amount = zero.clone();
    let mut missing_price_assets = BTreeSet::new();

    for row in activity {
        let asset_symbol = row.asset_symbol.trim().to_ascii_uppercase();
        let has_value = row.amount != zero || row.basis_amount != zero;
        if !has_value {
            continue;
        }
        let price = if is_stablecoin(&asset_symbol) {
            Some(BigDecimal::from(1))
        } else {
            prices.get(&asset_symbol).cloned()
        };
        let Some(price) = price else {
            missing_price_assets.insert(asset_symbol);
            continue;
        };

        amount += row.amount * price.clone();
        basis_amount += row.basis_amount * price;
    }

    let amount = quantize_realized_return(&amount);
    let basis_amount = quantize_realized_return(&basis_amount);
    let rate = realized_return_rate(&amount, &basis_amount);
    let status = if missing_price_assets.is_empty() {
        TodayReturnStatus::Complete
    } else {
        TodayReturnStatus::Partial
    };

    TodayReturnResponse {
        scope: "realized",
        reporting_asset: TODAY_RETURN_REPORTING_ASSET,
        amount,
        basis_amount,
        rate,
        period_start_at,
        calculated_at,
        status,
        missing_price_assets: missing_price_assets.into_iter().collect(),
    }
}

/// 以收益金额除以本金基数得出收益率，基数不为正时返回规范化零而非报错或无穷大。
/// 结果是小数倍率而非百分比，展示层需自行乘一百；商同样按 18 位向零截断，因此极小收益率可能被截成零。
fn realized_return_rate(amount: &BigDecimal, basis_amount: &BigDecimal) -> BigDecimal {
    if basis_amount > &BigDecimal::from(0) {
        quantize_realized_return(&(amount.clone() / basis_amount.clone()))
    } else {
        realized_return_zero()
    }
}

/// 把收益中间结果统一收敛到 18 位定点：先向零截断，再把任意形式的零归一成固定标度的正零。
/// 向零截断意味着正收益少算、负收益也少算，绝对值只会变小，不会因舍入放大用户收益。
/// 零归一保证负数被截断后不会输出负零，也让累计过程中的零值拥有一致的标度与序列化形态。
fn quantize_realized_return(value: &BigDecimal) -> BigDecimal {
    let value = truncate_amount_to_asset_precision(value, TODAY_RETURN_REPORTING_SCALE);
    if value == 0 {
        realized_return_zero()
    } else {
        value
    }
}

/// 返回收益口径下的规范零值，即标度固定为 18 位的正零，作为累计初值与缺省收益率。
/// 常量文本在编译期已确定合法，解析失败属于不可能分支，因此直接断言而非向上传播错误。
fn realized_return_zero() -> BigDecimal {
    BigDecimal::from_str(REALIZED_RETURN_ZERO).expect("realized return zero is valid decimal")
}

/// 返回给定时刻所属 UTC 自然日的零点，是当日收益与历史曲线共用的唯一分日基准。
/// 一律按 UTC 而非用户本地时区切日，保证同一笔结算在任何客户端都归属同一天。
/// 零点在 UTC 下恒定存在，因此内部断言不会触发，不存在夏令时导致的缺失时刻问题。
pub(crate) fn utc_day_start(calculated_at: &DateTime<Utc>) -> DateTime<Utc> {
    calculated_at
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("UTC calendar day start is always valid")
        .and_utc()
}

/// 判定资产是否按美元平价计价，命中的资产在收益估值中直接取价格一而不查询任何行情。
/// 名单固定为三种美元稳定币，比对前裁剪空白并转大写；名单外的稳定币仍需真实报价，缺价即标记 partial。
/// 平价假设意味着脱锚行情不会反映在收益里，这是为避免稳定币报价缺失拖垮整体估值而做的取舍。
fn is_stablecoin(asset_symbol: &str) -> bool {
    matches!(
        asset_symbol.trim().to_ascii_uppercase().as_str(),
        "USDT" | "USDC" | "USD"
    )
}

/// 按已构建过滤器读取用户钱包流水和三桶后快照，不修改余额。
/// 过滤行与总数由基础设施统一构造，查询结果仅用于审计和展示。
pub(crate) async fn list_wallet_ledger(
    pool: &Pool<MySql>,
    user_id: u64,
    filter: WalletLedgerFilter,
) -> AppResult<WalletLedgerResponse> {
    infrastructure::list_wallet_ledger(pool, user_id, filter).await
}

/// 将钱包列表页大小规范为默认 50、最小 1、最大 100，钳制而非报错以免前端传界外值直接失败。
/// 零会被抬到一，避免除以页大小计算总页数时出现除零；后台入口可在此结果上再放宽到二百。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 将钱包列表偏移默认为 0，并封顶到十万，防止深翻页把数据库拖入大量无效扫描。
/// 超限同样只钳制不报错，因此请求极深页码会稳定返回封顶位置的数据而不是空错误响应。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 裁剪可选查询字符串的首尾空白，并把裁剪后为空的取值归一成缺省。
/// 归一避免空串被当作有效筛选条件写进 SQL，从而把本应全量的查询意外收窄成零结果。
pub(crate) fn normalize_optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 裁剪资产代码首尾空白并转为大写，空值与格式非法分别返回不同的校验错误消息。
/// 只接受纯 ASCII 字母数字且裁剪后长度不超过三十二字节，短横线、下划线和中文等字符一律拒绝。
/// 长度按字节比较，因此多字节字符在字符集校验阶段就已被拦下，不会绕过长度限制。
/// 归一只保证格式合法，不查询资产是否存在、是否启用，也不判断其充提开关。
pub(crate) fn normalize_asset_symbol(value: &str) -> AppResult<String> {
    let symbol = value.trim();
    if symbol.is_empty() {
        return Err(AppError::Validation("asset_symbol is required".to_owned()));
    }
    if symbol.len() > 32 || !symbol.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::Validation(
            "asset_symbol format is invalid".to_owned(),
        ));
    }
    Ok(symbol.to_ascii_uppercase())
}

/// 把网络标识收敛到受支持的规范名，采用固定别名表而非通用格式校验，杜绝任意字符串流入地址池查询。
/// 以太坊系的三种写法统一成 eth，波场系三种写法统一成 tron，比特币与 Solana 各自合并两种写法，Base 单独保留。
/// 比对前裁剪空白并转小写，因此大小写混写可以通过；别名表之外的取值一律返回不支持该充值网络的校验错误。
/// 归一结果同时用于地址分配和链事件网络比对，两侧共用本函数才能保证网关回执与本地记录能对上。
/// 本函数只做名称映射，不查询网络配置是否启用，也不判断该资产是否被该网络接受。
pub(crate) fn normalize_deposit_network(value: &str) -> AppResult<String> {
    let network = value.trim().to_ascii_lowercase();
    match network.as_str() {
        "eth" | "ethereum" | "erc20" => Ok("eth".to_owned()),
        "base" => Ok("base".to_owned()),
        "tron" | "trx" | "trc20" => Ok("tron".to_owned()),
        "btc" | "bitcoin" => Ok("btc".to_owned()),
        "sol" | "solana" => Ok("solana".to_owned()),
        _ => Err(AppError::Validation(
            "unsupported deposit network".to_owned(),
        )),
    }
}

/// 把账本查询 DTO 规范为资产、分类、引用、时间及分页过滤器。
/// 分类必须精确匹配十类之一，未知取值返回带完整候选清单的校验错误，而不是静默忽略该筛选条件。
/// 资产代码走统一归一并可能报错，其余文本条件只做裁剪与空值归一，起止时间原样保留交由 SQL 比较。
/// 分页在此完成钳制，页大小落在一到一百之间，偏移封顶十万，因此过滤器交给基础设施时已是安全边界。
/// 未知分类、非法资产代码在执行 SQL 前拒绝；行查询与计数随后复用同一过滤器以保证总数与数据一致。
pub(crate) fn build_wallet_ledger_filter(
    query: WalletLedgerQuery,
) -> AppResult<WalletLedgerFilter> {
    let category = query
        .category
        .map(|value| {
            WalletLedgerCategory::parse(value.trim()).ok_or_else(|| {
                AppError::Validation(format!(
                    "unsupported wallet ledger category; expected one of: {}",
                    WalletLedgerCategory::ALL
                        .iter()
                        .map(|category| category.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
        })
        .transpose()?;

    Ok(WalletLedgerFilter {
        asset_id: query.asset_id,
        asset_symbol: query
            .asset_symbol
            .map(|value| normalize_asset_symbol(&value))
            .transpose()?,
        change_type: normalize_optional_query_string(query.change_type),
        category,
        ref_type: normalize_optional_query_string(query.ref_type),
        ref_id: normalize_optional_query_string(query.ref_id),
        start_time: normalize_optional_query_string(query.start_time),
        end_time: normalize_optional_query_string(query.end_time),
        limit: route_limit(query.limit),
        offset: route_offset(query.offset),
    })
}

/// 生成并持久化服务端权威提现报价。金额先按资产精度向零截断，再用规范化阶梯计费；
/// fee/net/total_reserved、配置版本、所有者和指纹一次入库，提交时不再信任客户端金额派生值。
pub(crate) async fn create_withdrawal_quote(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreateWithdrawalQuoteRequest,
) -> AppResult<WithdrawalQuoteResponse> {
    let asset_symbol = normalize_asset_symbol(&request.asset_symbol)?;
    let network = normalize_deposit_network(&request.network)?;
    if request.amount <= 0 {
        return Err(AppError::Validation("amount must be positive".to_owned()));
    }

    // 报价同样必须取配置行锁：先网络、后资产，与提现消费事务保持同一锁序。
    // 这样精度、阶梯、版本与入库 quote 来自同一个不可并发改写的快照。
    let mut tx = pool.begin().await?;
    if let Err(error) =
        infrastructure::lock_active_withdrawal_network_in_tx(&mut tx, &network, &asset_symbol).await
    {
        return match error {
            AppError::Conflict(_) => Err(AppError::Validation(format!(
                "asset {asset_symbol} does not support withdrawal network {network}"
            ))),
            error => Err(error),
        };
    }
    // 先取精度，再以标准化金额重算阶梯，避免截断恰好跨越费率边界。
    let initial =
        infrastructure::load_withdrawal_asset_rule_in_tx(&mut tx, &asset_symbol, &request.amount)
            .await?;
    let amount =
        truncate_amount_to_asset_precision(&request.amount, initial.precision_scale).with_scale(18);
    if amount <= 0 {
        return Err(AppError::Validation(
            "amount is below the asset precision".to_owned(),
        ));
    }
    let asset =
        infrastructure::load_withdrawal_asset_rule_in_tx(&mut tx, &asset_symbol, &amount).await?;
    let quote = infrastructure::insert_withdrawal_quote_in_tx(
        &mut tx,
        user_id,
        &asset,
        &asset_symbol,
        &network,
        &amount,
        Utc::now() + TimeDelta::seconds(WITHDRAWAL_QUOTE_TTL_SECONDS),
    )
    .await?;
    tx.commit().await?;
    Ok(quote)
}

/// 创建提现申请并冻结“申请金额 + 服务端报价费用”；调用方必须提供已认证用户、稳定幂等键及安全验证凭据。
/// 用例依次执行资产精度/费用规则、幂等重放、风控和资金安全校验，命中拒绝时不得消耗资金或生成申请。
/// 申请记录、available→frozen 变更及账本由基础设施在同一事务提交；费用以资产精度截断后的服务端规则为准。
/// 冻结只写一条 available 负流水，金额为本金加费用；frozen 增量体现在同条流水的三桶账后快照。
/// 相同幂等键只接受资产、网络、地址、金额和费用一致的重放；并发唯一键冲突会回读旧申请，绝不二次冻结。
/// 本函数不广播链上交易，后续审核与网关 worker 只能消费已提交的申请状态。
pub(crate) async fn create_withdrawal_request(
    pool: &Pool<MySql>,
    settings: &Settings,
    user_id: u64,
    request: CreateWithdrawalRequest,
) -> AppResult<WithdrawalRequestResponse> {
    let request = validate_withdrawal_request(request)?;
    let quote = infrastructure::load_withdrawal_quote(pool, &request.quote_id, user_id).await?;
    if request
        .network
        .as_deref()
        .is_some_and(|network| network != quote.network)
    {
        return Err(AppError::Conflict(
            "withdrawal quote does not match request parameters".to_owned(),
        ));
    }
    let network = quote.network.as_str();
    let expected_fingerprint = super::withdrawal_quote_fingerprint(
        user_id,
        quote.asset_id,
        &request.asset_symbol,
        network,
        &request.amount,
    );
    if quote.asset_symbol != request.asset_symbol
        || quote.amount != request.amount
        || quote.fee != request.fee
        || quote.request_fingerprint != expected_fingerprint
    {
        return Err(AppError::Conflict(
            "withdrawal quote does not match request parameters".to_owned(),
        ));
    }
    if let Some(existing) =
        infrastructure::load_withdrawal_by_user_key(pool, user_id, &request.idempotency_key).await?
    {
        ensure_withdrawal_replay_matches(&existing, &request, &quote.response())?;
        return withdrawal_request_response(existing, quote.response());
    }
    if quote.expires_at <= Utc::now() {
        return Err(AppError::Validation(
            "withdrawal quote is expired".to_owned(),
        ));
    }
    if quote.consumed_at.is_some() || quote.withdrawal_id.is_some() {
        return Err(AppError::Conflict(
            "withdrawal quote was already consumed".to_owned(),
        ));
    }
    if let Err(error) =
        infrastructure::ensure_active_withdrawal_network(pool, network, &request.asset_symbol).await
    {
        return match error {
            AppError::Validation(_) | AppError::NotFound => Err(AppError::Conflict(
                "withdrawal quote network configuration has changed".to_owned(),
            )),
            error => Err(error),
        };
    }
    let asset =
        infrastructure::load_withdrawal_asset_rule(pool, &request.asset_symbol, &request.amount)
            .await?;
    if asset.id != quote.asset_id
        || asset.fee_config_version != quote.fee_config_version
        || asset.fee != quote.fee
    {
        return Err(AppError::Conflict(
            "withdrawal quote fee configuration has changed".to_owned(),
        ));
    }
    // 风控闸门先于安全校验和冻结执行，命中拒绝时不消耗验证凭据、也不产生任何资金状态。
    // 提现用例不持有 Redis 句柄，限频规则在该路径不生效。
    enforce_risk_control(
        pool,
        None,
        RiskGuardInput {
            user_id,
            operation: "wallet.withdrawal.create",
            scopes: vec![
                RiskScope::new("user", user_id.to_string()),
                RiskScope::new("asset", request.asset_symbol.clone()),
            ],
            amount: Some(request.amount.clone()),
            price: None,
            reference_price: None,
        },
    )
    .await?;
    let security_method = verify_user_security_action(
        pool,
        settings,
        user_id,
        SecurityAction::Withdraw,
        SecurityVerificationInput {
            fund_password: request.fund_password.as_deref(),
            totp_code: request.totp_code.as_deref(),
        },
    )
    .await?;

    // 请求、余额冻结和账本必须同事务提交；唯一键冲突时只允许返回完全一致的历史请求。
    let withdrawal = match infrastructure::reserve_withdrawal_request(
        pool,
        user_id,
        &request.quote_id,
        &request.asset_symbol,
        network,
        &request.address,
        &request.amount,
        &request.idempotency_key,
        security_method.as_str(),
    )
    .await
    {
        Ok(withdrawal) => withdrawal,
        Err(AppError::Database(error)) if is_duplicate_key_error(&error) => {
            let existing = infrastructure::load_withdrawal_by_user_key(
                pool,
                user_id,
                &request.idempotency_key,
            )
            .await?
            .ok_or_else(|| {
                AppError::Conflict("withdrawal idempotency key was used concurrently".to_owned())
            })?;
            let replay_quote =
                infrastructure::load_withdrawal_quote(pool, &request.quote_id, user_id).await?;
            ensure_withdrawal_replay_matches(&existing, &request, &replay_quote.response())?;
            return withdrawal_request_response(existing, replay_quote.response());
        }
        Err(error) => return Err(error),
    };
    withdrawal_request_response(withdrawal.withdrawal, withdrawal.quote)
}

/// 按当前用户和可选状态读取提现请求，不暴露其他用户记录。
/// 查询只读申请和链进度，不锁钱包，也不移动 available 或 frozen。
pub(crate) async fn list_user_withdrawals(
    pool: &Pool<MySql>,
    user_id: u64,
    query: WalletWithdrawalQuery,
) -> AppResult<Vec<WalletWithdrawalResponse>> {
    let status = normalize_withdrawal_status(query.status)?;
    infrastructure::list_wallet_withdrawals(
        pool,
        Some(user_id),
        status.as_deref(),
        route_limit(query.limit),
    )
    .await
}

/// 规范后台状态与分页后读取提现请求及匹配总数，供运营侧翻页审阅。
/// 状态取值必须属于八个已登记状态之一，非法取值在查库前返回校验错误；用户编号缺省时跨用户返回全量申请。
/// 页大小先按通用规则钳到一到一百，再取与二百的较小值，因此后台单页上限实际仍是一百，偏移沿用十万封顶。
/// 行数据和 total 使用相同筛选，查询不改变冻结预留额或提现状态，也不推进任何链上进度。
pub(crate) async fn list_admin_withdrawals(
    pool: &Pool<MySql>,
    query: AdminWalletListQuery,
) -> AppResult<AdminWalletWithdrawalsResponse> {
    let status = normalize_withdrawal_status(query.status)?;
    let (withdrawals, total) = infrastructure::list_admin_wallet_withdrawals_page(
        pool,
        query.user_id,
        status.as_deref(),
        route_limit(query.limit).min(200),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminWalletWithdrawalsResponse { withdrawals, total })
}

/// 由应用层开启事务推进待审核提现为 approved，保留原 frozen 预留额等待链上广播。
/// 审核意见可选，仅做裁剪与空值归一，不强制填写也不限制长度；管理员编号由调用方从后台令牌解析后传入。
/// 批准会把下次尝试时刻置为当前时间，使链网关 worker 在下一轮即可认领该申请，因此本调用是自动广播的触发点。
/// 重复已批准请求幂等返回；状态冲突或写入失败时事务回滚，余额与流水完全不变。
pub(crate) async fn approve_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: ReviewWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = normalize_optional_query_string(request.reason);
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::approve_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        admin_id,
        reason.as_deref(),
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

/// 由应用层开启事务拒绝提现，并把完整 frozen 预留额连本带费退回 available 后写释放流水。
/// 拒绝原因是必填项，缺失或全空白返回校验错误，超过五百一十二字符同样拒绝，避免超长文本写入审核字段。
/// 仅允许从待审核或已批准状态拒绝，已经进入广播的申请必须改走失败或人工审核路径。
/// 订单先锁、钱包后锁；已拒绝重放不二次退款，状态、余额与流水任一步失败都整体回滚。
pub(crate) async fn reject_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: ReviewWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = required_reason(request.reason, "rejection reason")?;
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::release_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        "rejected",
        &reason,
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

/// 在应用层事务记录已由外部流程取得的交易哈希和确认进度，不发起链网关 HTTP，也不核销 frozen 预留额。
/// 交易哈希必填并先做格式归一，空串、超长或含空白字符直接返回校验错误；确认数缺省按零处理。
/// 该入口用于人工补录已在链上发出的交易，与 worker 自动广播共用同一状态迁移，因此两条路径不会重复推进。
/// 同哈希重放仅推进确认数；状态冲突或写入失败时事务回滚，链进度不会部分提交。
pub(crate) async fn broadcast_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: BroadcastWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let tx_hash = normalize_chain_identifier(request.tx_hash, "tx_hash")?;
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::mark_withdrawal_broadcasted_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        &tx_hash,
        request.block_height,
        request.confirmations.unwrap_or(0),
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

/// 在应用层事务核销提现 frozen 预留额、写确认流水并推进 confirmed，这是资金真正离开钱包的一步。
/// 确认数缺省按一处理，区块高度可缺省，两者最终都以取较大值或择非空的方式写入，不会让链上进度倒退。
/// 只接受已广播或人工审核状态；本入口供后台在链回执缺失时手工确认，与 worker 自动确认互为幂等。
/// 已确认重放不二次扣减；冻结不足或任一步失败时余额、流水和状态整体回滚。
pub(crate) async fn confirm_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: ConfirmWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::confirm_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        request.block_height,
        request.confirmations.unwrap_or(1),
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

/// 在尚可安全退款的状态下把提现标记失败，并将 frozen 全额连本带费退回 available。
/// 失败原因由请求体必填字段提供，仍需经过非空与五百一十二字符上限校验，空白原因视为缺失。
/// 与拒绝共用同一释放实现但只接受尚未进入广播的已批准状态，覆盖调用链网关之前已确定失败的场景。
/// 广播中、结果不明或已有交易哈希的请求均不会经此释放，必须查询到权威未受理或继续人工复核；目标状态重放不生成第二笔退款流水。
pub(crate) async fn fail_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: FailWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = required_reason(Some(request.reason), "failure reason")?;
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::release_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        "failed",
        &reason,
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

/// 规范链充值事件后按 network/tx_hash/event_index 幂等观察；仅达到服务端确认数时增加 available。
/// 首次入账写正向 deposit 流水，frozen/locked 不变；重复事件只推进确认数且不二次入账。
/// 基础设施拥有事件、钱包和流水事务，字段冲突、精度/最小额不符或 SQL 失败均不留下本次部分资金结果。
pub(crate) async fn observe_deposit(
    pool: &Pool<MySql>,
    request: ObserveDepositRequest,
) -> AppResult<WalletDepositEventResponse> {
    let request = normalize_observe_deposit_request(request)?;
    infrastructure::observe_deposit_event(pool, &request).await
}

/// 按充值编号执行幂等链重组冲正：available 足额时扣回原到账金额并写负向流水。
/// 已冲正重放直接返回；状态不允许时冲突，available 不足则提交 manual_review 而不扣款、不写冲正流水。
/// 事件、余额和流水事务由基础设施拥有，本应用入口只规范必填原因并转交处理。
pub(crate) async fn reverse_deposit(
    pool: &Pool<MySql>,
    deposit_id: u64,
    request: ReverseDepositRequest,
) -> AppResult<WalletDepositEventResponse> {
    let reason = required_reason(Some(request.reason), "reversal reason")?;
    infrastructure::reverse_deposit_event(pool, deposit_id, &reason).await
}

/// 按后台用户和分页条件读取充值链事件及匹配总数，不触发入账或冲正。
/// 该只读用例不锁钱包、不写流水，也不推进链网关游标或确认状态。
pub(crate) async fn list_admin_deposits(
    pool: &Pool<MySql>,
    query: AdminWalletListQuery,
) -> AppResult<WalletDepositsResponse> {
    let (deposits, total) = infrastructure::list_deposit_events(
        pool,
        query.user_id,
        route_limit(query.limit).min(200),
        route_offset(query.offset),
    )
    .await?;
    Ok(WalletDepositsResponse { deposits, total })
}

/// 从应用状态取出 MySQL 连接池的克隆句柄，池未配置时返回内部错误而非让调用方拿到空依赖。
/// 克隆只增加引用计数、不新建连接，因此每个请求各自取用不会造成额外开销。
/// 钱包全部用例都必须经此获取连接，从而把依赖缺失统一表达为可观测的服务端错误。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for wallet routes".to_owned())
    })
}

/// 从后台令牌主体解析管理员编号，要求带固定前缀且其余部分能解析为无符号整数。
/// 前缀不符或解析失败一律返回未授权而非校验错误，避免把令牌结构异常暴露成参数问题。
/// 审核、拒绝、广播、确认和失败五类操作的操作人都取自这里，管理路由因此不会信任请求体传来的管理员标识。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 校验并归一提现请求体，返回可直接进入资产规则与冻结流程的规范化副本。
/// 资产代码与地址不得为空白，金额必须严格为正，客户端传入的费用只要求非负且不参与实际计费。
/// 幂等键裁剪后不得为空、不超过一百二十八字符，且只允许 ASCII 字母数字与短横线、下划线、冒号、点号。
/// 资产代码在此直接转大写而不走通用归一，因此长度与字符集不受资产代码规则约束；网络字段按别名表收敛，非法网络在此报错。
/// 资金密码与两步验证码原样透传给后续安全校验，本函数不做脱敏、不校验其正确性，也不触达数据库。
fn validate_withdrawal_request(
    request: CreateWithdrawalRequest,
) -> AppResult<CreateWithdrawalRequest> {
    let quote_id = request.quote_id.trim();
    if uuid::Uuid::parse_str(quote_id).is_err() {
        return Err(AppError::Validation(
            "quote_id format is invalid".to_owned(),
        ));
    }
    if request.asset_symbol.trim().is_empty() {
        return Err(AppError::Validation("asset_symbol is required".to_owned()));
    }
    if request.address.trim().is_empty() {
        return Err(AppError::Validation("address is required".to_owned()));
    }
    if request.amount <= 0 {
        return Err(AppError::Validation("amount must be positive".to_owned()));
    }
    if request.fee < 0 {
        return Err(AppError::Validation("fee must be non-negative".to_owned()));
    }
    let idempotency_key = request.idempotency_key.trim();
    if idempotency_key.is_empty()
        || idempotency_key.len() > 128
        || !idempotency_key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
    {
        return Err(AppError::Validation(
            "idempotency_key format is invalid".to_owned(),
        ));
    }

    Ok(CreateWithdrawalRequest {
        quote_id: quote_id.to_owned(),
        asset_symbol: request.asset_symbol.trim().to_ascii_uppercase(),
        network: request
            .network
            .map(|network| normalize_deposit_network(&network))
            .transpose()?,
        address: request.address.trim().to_owned(),
        amount: request.amount,
        fee: request.fee,
        idempotency_key: idempotency_key.to_owned(),
        fund_password: request.fund_password,
        totp_code: request.totp_code,
    })
}

/// 核对幂等重放是否与既有申请完全一致，任一关键字段不同即返回冲突而不是返回旧申请。
/// 比对资产代码、网络、地址、金额四项请求参数，并额外比对既有申请的费用与当前服务端算出的费用。
/// 把费用纳入比对意味着费率配置变更后同一幂等键会被判为冲突，这是刻意为之，避免按旧费率重放新意图。
/// 校验通过不代表申请已完成，只说明可以安全返回既有结果；该函数不修改任何状态也不加锁。
fn ensure_withdrawal_replay_matches(
    existing: &WalletWithdrawalResponse,
    request: &CreateWithdrawalRequest,
    quote: &WithdrawalQuoteResponse,
) -> AppResult<()> {
    if existing.withdrawal_quote_id.as_deref() != Some(request.quote_id.as_str())
        || existing.asset_symbol != request.asset_symbol
        || existing.network.as_deref() != Some(quote.network.as_str())
        || request
            .network
            .as_deref()
            .is_some_and(|network| network != quote.network)
        || existing.address != request.address
        || existing.amount != request.amount
        || existing.fee != quote.fee
        || existing.total_reserved != quote.total_reserved
    {
        return Err(AppError::Conflict(
            "withdrawal idempotency key was reused with different parameters".to_owned(),
        ));
    }
    Ok(())
}

/// 把完整提现申请裁剪为创建接口的精简响应，只回传编号、状态、冻结总额与实际使用的安全校验方式。
/// 安全方式字符串需反解为枚举，仅接受资金密码、两步验证及两者兼备三种取值，未知取值返回内部错误。
/// 这里刻意报错而非兜底，因为无法识别的安全方式意味着申请记录已损坏，不应继续对外展示。
/// 响应不包含地址、幂等键和链上进度，避免创建接口回显敏感或尚未生效的字段。
fn withdrawal_request_response(
    withdrawal: WalletWithdrawalResponse,
    quote: WithdrawalQuoteResponse,
) -> AppResult<WithdrawalRequestResponse> {
    let security_method = match withdrawal.security_method.as_str() {
        "fund_password" => crate::modules::security::SecurityVerificationMethod::FundPassword,
        "two_factor" => crate::modules::security::SecurityVerificationMethod::TwoFactor,
        "fund_password_and_two_factor" => {
            crate::modules::security::SecurityVerificationMethod::FundPasswordAndTwoFactor
        }
        _ => {
            return Err(AppError::Internal(
                "withdrawal security method is invalid".to_owned(),
            ));
        }
    };
    if withdrawal.withdrawal_quote_id.as_deref() != Some(quote.quote_id.as_str())
        || withdrawal.asset_symbol != quote.asset_symbol
        || withdrawal.network.as_deref() != Some(quote.network.as_str())
        || withdrawal.amount != quote.amount
        || withdrawal.fee != quote.fee
        || withdrawal.total_reserved != quote.total_reserved
    {
        return Err(AppError::Internal(
            "withdrawal and quote snapshots do not match".to_owned(),
        ));
    }
    Ok(WithdrawalRequestResponse {
        id: withdrawal.id,
        quote_id: quote.quote_id,
        status: withdrawal.status,
        asset_symbol: quote.asset_symbol,
        network: quote.network,
        amount: quote.amount,
        fee: quote.fee,
        net: quote.net,
        total_reserved: quote.total_reserved,
        fee_config_version: quote.fee_config_version,
        expires_at: quote.expires_at,
        security_method,
    })
}

/// 校验提现状态筛选值，先做裁剪与空值归一，再要求命中九个已登记状态之一。
/// 允许集合覆盖待审核、已批准、广播中、结果不明、已广播、已确认、人工审核、已拒绝和已失败，缺省表示不按状态筛选。
/// 比对区分大小写且不做别名映射，非法取值返回校验错误，避免拼错状态时静默返回空列表让运营误判。
fn normalize_withdrawal_status(status: Option<String>) -> AppResult<Option<String>> {
    let status = normalize_optional_query_string(status);
    if let Some(status) = status.as_deref()
        && !matches!(
            status,
            "pending_review"
                | "approved"
                | "broadcasting"
                | "unknown_broadcast"
                | "broadcasted"
                | "confirmed"
                | "manual_review"
                | "rejected"
                | "failed"
        )
    {
        return Err(AppError::Validation(
            "withdrawal status is invalid".to_owned(),
        ));
    }
    Ok(status)
}

/// 校验拒绝、失败、冲正等操作的必填原因，缺失或全空白返回带业务标签的校验错误。
/// 长度上限五百一十二字符按字节计，超限直接报错而非截断，确保存档原因与运营填写内容完全一致。
fn required_reason(reason: Option<String>, label: &str) -> AppResult<String> {
    let reason = normalize_optional_query_string(reason)
        .ok_or_else(|| AppError::Validation(format!("{label} is required")))?;
    if reason.len() > 512 {
        return Err(AppError::Validation(format!(
            "{label} must not exceed 512 characters"
        )));
    }
    Ok(reason)
}

/// 校验链上地址或交易哈希等标识，裁剪首尾空白后拒绝空串、超过二百五十五字节以及任何内嵌空白字符。
/// 标识保持原始大小写，因为部分链的地址校验和依赖大小写，归一会破坏其可校验性。
fn normalize_chain_identifier(value: String, label: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 255 || value.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(format!("{label} format is invalid")));
    }
    Ok(value.to_owned())
}

/// 校验并归一链上充值观测请求，使外部观测在进入幂等入账前具备可比对的稳定形态。
/// 金额必须严格为正，地址与交易哈希按链上标识规则校验，资产代码转大写，网络按别名表收敛到规范名。
/// 备注做裁剪并把空白归一为缺省，因为备注参与事件身份一致性比对，空串与缺省必须视为同一含义。
/// 事件序号、区块高度与确认数原样透传，本函数不判断确认是否达标，也不校验地址是否已分配给某个用户。
/// 归一只保证形态，入账与否完全由基础设施在事务中按幂等键与确认阈值决定。
fn normalize_observe_deposit_request(
    request: ObserveDepositRequest,
) -> AppResult<ObserveDepositRequest> {
    if request.amount <= 0 {
        return Err(AppError::Validation(
            "deposit amount must be positive".to_owned(),
        ));
    }
    let address = normalize_chain_identifier(request.address, "address")?;
    let tx_hash = normalize_chain_identifier(request.tx_hash, "tx_hash")?;
    Ok(ObserveDepositRequest {
        asset_symbol: normalize_asset_symbol(&request.asset_symbol)?,
        network: normalize_deposit_network(&request.network)?,
        address,
        memo: optional_string(request.memo),
        tx_hash,
        event_index: request.event_index,
        amount: request.amount,
        block_height: request.block_height,
        confirmations: request.confirmations,
    })
}

/// 判定数据库错误是否为唯一键冲突，用于把并发重放识别成可回读旧申请而非直接失败。
/// 通过错误码判断，同时接受重复键专用码与完整性约束通用码；非数据库错误一律返回否。
/// 仅在提现创建路径使用：命中后回读同幂等键的既有申请并重新核对参数，绝不重复冻结资金。
fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        matches!(database_error.code().as_deref(), Some("1062" | "23000"))
    })
}

/// 归一充值地址申请的资产代码与网络，两者都是必填项，任一格式非法即在查库前终止。
/// 资产走大写与字符集校验，网络按别名表收敛，从而保证后续地址组匹配与既有分配复用使用同一口径。
/// 该函数不判断资产是否开放充值、网络是否启用，也不检查两者组合是否被允许。
fn normalize_deposit_address_request(
    request: DepositAddressRequest,
) -> AppResult<DepositAddressRequest> {
    let asset_symbol = normalize_asset_symbol(&request.asset_symbol)?;
    let network = normalize_deposit_network(&request.network)?;
    Ok(DepositAddressRequest {
        asset_symbol,
        network,
    })
}

/// 裁剪可空文本字段并把纯空白归一为缺省，专供充值备注这类参与身份比对的可选字段使用。
/// 与查询参数归一实现相同但语义不同：这里的目的是让空备注与无备注在幂等比对中被视作同一取值。
fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_application_tests.rs"]
mod tests;
