//! wallet bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use super::{
    BalanceBucket, LedgerBatch, LockPosition, WalletAccount, WalletLedgerEntry, WalletRepository,
    WalletServiceError, WithdrawFeeTier, calculate_withdraw_fee, normalize_withdraw_fee_tiers,
};
use crate::{
    error::{AppError, AppResult},
    modules::wallet::presentation::{
        DepositAddressResponse, DepositAssetResponse, DepositNetworkResponse,
        WalletAccountResponse, WalletLedgerEntryResponse, WalletLedgerPageResponse,
        WalletLedgerResponse,
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

#[derive(Debug, Clone)]
pub struct NewAssetLockPosition {
    pub user_id: u64,
    pub asset_id: u64,
    pub unlock_type: String,
    pub unlock_at: chrono::DateTime<chrono::Utc>,
    pub locked_amount: BigDecimal,
    pub remaining_amount: BigDecimal,
    pub merge_key: String,
    pub sources: Vec<NewAssetLockPositionSource>,
}

#[derive(Debug, Clone)]
pub struct NewAssetLockPositionSource {
    pub source_type: String,
    pub source_id: String,
    pub source_amount: BigDecimal,
    pub source_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct MySqlWalletRepository {
    pool: Pool<MySql>,
}

impl MySqlWalletRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    pub async fn get_or_create_account_async(
        &self,
        user_id: u64,
        asset_id: u64,
    ) -> Result<WalletAccount, WalletServiceError> {
        // SQL 细节已下沉到 infrastructure，仓储对象专注持久化编排。
        get_or_create_account_async(&self.pool, user_id, asset_id).await
    }

    pub async fn load_account_async(
        &self,
        user_id: u64,
        asset_id: u64,
    ) -> Result<Option<WalletAccount>, WalletServiceError> {
        load_account_async(&self.pool, user_id, asset_id).await
    }

    pub async fn save_account_with_ledger_async(
        &self,
        account: WalletAccount,
        ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError> {
        // 与领域服务共享账务规则前置条件：真正的写库逻辑在这里执行。
        save_account_with_ledger_async(&self.pool, account, ledger).await
    }

    pub async fn list_ledger_by_ref_async(
        &self,
        ref_type: &str,
        ref_id: &str,
    ) -> Result<Vec<WalletLedgerEntry>, WalletServiceError> {
        // 基础设施返回持久化后的领域实体，供领域服务消费。
        list_ledger_by_ref_async(&self.pool, ref_type, ref_id).await
    }

    pub async fn insert_asset_lock_positions_async(
        &self,
        positions: Vec<NewAssetLockPosition>,
    ) -> Result<Vec<u64>, WalletServiceError> {
        // 锁仓来源与冻结量更新都在基础设施层做幂等落库，保障并发安全。
        insert_asset_lock_positions_async(&self.pool, positions).await
    }

    pub async fn count_lock_position_sources_async(
        &self,
        lock_position_id: u64,
    ) -> Result<u64, WalletServiceError> {
        // 仅作为仓储统计查询，不在领域层拼 SQLx。
        count_lock_position_sources_async(&self.pool, lock_position_id).await
    }
}

#[async_trait]
impl WalletRepository for MySqlWalletRepository {
    fn load_account(
        &mut self,
        _user_id: &str,
        _asset_id: &str,
    ) -> Result<WalletAccount, WalletServiceError> {
        Err(WalletServiceError::Repository(
            "MySqlWalletRepository requires async SQLx methods".to_owned(),
        ))
    }

    fn save_account_with_ledger(
        &mut self,
        _account: WalletAccount,
        _ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError> {
        Err(WalletServiceError::Repository(
            "MySqlWalletRepository requires async SQLx methods".to_owned(),
        ))
    }

    fn insert_lock_positions(
        &mut self,
        _positions: Vec<LockPosition>,
    ) -> Result<(), WalletServiceError> {
        Err(WalletServiceError::Repository(
            "MySqlWalletRepository requires async SQLx methods".to_owned(),
        ))
    }
}

#[derive(Debug, sqlx::FromRow)]
struct WalletAccountRow {
    user_id: u64,
    asset_id: u64,
    symbol: String,
    logo_url: Option<String>,
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletLedgerEntryRow {
    id: u64,
    user_id: u64,
    asset_id: u64,
    symbol: String,
    change_type: String,
    amount: BigDecimal,
    balance_type: String,
    balance_after: BigDecimal,
    available_after: BigDecimal,
    frozen_after: BigDecimal,
    locked_after: BigDecimal,
    fee: BigDecimal,
    ref_type: String,
    ref_id: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct DepositNetworkRow {
    network: String,
    display_name: String,
    address_group_code: String,
    address_group_name: Option<String>,
    asset_symbols: SqlxJson<Vec<String>>,
}

#[derive(Debug, sqlx::FromRow)]
struct DepositAddressRow {
    id: u64,
    asset_symbol: String,
    network: String,
    address: String,
    memo: Option<String>,
    assigned_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct DepositAssetRow {
    symbol: String,
    name: String,
    logo_url: Option<String>,
    precision_scale: i32,
    deposit_enabled: bool,
    withdraw_enabled: bool,
    min_deposit_amount: BigDecimal,
    deposit_fee: BigDecimal,
    withdraw_fee: BigDecimal,
    withdraw_fee_tiers: SqlxJson<Vec<WithdrawFeeTier>>,
}

#[derive(Debug)]
pub(crate) struct WalletLedgerFilter {
    pub(crate) asset_id: Option<u64>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) change_type: Option<String>,
    pub(crate) ref_type: Option<String>,
    pub(crate) ref_id: Option<String>,
    pub(crate) start_time: Option<String>,
    pub(crate) end_time: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

pub(crate) async fn list_deposit_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    let rows = sqlx::query_as::<_, DepositAssetRow>(&deposit_assets_sql(true))
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(deposit_asset_response).collect())
}

pub(crate) async fn list_withdraw_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    let rows = sqlx::query_as::<_, DepositAssetRow>(&deposit_assets_sql(false))
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(deposit_asset_response).collect())
}

pub(crate) async fn get_or_create_account_async(
    pool: &Pool<MySql>,
    user_id: u64,
    asset_id: u64,
) -> Result<WalletAccount, WalletServiceError> {
    // 下沉 SQL：创建不存在的钱包账户，已存在则保持幂等。
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(pool)
    .await
    .map_err(map_wallet_sqlx_error)?;

    load_account_async(pool, user_id, asset_id)
        .await?
        .ok_or_else(|| WalletServiceError::Repository("wallet account was not created".to_owned()))
}

pub(crate) async fn load_account_async(
    pool: &Pool<MySql>,
    user_id: u64,
    asset_id: u64,
) -> Result<Option<WalletAccount>, WalletServiceError> {
    let row = sqlx::query_as::<_, (u64, u64, BigDecimal, BigDecimal, BigDecimal)>(
        r#"SELECT user_id, asset_id, available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(pool)
    .await
    .map_err(map_wallet_sqlx_error)?;

    Ok(row.map(wallet_account_from_row))
}

pub(crate) async fn save_account_with_ledger_async(
    pool: &Pool<MySql>,
    account: WalletAccount,
    ledger: LedgerBatch,
) -> Result<(), WalletServiceError> {
    let user_id = parse_u64_identifier("user_id", &account.user_id)?;
    let asset_id = parse_u64_identifier("asset_id", &account.asset_id)?;
    let mut tx = pool.begin().await.map_err(map_wallet_sqlx_error)?;

    sqlx::query(
        r#"UPDATE wallet_accounts
           SET available = ?, frozen = ?, locked = ?
           WHERE user_id = ? AND asset_id = ?"#,
    )
    .bind(&account.available)
    .bind(&account.frozen)
    .bind(&account.locked)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut *tx)
    .await
    .map_err(map_wallet_sqlx_error)?;

    for entry in ledger.into_entries() {
        let parsed_user_id = parse_u64_identifier("ledger.user_id", &entry.user_id)?;
        let parsed_asset_id = parse_u64_identifier("ledger.asset_id", &entry.asset_id)?;
        sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(parsed_user_id)
        .bind(parsed_asset_id)
        .bind(entry.change_type)
        .bind(entry.amount)
        .bind(balance_bucket_as_str(entry.balance_type))
        .bind(entry.balance_after)
        .bind(entry.available_after)
        .bind(entry.frozen_after)
        .bind(entry.locked_after)
        .bind(entry.ref_type)
        .bind(entry.ref_id)
        .execute(&mut *tx)
        .await
        .map_err(map_wallet_sqlx_error)?;
    }

    tx.commit().await.map_err(map_wallet_sqlx_error)
}

pub(crate) async fn list_ledger_by_ref_async(
    pool: &Pool<MySql>,
    ref_type: &str,
    ref_id: &str,
) -> Result<Vec<WalletLedgerEntry>, WalletServiceError> {
    let rows = sqlx::query_as::<
        _,
        (
            u64,
            u64,
            String,
            BigDecimal,
            String,
            BigDecimal,
            BigDecimal,
            BigDecimal,
            BigDecimal,
            String,
            String,
        ),
    >(
        r#"SELECT user_id, asset_id, change_type, amount, balance_type, balance_after,
                  available_after, frozen_after, locked_after, ref_type, ref_id
           FROM wallet_ledger
           WHERE ref_type = ? AND ref_id = ?
           ORDER BY id ASC"#,
    )
    .bind(ref_type)
    .bind(ref_id)
    .fetch_all(pool)
    .await
    .map_err(map_wallet_sqlx_error)?;

    rows.into_iter().map(wallet_ledger_from_row).collect()
}

pub(crate) async fn insert_asset_lock_positions_async(
    pool: &Pool<MySql>,
    positions: Vec<NewAssetLockPosition>,
) -> Result<Vec<u64>, WalletServiceError> {
    let mut tx = pool.begin().await.map_err(map_wallet_sqlx_error)?;
    let mut ids = Vec::with_capacity(positions.len());

    for position in positions {
        let position_id = insert_asset_lock_position_in_tx(&mut tx, position).await?;
        ids.push(position_id);
    }

    tx.commit().await.map_err(map_wallet_sqlx_error)?;
    Ok(ids)
}

pub(crate) async fn count_lock_position_sources_async(
    pool: &Pool<MySql>,
    lock_position_id: u64,
) -> Result<u64, WalletServiceError> {
    let (count,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM asset_lock_position_sources WHERE lock_position_id = ?",
    )
    .bind(lock_position_id)
    .fetch_one(pool)
    .await
    .map_err(map_wallet_sqlx_error)?;

    Ok(count as u64)
}

async fn insert_asset_lock_position_in_tx(
    tx: &mut Transaction<'_, MySql>,
    position: NewAssetLockPosition,
) -> Result<u64, WalletServiceError> {
    let result = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount,
            remaining_amount, merge_key, status)
           VALUES (?, ?, ?, ?, 0, 0, ?, 'active')
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(position.user_id)
    .bind(position.asset_id)
    .bind(&position.unlock_type)
    .bind(position.unlock_at.naive_utc())
    .bind(&position.merge_key)
    .execute(&mut **tx)
    .await
    .map_err(map_wallet_sqlx_error)?;

    let position_id = if result.last_insert_id() == 0 {
        sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_lock_positions WHERE merge_key = ? LIMIT 1",
        )
        .bind(&position.merge_key)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_wallet_sqlx_error)?
        .0
    } else {
        result.last_insert_id()
    };

    for source in position.sources {
        let inserted = sqlx::query(
            r#"INSERT IGNORE INTO asset_lock_position_sources
               (lock_position_id, source_type, source_id, source_amount, source_time)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(position_id)
        .bind(&source.source_type)
        .bind(&source.source_id)
        .bind(&source.source_amount)
        .bind(source.source_time.naive_utc())
        .execute(&mut **tx)
        .await
        .map_err(map_wallet_sqlx_error)?
        .rows_affected();

        if inserted > 0 {
            sqlx::query(
                r#"UPDATE asset_lock_positions
                   SET locked_amount = locked_amount + ?,
                       remaining_amount = remaining_amount + ?
                   WHERE id = ?"#,
            )
            .bind(&source.source_amount)
            .bind(&source.source_amount)
            .bind(position_id)
            .execute(&mut **tx)
            .await
            .map_err(map_wallet_sqlx_error)?;
        }
    }

    Ok(position_id)
}

fn wallet_account_from_row(row: (u64, u64, BigDecimal, BigDecimal, BigDecimal)) -> WalletAccount {
    let (user_id, asset_id, available, frozen, locked) = row;
    WalletAccount {
        user_id: user_id.to_string(),
        asset_id: asset_id.to_string(),
        available,
        frozen,
        locked,
    }
}

fn wallet_ledger_from_row(
    row: (
        u64,
        u64,
        String,
        BigDecimal,
        String,
        BigDecimal,
        BigDecimal,
        BigDecimal,
        BigDecimal,
        String,
        String,
    ),
) -> Result<WalletLedgerEntry, WalletServiceError> {
    let (
        user_id,
        asset_id,
        change_type,
        amount,
        balance_type,
        balance_after,
        available_after,
        frozen_after,
        locked_after,
        ref_type,
        ref_id,
    ) = row;

    Ok(WalletLedgerEntry {
        user_id: user_id.to_string(),
        asset_id: asset_id.to_string(),
        change_type,
        amount,
        balance_type: balance_bucket_from_str(&balance_type)?,
        balance_after,
        available_after,
        frozen_after,
        locked_after,
        ref_type,
        ref_id,
    })
}

fn balance_bucket_as_str(bucket: BalanceBucket) -> &'static str {
    match bucket {
        BalanceBucket::Available => "available",
        BalanceBucket::Frozen => "frozen",
        BalanceBucket::Locked => "locked",
    }
}

