//! 抵押贷健康扫描的幂等清算与平台会计闭环。
//!
//! 当前工程没有把抵押物送往外部交易场所卖出的执行通道，因此清算采用明确的
//! platform_collateral_clearing_no_external_sale 模式：用户冻结抵押物中用于覆盖债务的部分
//! 转入平台抵押品库存，剩余部分退回可用余额；按权威价格得到的名义回收额与坏账额分别入账。
//! 清算结果、用户钱包流水、平台抵押腿、贷款应收关闭腿、坏账腿和订单终态全部位于同一 MySQL 事务。
//! 订单行始终先以 FOR UPDATE 锁定，因此还款、逾期推进和并发扫描最终只会赢得一个终态。

use crate::{
    error::{AppError, AppResult},
    modules::{
        loan::{
            domain::{
                LOAN_TYPE_COLLATERALIZED, STATUS_DISBURSED, STATUS_LIQUIDATED, STATUS_OVERDUE,
            },
            infrastructure::lock_loan_asset_precisions_in_order,
            oracle::{LoanOraclePrice, ensure_loan_oracle_observation_fresh},
            service::{
                calculate_interest_amount, calculate_loan_ltv, ensure_amount_precision,
                validate_loan_ltv_thresholds,
            },
        },
        wallet::truncate_amount_to_asset_precision,
    },
};
use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

const LIQUIDATION_CONTEXT: &str = "loan_liquidation";
const LIQUIDATION_MODE: &str = "platform_collateral_clearing_no_external_sale";

/// 单轮扫描所需的不可变订单行情配置；实际动账前仍会锁行并逐字段复核。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LoanLiquidationCandidate {
    pub order_id: u64,
    pub oracle_symbol: Option<String>,
    pub oracle_source: Option<String>,
    pub oracle_max_age_seconds: Option<u64>,
}

/// 清算表的完整对账结果，也是并发重放时返回的原始结算快照。
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct LoanLiquidationRecord {
    pub order_id: u64,
    pub transaction_key: String,
    pub user_id: u64,
    pub loan_asset_id: u64,
    pub collateral_asset_id: u64,
    pub oracle_symbol: String,
    pub oracle_source: String,
    pub oracle_price: BigDecimal,
    pub oracle_observed_at: DateTime<Utc>,
    pub ltv: BigDecimal,
    pub principal_amount: BigDecimal,
    pub interest_amount: BigDecimal,
    pub debt_amount: BigDecimal,
    pub collateral_seized: BigDecimal,
    pub collateral_returned: BigDecimal,
    pub recovered_amount: BigDecimal,
    pub bad_debt_amount: BigDecimal,
}