fn balance_bucket_from_str(value: &str) -> Result<BalanceBucket, WalletServiceError> {
    match value {
        "available" => Ok(BalanceBucket::Available),
        "frozen" => Ok(BalanceBucket::Frozen),
        "locked" => Ok(BalanceBucket::Locked),
        _ => Err(WalletServiceError::Repository(format!(
            "unknown wallet ledger balance_type: {value}"
        ))),
    }
}

fn parse_u64_identifier(field: &str, value: &str) -> Result<u64, WalletServiceError> {
    value.parse::<u64>().map_err(|error| {
        WalletServiceError::Repository(format!("invalid numeric {field} `{value}`: {error}"))
    })
}

fn map_wallet_sqlx_error(error: sqlx::Error) -> WalletServiceError {
    WalletServiceError::Repository(error.to_string())
}

pub(crate) async fn list_active_deposit_networks(
    pool: &Pool<MySql>,
    asset_symbol: Option<&str>,
) -> AppResult<Vec<DepositNetworkResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(deposit_networks_sql());
    if let Some(symbol) = asset_symbol {
        builder.push(
            " AND (asset_symbols_json IS NULL OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(",
        );
        builder.push_bind(symbol);
        builder.push(")))");
    }
    builder.push(" ORDER BY sort_order ASC, id ASC");
    let rows = builder
        .build_query_as::<DepositNetworkRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(deposit_network_response).collect())
}

pub(crate) async fn ensure_deposit_enabled_asset(
    pool: &Pool<MySql>,
    asset_symbol: &str,
) -> AppResult<()> {
    let deposit_enabled = sqlx::query_scalar::<_, bool>(
        "SELECT deposit_enabled FROM assets WHERE symbol = ? AND status = 'active' LIMIT 1",
    )
    .bind(asset_symbol)
    .fetch_optional(pool)
    .await?;
    match deposit_enabled {
        Some(true) => Ok(()),
        Some(false) => Err(AppError::Validation(
            "asset does not support deposit".to_owned(),
        )),
        None => Err(AppError::NotFound),
    }
}

pub(crate) async fn load_active_deposit_network_config(
    pool: &Pool<MySql>,
    network: &str,
    asset_symbol: &str,
) -> AppResult<DepositNetworkResponse> {
    let row = sqlx::query_as::<_, DepositNetworkRow>(
        r#"SELECT network,
                  display_name,
                  address_group_code,
                  address_group_name,
                  COALESCE(asset_symbols_json, JSON_ARRAY()) AS asset_symbols
           FROM deposit_network_configs
           WHERE network = ?
             AND status = 'active'
             AND (asset_symbols_json IS NULL OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(?)))
           LIMIT 1"#,
    )
    .bind(network)
    .bind(asset_symbol)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::Validation(format!(
            "asset {asset_symbol} does not support deposit network {network}"
        ))
    })?;
    Ok(deposit_network_response(row))
}