/// 单笔候选的幂等处理结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoanLiquidationOutcome {
    /// 本事务完成了第一次清算。
    Liquidated(LoanLiquidationRecord),
    /// 订单此前已清算，返回唯一结果且未写第二份资金腿。
    AlreadyLiquidated(LoanLiquidationRecord),
    /// 价格有效但 LTV 尚未达到清算线。
    NotRequired { ltv: BigDecimal },
    /// 还款或其他终态赢得了订单锁，本轮没有副作用。
    SkippedTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoanCollateralSettlementAmounts {
    pub(crate) collateral_seized: BigDecimal,
    pub(crate) collateral_returned: BigDecimal,
    pub(crate) recovered_amount: BigDecimal,
    pub(crate) bad_debt_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedLiquidationOrder {
    id: u64,
    user_id: u64,
    loan_type: String,
    loan_asset_id: u64,
    principal_amount: BigDecimal,
    interest_rate: BigDecimal,
    interest_calculation_mode: String,
    term_days: u32,
    collateral_asset_id: Option<u64>,
    collateral_amount: Option<BigDecimal>,
    initial_ltv: Option<BigDecimal>,
    maintenance_ltv: Option<BigDecimal>,
    liquidation_ltv: Option<BigDecimal>,
    oracle_symbol: Option<String>,
    oracle_source: Option<String>,
    oracle_max_age_seconds: Option<u64>,
    status: String,
    disbursed_at: Option<DateTime<Utc>>,
    collateral_released_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

/// 事务内以 SKIP LOCKED 领取最久未检查的抵押贷并刷新游标；多实例不会长期争抢同一批头部订单。
/// 配置缺失的历史债务也会轮转进入扫描并显式计为失败，不会永久饿死后续健康订单。
pub async fn fetch_loan_liquidation_candidates(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<LoanLiquidationCandidate>> {
    let mut tx = pool.begin().await?;
    let candidates = sqlx::query_as::<_, LoanLiquidationCandidate>(
        r#"SELECT id AS order_id, oracle_symbol, oracle_source, oracle_max_age_seconds
           FROM loan_orders
           WHERE loan_type = 'collateralized'
             AND status IN ('disbursed', 'overdue')
             AND collateral_released_at IS NULL
           ORDER BY health_checked_at ASC, id ASC
           LIMIT ?
           FOR UPDATE SKIP LOCKED"#,
    )
    .bind(i64::from(limit.clamp(1, 1_000)))
    .fetch_all(&mut *tx)
    .await?;
    for candidate in &candidates {
        sqlx::query("UPDATE loan_orders SET health_checked_at = CURRENT_TIMESTAMP(6) WHERE id = ?")
            .bind(candidate.order_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(candidates)
}

/// 对一笔订单执行“检查并在需要时清算”；传入 ticker 必须已经由借贷 oracle 适配器校验。
pub async fn liquidate_loan_order_if_required(
    pool: &Pool<MySql>,
    order_id: u64,
    ticker: &LoanOraclePrice,
    now: DateTime<Utc>,
) -> AppResult<LoanLiquidationOutcome> {
    let mut tx = pool.begin().await?;
    let order = lock_liquidation_order(&mut tx, order_id).await?;
    if order.status == STATUS_LIQUIDATED {
        let record = load_liquidation_record_in_tx(&mut tx, order.id).await?;
        tx.commit().await?;
        return Ok(LoanLiquidationOutcome::AlreadyLiquidated(record));
    }
    if order.status != STATUS_DISBURSED && order.status != STATUS_OVERDUE {
        tx.rollback().await?;
        return Ok(LoanLiquidationOutcome::SkippedTerminal);
    }
    if order.loan_type != LOAN_TYPE_COLLATERALIZED || order.collateral_released_at.is_some() {
        return Err(AppError::Conflict(
            "loan order is not eligible for collateral liquidation".to_owned(),
        ));
    }

    let collateral_asset_id = order.collateral_asset_id.ok_or_else(|| {
        AppError::Internal("collateralized loan order is missing collateral_asset_id".to_owned())
    })?;
    let collateral_amount = order.collateral_amount.clone().ok_or_else(|| {
        AppError::Internal("collateralized loan order is missing collateral_amount".to_owned())
    })?;
    let (_, _, liquidation_ltv) = validate_loan_ltv_thresholds(
        &order.loan_type,
        order.initial_ltv.clone(),
        order.maintenance_ltv.clone(),
        order.liquidation_ltv.clone(),
    )?
    .ok_or_else(|| {
        AppError::Internal("collateralized loan order is missing LTV snapshots".to_owned())
    })?;
    let disbursed_at = order.disbursed_at.ok_or_else(|| {
        AppError::Internal("disbursed loan order is missing disbursed_at".to_owned())
    })?;
    let asset_precisions =
        lock_loan_asset_precisions_in_order(&mut tx, [order.loan_asset_id, collateral_asset_id])
            .await?;
    let loan_precision = asset_precisions
        .iter()
        .find_map(|(asset_id, precision)| (*asset_id == order.loan_asset_id).then_some(*precision))
        .ok_or_else(|| AppError::Internal("locked loan asset precision is missing".to_owned()))?;
    let collateral_precision = asset_precisions
        .iter()
        .find_map(|(asset_id, precision)| (*asset_id == collateral_asset_id).then_some(*precision))
        .ok_or_else(|| {
            AppError::Internal("locked collateral asset precision is missing".to_owned())
        })?;
    let collateral_wallet =
        lock_collateral_wallet_in_tx(&mut tx, &order, collateral_asset_id).await?;
    // 等待订单及钱包行锁的时间都计入行情年龄和利息，旧 ticker 不得在事务尾部继续触发资金处置。
    let decision_now = now.max(Utc::now());
    ensure_ticker_matches_locked_order(&order, ticker, decision_now)?;
    let interest_amount = calculate_interest_amount(
        &order.principal_amount,
        &order.interest_rate,
        &order.interest_calculation_mode,
        order.term_days,
        disbursed_at,
        decision_now,
        loan_precision,
    )?;
    let debt_amount = truncate_amount_to_asset_precision(
        &(order.principal_amount.clone() + interest_amount.clone()),
        loan_precision,
    );
    let ltv = calculate_loan_ltv(&debt_amount, &collateral_amount, &ticker.price)?;
    if ltv < liquidation_ltv {
        tx.rollback().await?;
        return Ok(LoanLiquidationOutcome::NotRequired { ltv });
    }

    let settlement = calculate_loan_collateral_settlement_amounts(
        &collateral_amount,
        &ticker.price,
        &debt_amount,
        loan_precision,
        collateral_precision,
    )?;
    let collateral_seized = settlement.collateral_seized;
    let collateral_returned = settlement.collateral_returned;
    let recovered_amount = settlement.recovered_amount;
    let bad_debt_amount = settlement.bad_debt_amount;
    let transaction_key = format!("loan_liquidation:{}", order.id);

    apply_collateral_liquidation_wallet_in_tx(
        &mut tx,
        &order,
        collateral_asset_id,
        &collateral_amount,
        &collateral_returned,
        collateral_wallet,
    )
    .await?;
    insert_liquidation_record_in_tx(
        &mut tx,
        &order,
        collateral_asset_id,
        ticker,
        &ltv,
        &interest_amount,
        &debt_amount,
        &collateral_seized,
        &collateral_returned,
        &recovered_amount,
        &bad_debt_amount,
        &transaction_key,
    )
    .await?;
    insert_liquidation_platform_journal_in_tx(
        &mut tx,
        &order,
        collateral_asset_id,
        &interest_amount,
        &collateral_seized,
        &recovered_amount,
        &bad_debt_amount,
        &transaction_key,
    )
    .await?;
    let updated = sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'liquidated',
               interest_amount = ?,
               repayment_amount = ?,
               liquidated_at = ?,
               collateral_released_at = ?
           WHERE id = ? AND status IN ('disbursed', 'overdue')"#,
    )
    .bind(&interest_amount)
    .bind(&recovered_amount)
    .bind(decision_now.naive_utc())
    .bind(decision_now.naive_utc())
    .bind(order.id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "loan order terminal state changed during liquidation".to_owned(),
        ));
    }
    let record = load_liquidation_record_in_tx(&mut tx, order.id).await?;
    tx.commit().await?;
    Ok(LoanLiquidationOutcome::Liquidated(record))
}

/// 以贷款资产精度估值；抵押充足时向上量化处置数量，防止因向下舍入凭空产生坏账。
pub(crate) fn calculate_loan_collateral_settlement_amounts(
    collateral_amount: &BigDecimal,
    oracle_price: &BigDecimal,
    debt_amount: &BigDecimal,
    loan_precision: i32,
    collateral_precision: i32,
) -> AppResult<LoanCollateralSettlementAmounts> {
    if collateral_amount <= &BigDecimal::from(0)
        || oracle_price <= &BigDecimal::from(0)
        || debt_amount <= &BigDecimal::from(0)
    {
        return Err(AppError::Validation(
            "loan liquidation amounts and oracle price must be positive".to_owned(),
        ));
    }
    ensure_amount_precision(collateral_amount, collateral_precision, "collateral_amount")?;
    ensure_amount_precision(debt_amount, loan_precision, "debt_amount")?;
    let exact_collateral_value = collateral_amount.clone() * oracle_price.clone();
    let collateral_seized = if exact_collateral_value <= *debt_amount {
        collateral_amount.clone()
    } else {
        (debt_amount.clone() / oracle_price.clone())
            .with_scale_round(i64::from(collateral_precision), RoundingMode::Up)
            .min(collateral_amount.clone())
    };
    let collateral_returned = collateral_amount.clone() - collateral_seized.clone();
    let recovered_amount = truncate_amount_to_asset_precision(
        &(collateral_seized.clone() * oracle_price.clone()),
        loan_precision,
    )
    .min(debt_amount.clone());
    let bad_debt_amount = debt_amount.clone() - recovered_amount.clone();
    Ok(LoanCollateralSettlementAmounts {
        collateral_seized,
        collateral_returned,
        recovered_amount,
        bad_debt_amount,
    })
}

async fn lock_liquidation_order(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<LockedLiquidationOrder> {
    sqlx::query_as::<_, LockedLiquidationOrder>(
        r#"SELECT id, user_id, loan_type, asset_id AS loan_asset_id,
                  amount AS principal_amount, interest_rate, interest_calculation_mode,
                  term_days, collateral_asset_id, collateral_amount,
                  initial_ltv, maintenance_ltv, liquidation_ltv,
                  oracle_symbol, oracle_source, oracle_max_age_seconds, status,
                  disbursed_at, collateral_released_at
           FROM loan_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

fn ensure_ticker_matches_locked_order(
    order: &LockedLiquidationOrder,
    ticker: &LoanOraclePrice,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let symbol = order.oracle_symbol.as_deref().ok_or_else(|| {
        AppError::Internal("collateralized loan order is missing oracle_symbol".to_owned())
    })?;
    let source = order.oracle_source.as_deref().ok_or_else(|| {
        AppError::Internal("collateralized loan order is missing oracle_source".to_owned())
    })?;
    let max_age_seconds = order.oracle_max_age_seconds.ok_or_else(|| {
        AppError::Internal("collateralized loan order is missing oracle_max_age_seconds".to_owned())
    })?;
    if ticker.symbol != symbol || ticker.source != source {
        return Err(AppError::Conflict(
            "loan oracle ticker does not match locked order snapshot".to_owned(),
        ));
    }
    if ticker.price <= 0 {
        return Err(AppError::Validation(
            "loan liquidation requires a positive oracle ticker".to_owned(),
        ));
    }
    ensure_amount_precision(&ticker.price, 18, "loan oracle price")?;
    ensure_loan_oracle_observation_fresh(ticker.observed_at, max_age_seconds, now)
}

async fn lock_collateral_wallet_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &LockedLiquidationOrder,
    collateral_asset_id: u64,
) -> AppResult<WalletRow> {
    sqlx::query_as::<_, WalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order.user_id)
    .bind(collateral_asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Conflict("loan collateral wallet is missing".to_owned()))
}

async fn apply_collateral_liquidation_wallet_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &LockedLiquidationOrder,
    collateral_asset_id: u64,
    collateral_amount: &BigDecimal,
    collateral_returned: &BigDecimal,
    wallet: WalletRow,
) -> AppResult<()> {
    if wallet.frozen < *collateral_amount {
        return Err(AppError::Conflict(
            "loan collateral frozen balance is insufficient for liquidation".to_owned(),
        ));
    }
    let available_after = wallet.available + collateral_returned.clone();
    let frozen_after = wallet.frozen - collateral_amount.clone();
    sqlx::query(
        r#"UPDATE wallet_accounts
           SET available = ?, frozen = ?
           WHERE user_id = ? AND asset_id = ?"#,
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(order.user_id)
    .bind(collateral_asset_id)
    .execute(&mut **tx)
    .await?;

    insert_liquidation_wallet_ledger(
        tx,
        order,
        collateral_asset_id,
        -collateral_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
    )
    .await?;
    if collateral_returned > &BigDecimal::from(0) {
        insert_liquidation_wallet_ledger(
            tx,
            order,
            collateral_asset_id,
            collateral_returned.clone(),
            "available",
            &available_after,
            &available_after,
            &frozen_after,
            &wallet.locked,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_liquidation_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    order: &LockedLiquidationOrder,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'loan_collateral_liquidation', ?, ?, ?, ?, ?, ?,
                   'loan_liquidation', ?)"#,
    )
    .bind(order.user_id)
    .bind(asset_id)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(order.id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_liquidation_record_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &LockedLiquidationOrder,
    collateral_asset_id: u64,
    ticker: &LoanOraclePrice,
    ltv: &BigDecimal,
    interest_amount: &BigDecimal,
    debt_amount: &BigDecimal,
    collateral_seized: &BigDecimal,
    collateral_returned: &BigDecimal,
    recovered_amount: &BigDecimal,
    bad_debt_amount: &BigDecimal,
    transaction_key: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO loan_liquidations
           (order_id, transaction_key, user_id, loan_asset_id, collateral_asset_id,
            oracle_symbol, oracle_source, oracle_price, oracle_observed_at, ltv,
            principal_amount, interest_amount, debt_amount, collateral_seized,
            collateral_returned, recovered_amount, bad_debt_amount, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'completed')"#,
    )
    .bind(order.id)
    .bind(transaction_key)
    .bind(order.user_id)
    .bind(order.loan_asset_id)
    .bind(collateral_asset_id)
    .bind(&ticker.symbol)
    .bind(&ticker.source)
    .bind(&ticker.price)
    .bind(ticker.observed_at.naive_utc())
    .bind(ltv)
    .bind(&order.principal_amount)
    .bind(interest_amount)
    .bind(debt_amount)
    .bind(collateral_seized)
    .bind(collateral_returned)
    .bind(recovered_amount)
    .bind(bad_debt_amount)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_liquidation_platform_journal_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &LockedLiquidationOrder,
    collateral_asset_id: u64,
    interest_amount: &BigDecimal,
    collateral_seized: &BigDecimal,
    recovered_amount: &BigDecimal,
    bad_debt_amount: &BigDecimal,
    transaction_key: &str,
) -> AppResult<()> {
    if collateral_seized > &BigDecimal::from(0) {
        insert_platform_journal_leg(
            tx,
            transaction_key,
            "user_collateral_seized",
            collateral_asset_id,
            -collateral_seized.clone(),
            order,
        )
        .await?;
        insert_platform_journal_leg(
            tx,
            transaction_key,
            "platform_collateral_inventory",
            collateral_asset_id,
            collateral_seized.clone(),
            order,
        )
        .await?;
    }
    insert_platform_journal_leg(
        tx,
        transaction_key,
        "loan_principal_receivable_close",
        order.loan_asset_id,
        -order.principal_amount.clone(),
        order,
    )
    .await?;
    if interest_amount > &BigDecimal::from(0) {
        insert_platform_journal_leg(
            tx,
            transaction_key,
            "loan_interest_receivable_close",
            order.loan_asset_id,
            -interest_amount.clone(),
            order,
        )
        .await?;
    }
    if recovered_amount > &BigDecimal::from(0) {
        insert_platform_journal_leg(
            tx,
            transaction_key,
            "platform_collateral_clearing_recovery",
            order.loan_asset_id,
            recovered_amount.clone(),
            order,
        )
        .await?;
    }
    if bad_debt_amount > &BigDecimal::from(0) {
        insert_platform_journal_leg(
            tx,
            transaction_key,
            "platform_bad_debt_expense",
            order.loan_asset_id,
            bad_debt_amount.clone(),
            order,
        )
        .await?;
    }
    Ok(())
}

async fn insert_platform_journal_leg(
    tx: &mut Transaction<'_, MySql>,
    transaction_key: &str,
    account_code: &str,
    asset_id: u64,
    amount: BigDecimal,
    order: &LockedLiquidationOrder,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO platform_financial_journal
           (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id,
            metadata_json)
           VALUES (?, ?, ?, ?, ?, 'loan_order', ?, ?)"#,
    )
    .bind(transaction_key)
    .bind(LIQUIDATION_CONTEXT)
    .bind(account_code)
    .bind(asset_id)
    .bind(amount)
    .bind(order.id.to_string())
    .bind(SqlxJson(json!({
        "user_id": order.user_id,
        "mode": LIQUIDATION_MODE,
    })))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_liquidation_record_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<LoanLiquidationRecord> {
    sqlx::query_as::<_, LoanLiquidationRecord>(
        r#"SELECT order_id, transaction_key, user_id, loan_asset_id, collateral_asset_id,
                  oracle_symbol, oracle_source, oracle_price, oracle_observed_at, ltv,
                  principal_amount, interest_amount, debt_amount, collateral_seized,
                  collateral_returned, recovered_amount, bad_debt_amount
           FROM loan_liquidations
           WHERE order_id = ?
           LIMIT 1"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Internal("liquidated loan order is missing liquidation record".to_owned())
    })
}