pub(crate) async fn load_user_deposit_address(
    pool: &Pool<MySql>,
    user_id: u64,
    asset_symbol: &str,
    address_group_code: &str,
    network: &str,
) -> AppResult<Option<DepositAddressResponse>> {
    let row = sqlx::query_as::<_, DepositAddressRow>(
        r#"SELECT id, assigned_asset_symbol AS asset_symbol, network, address, memo, assigned_at
           FROM deposit_address_pool
           WHERE assigned_user_id = ?
             AND assigned_asset_symbol = ?
             AND address_group_code = ?
             AND status = 'assigned'
           ORDER BY CASE WHEN network = ? THEN 0 ELSE 1 END, id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_symbol)
    .bind(address_group_code)
    .bind(network)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(deposit_address_response))
}

pub(crate) async fn lock_available_deposit_address(
    tx: &mut Transaction<'_, MySql>,
    asset_symbol: &str,
    address_group_code: &str,
    network: &str,
) -> AppResult<u64> {
    sqlx::query_scalar::<_, u64>(
        r#"SELECT id
           FROM deposit_address_pool
           WHERE address_group_code = ?
             AND status = 'available'
             AND (
                 (asset_symbols_json IS NULL AND (asset_symbol IS NULL OR asset_symbol = ?))
                 OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(?))
             )
           ORDER BY CASE WHEN network = ? THEN 0 ELSE 1 END,
             CASE
                 WHEN JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(?)) THEN 0
                 WHEN asset_symbol = ? THEN 1
                 ELSE 2
             END, id ASC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(address_group_code)
    .bind(asset_symbol)
    .bind(asset_symbol)
    .bind(network)
    .bind(asset_symbol)
    .bind(asset_symbol)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn load_user_email_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>("SELECT email FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn assign_deposit_address_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
    user_id: u64,
    user_email: Option<String>,
    asset_symbol: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE deposit_address_pool
           SET status = 'assigned',
               assigned_user_id = ?,
               assigned_user_email = ?,
               assigned_asset_symbol = ?,
               assigned_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(user_id)
    .bind(user_email)
    .bind(asset_symbol)
    .bind(address_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_deposit_address_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
) -> AppResult<DepositAddressResponse> {
    let row = sqlx::query_as::<_, DepositAddressRow>(
        r#"SELECT id, assigned_asset_symbol AS asset_symbol, network, address, memo, assigned_at
           FROM deposit_address_pool
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(address_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(deposit_address_response(row))
}

pub(crate) async fn load_asset_withdraw_fee(
    pool: &Pool<MySql>,
    asset_symbol: &str,
    amount: &BigDecimal,
) -> AppResult<BigDecimal> {
    let row = sqlx::query_as::<_, (bool, BigDecimal, i32, SqlxJson<Vec<WithdrawFeeTier>>)>(
        r#"SELECT withdraw_enabled,
                  withdraw_fee,
                  precision_scale,
                  COALESCE(withdraw_fee_tiers_json, JSON_ARRAY()) AS withdraw_fee_tiers
           FROM assets
           WHERE symbol = ? AND status = 'active'
           LIMIT 1"#,
    )
    .bind(asset_symbol)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((true, withdraw_fee, precision_scale, SqlxJson(tiers))) => {
            let normalized_tiers =
                normalize_withdraw_fee_tiers(tiers).map_err(AppError::Validation)?;
            Ok(calculate_withdraw_fee(
                amount,
                &withdraw_fee,
                &normalized_tiers,
                precision_scale,
            ))
        }
        Some((false, _, _, _)) => Err(AppError::Validation(
            "asset does not support withdraw".to_owned(),
        )),
        None => Err(AppError::NotFound),
    }
}

pub(crate) async fn insert_withdrawal_request(
    pool: &Pool<MySql>,
    user_id: u64,
    asset_symbol: &str,
    network: Option<&str>,
    address: &str,
    amount: &BigDecimal,
    fee: &BigDecimal,
    security_method: &str,
) -> AppResult<u64> {
    let status = "pending";
    let result = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
              (user_id, asset_symbol, network, address, amount, fee, status, security_method)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_symbol)
    .bind(network)
    .bind(address)
    .bind(amount)
    .bind(fee)
    .bind(status)
    .bind(security_method)
    .execute(pool)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn list_wallet_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<WalletAccountResponse>> {
    let rows = sqlx::query_as::<_, WalletAccountRow>(
        r#"SELECT wa.user_id, wa.asset_id, a.symbol, a.logo_url, wa.available, wa.frozen, wa.locked
           FROM wallet_accounts wa
           JOIN assets a ON a.id = wa.asset_id
           WHERE wa.user_id = ?
           ORDER BY a.symbol ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(wallet_account_response).collect())
}

pub(crate) async fn list_wallet_ledger(
    pool: &Pool<MySql>,
    user_id: u64,
    filter: WalletLedgerFilter,
) -> AppResult<WalletLedgerResponse> {
    let total = count_wallet_ledger(pool, user_id, &filter).await?;
    let mut builder = QueryBuilder::<MySql>::new(wallet_ledger_select_sql());
    builder.push_bind(user_id);
    push_wallet_ledger_filters(&mut builder, &filter);
    builder.push(" ORDER BY wl.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    let entries = builder
        .build_query_as::<WalletLedgerEntryRow>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(wallet_ledger_entry_response)
        .collect();
    let total_pages = if total == 0 {
        1
    } else {
        ((total + filter.limit as u64 - 1) / filter.limit as u64) as u32
    };

    Ok(WalletLedgerResponse {
        entries,
        page: WalletLedgerPageResponse {
            number: filter.offset / filter.limit,
            size: filter.limit,
            total_elements: total,
            total_pages,
        },
    })
}

async fn count_wallet_ledger(
    pool: &Pool<MySql>,
    user_id: u64,
    filter: &WalletLedgerFilter,
) -> AppResult<u64> {
    let mut count_builder = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM wallet_ledger wl
           JOIN assets a ON a.id = wl.asset_id
           WHERE wl.user_id = "#,
    );
    count_builder.push_bind(user_id);
    push_wallet_ledger_filters(&mut count_builder, filter);
    Ok(count_builder
        .build_query_scalar::<i64>()
        .fetch_one(pool)
        .await?
        .max(0) as u64)
}

fn push_wallet_ledger_filters<'args>(
    builder: &mut QueryBuilder<'args, MySql>,
    filter: &'args WalletLedgerFilter,
) {
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND wl.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(asset_symbol) = filter.asset_symbol.as_deref() {
        builder.push(" AND UPPER(a.symbol) = ");
        builder.push_bind(asset_symbol);
    }
    if let Some(change_type) = filter.change_type.as_deref() {
        builder.push(" AND wl.change_type = ");
        builder.push_bind(change_type);
    }
    if let Some(ref_type) = filter.ref_type.as_deref() {
        builder.push(" AND wl.ref_type = ");
        builder.push_bind(ref_type);
    }
    if let Some(ref_id) = filter.ref_id.as_deref() {
        builder.push(" AND wl.ref_id = ");
        builder.push_bind(ref_id);
    }
    if let Some(start_time) = filter.start_time.as_deref() {
        builder.push(" AND wl.created_at >= ");
        builder.push_bind(start_time);
    }
    if let Some(end_time) = filter.end_time.as_deref() {
        builder.push(" AND wl.created_at <= ");
        builder.push_bind(end_time);
    }
}

fn deposit_assets_sql(deposit_enabled: bool) -> String {
    let flag = if deposit_enabled {
        "deposit_enabled"
    } else {
        "withdraw_enabled"
    };
    format!(
        r#"SELECT symbol,
                  name,
                  logo_url,
                  precision_scale,
                  deposit_enabled,
                  withdraw_enabled,
                  min_deposit_amount,
                  deposit_fee,
                  withdraw_fee,
                  COALESCE(withdraw_fee_tiers_json, JSON_ARRAY()) AS withdraw_fee_tiers
           FROM assets
           WHERE status = 'active' AND {flag} = TRUE
           ORDER BY symbol ASC"#
    )
}

fn deposit_networks_sql() -> &'static str {
    r#"SELECT network,
              display_name,
              address_group_code,
              address_group_name,
              COALESCE(asset_symbols_json, JSON_ARRAY()) AS asset_symbols
       FROM deposit_network_configs
       WHERE status = 'active'"#
}

fn wallet_ledger_select_sql() -> &'static str {
    r#"SELECT wl.id, wl.user_id, wl.asset_id, a.symbol, wl.change_type, wl.amount,
              wl.balance_type, wl.balance_after, wl.available_after, wl.frozen_after,
              wl.locked_after,
              COALESCE(
                  CASE WHEN wl.ref_type = 'convert_order' THEN convert_orders.fee_amount END,
                  CASE WHEN wl.ref_type = 'spot_trade' THEN spot_trades.fee END,
                  CASE
                      WHEN wl.ref_type IN (
                          'wallet_withdrawal_request',
                          'wallet_withdrawal',
                          'withdrawal_request'
                      )
                      THEN wallet_withdrawal_requests.fee
                  END,
                  CASE
                      WHEN wl.ref_type IN ('withdraw_record', 'withdrawal_record', 'withdraw')
                      THEN withdraw_records.fee
                  END,
                  0
              ) AS fee,
              wl.ref_type, wl.ref_id, wl.created_at
       FROM wallet_ledger wl
       JOIN assets a ON a.id = wl.asset_id
       LEFT JOIN convert_orders
              ON wl.ref_type = 'convert_order'
             AND convert_orders.quote_id = wl.ref_id
             AND convert_orders.user_id = wl.user_id
             AND convert_orders.from_asset = wl.asset_id
       LEFT JOIN spot_trades
              ON wl.ref_type = 'spot_trade'
             AND spot_trades.buy_order_id = CAST(SUBSTRING_INDEX(wl.ref_id, ':', 1) AS UNSIGNED)
             AND spot_trades.sell_order_id = CAST(SUBSTRING_INDEX(wl.ref_id, ':', -1) AS UNSIGNED)
       LEFT JOIN wallet_withdrawal_requests
              ON wl.ref_type IN (
                     'wallet_withdrawal_request',
                     'wallet_withdrawal',
                     'withdrawal_request'
                 )
             AND wallet_withdrawal_requests.id = CAST(wl.ref_id AS UNSIGNED)
             AND wallet_withdrawal_requests.user_id = wl.user_id
             AND wallet_withdrawal_requests.asset_symbol = a.symbol
       LEFT JOIN withdraw_records
              ON wl.ref_type IN ('withdraw_record', 'withdrawal_record', 'withdraw')
             AND withdraw_records.id = CAST(wl.ref_id AS UNSIGNED)
             AND withdraw_records.user_id = wl.user_id
             AND withdraw_records.asset_id = wl.asset_id
       WHERE wl.user_id = "#
}

fn deposit_asset_response(row: DepositAssetRow) -> DepositAssetResponse {
    DepositAssetResponse {
        symbol: row.symbol,
        name: row.name,
        logo_url: row.logo_url,
        precision_scale: row.precision_scale,
        deposit_enabled: row.deposit_enabled,
        withdraw_enabled: row.withdraw_enabled,
        min_deposit_amount: row.min_deposit_amount,
        deposit_fee: row.deposit_fee,
        withdraw_fee: row.withdraw_fee,
        withdraw_fee_tiers: row.withdraw_fee_tiers.0,
    }
}

fn deposit_network_response(row: DepositNetworkRow) -> DepositNetworkResponse {
    DepositNetworkResponse {
        network: row.network,
        display_name: row.display_name,
        address_group_code: row.address_group_code,
        address_group_name: row.address_group_name,
        asset_symbols: row.asset_symbols.0,
    }
}

fn deposit_address_response(row: DepositAddressRow) -> DepositAddressResponse {
    DepositAddressResponse {
        id: row.id,
        asset_symbol: row.asset_symbol,
        network: row.network,
        address: row.address,
        memo: row.memo,
        assigned_at: row.assigned_at,
    }
}

fn wallet_account_response(row: WalletAccountRow) -> WalletAccountResponse {
    WalletAccountResponse {
        user_id: row.user_id,
        asset_id: row.asset_id,
        symbol: row.symbol,
        logo_url: row.logo_url,
        available: row.available,
        frozen: row.frozen,
        locked: row.locked,
    }
}

fn wallet_ledger_entry_response(row: WalletLedgerEntryRow) -> WalletLedgerEntryResponse {
    WalletLedgerEntryResponse {
        id: row.id,
        user_id: row.user_id,
        asset_id: row.asset_id,
        symbol: row.symbol,
        change_type: row.change_type,
        amount: row.amount,
        balance_type: row.balance_type,
        balance_after: row.balance_after,
        available_after: row.available_after,
        frozen_after: row.frozen_after,
        locked_after: row.locked_after,
        fee: row.fee,
        ref_type: row.ref_type,
        ref_id: row.ref_id,
        created_at: row.created_at,
    }
}
