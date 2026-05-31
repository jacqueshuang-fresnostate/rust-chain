use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool};

pub mod routes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalanceBucket {
    Available,
    Frozen,
    Locked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletDomainError {
    NegativeBalance {
        bucket: BalanceBucket,
    },
    NonPositiveLockAmount,
    LockedBalanceInvariantMismatch {
        account_locked: BigDecimal,
        active_positions_remaining: BigDecimal,
    },
}

#[derive(Debug, Clone)]
pub struct WalletAccount {
    pub user_id: String,
    pub asset_id: String,
    pub available: BigDecimal,
    pub frozen: BigDecimal,
    pub locked: BigDecimal,
}

#[derive(Debug, Clone)]
pub struct BalanceChange {
    pub available: BigDecimal,
    pub frozen: BigDecimal,
    pub locked: BigDecimal,
}

impl BalanceChange {
    pub fn new(available: BigDecimal, frozen: BigDecimal, locked: BigDecimal) -> Self {
        Self {
            available,
            frozen,
            locked,
        }
    }
}

impl WalletAccount {
    pub fn apply_balance_change(&mut self, change: BalanceChange) -> Result<(), WalletDomainError> {
        let next_available = self.available.clone() + change.available;
        let next_frozen = self.frozen.clone() + change.frozen;
        let next_locked = self.locked.clone() + change.locked;

        ensure_non_negative(&next_available, BalanceBucket::Available)?;
        ensure_non_negative(&next_frozen, BalanceBucket::Frozen)?;
        ensure_non_negative(&next_locked, BalanceBucket::Locked)?;

        self.available = next_available;
        self.frozen = next_frozen;
        self.locked = next_locked;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletServiceError {
    Domain(WalletDomainError),
    MissingLedgerMetadata(&'static str),
    NonPositiveAmount,
    Repository(String),
}

impl From<WalletDomainError> for WalletServiceError {
    fn from(error: WalletDomainError) -> Self {
        Self::Domain(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerMetadata {
    change_type: String,
    ref_type: String,
    ref_id: String,
}

impl LedgerMetadata {
    pub fn new(
        change_type: impl Into<String>,
        ref_type: impl Into<String>,
        ref_id: impl Into<String>,
    ) -> Result<Self, WalletServiceError> {
        let change_type = change_type.into();
        let ref_type = ref_type.into();
        let ref_id = ref_id.into();

        ensure_required_metadata_field("change_type", &change_type)?;
        ensure_required_metadata_field("ref_type", &ref_type)?;
        ensure_required_metadata_field("ref_id", &ref_id)?;

        Ok(Self {
            change_type,
            ref_type,
            ref_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletLedgerEntry {
    pub user_id: String,
    pub asset_id: String,
    pub change_type: String,
    pub amount: BigDecimal,
    pub balance_type: BalanceBucket,
    pub balance_after: BigDecimal,
    pub available_after: BigDecimal,
    pub frozen_after: BigDecimal,
    pub locked_after: BigDecimal,
    pub ref_type: String,
    pub ref_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerBatch {
    entries: Vec<WalletLedgerEntry>,
}

impl LedgerBatch {
    pub fn from_account_change(
        account: &WalletAccount,
        change: BalanceChange,
        metadata: &LedgerMetadata,
    ) -> Self {
        let mut entries = Vec::new();
        push_ledger_entry(
            &mut entries,
            account,
            metadata,
            BalanceBucket::Available,
            change.available,
            account.available.clone(),
        );
        push_ledger_entry(
            &mut entries,
            account,
            metadata,
            BalanceBucket::Frozen,
            change.frozen,
            account.frozen.clone(),
        );
        push_ledger_entry(
            &mut entries,
            account,
            metadata,
            BalanceBucket::Locked,
            change.locked,
            account.locked.clone(),
        );

        Self { entries }
    }

    pub fn entries(&self) -> &[WalletLedgerEntry] {
        &self.entries
    }

    pub fn into_entries(self) -> Vec<WalletLedgerEntry> {
        self.entries
    }
}

pub trait WalletRepository {
    fn load_account(
        &mut self,
        user_id: &str,
        asset_id: &str,
    ) -> Result<WalletAccount, WalletServiceError>;

    fn save_account_with_ledger(
        &mut self,
        account: WalletAccount,
        ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError>;

    fn insert_lock_positions(
        &mut self,
        positions: Vec<LockPosition>,
    ) -> Result<(), WalletServiceError>;
}

#[derive(Debug, Clone)]
pub struct MySqlWalletRepository {
    pool: Pool<MySql>,
}

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
        sqlx::query(
            r#"INSERT INTO wallet_accounts (user_id, asset_id)
               VALUES (?, ?)
               ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
        )
        .bind(user_id)
        .bind(asset_id)
        .execute(&self.pool)
        .await
        .map_err(map_wallet_sqlx_error)?;

        self.load_account_async(user_id, asset_id)
            .await?
            .ok_or_else(|| {
                WalletServiceError::Repository("wallet account was not created".to_owned())
            })
    }

    pub async fn load_account_async(
        &self,
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
        .fetch_optional(&self.pool)
        .await
        .map_err(map_wallet_sqlx_error)?;

        Ok(row.map(wallet_account_from_row))
    }

    pub async fn save_account_with_ledger_async(
        &self,
        account: WalletAccount,
        ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError> {
        let user_id = parse_u64_identifier("user_id", &account.user_id)?;
        let asset_id = parse_u64_identifier("asset_id", &account.asset_id)?;
        let mut tx = self.pool.begin().await.map_err(map_wallet_sqlx_error)?;

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
            sqlx::query(
                r#"INSERT INTO wallet_ledger
                   (user_id, asset_id, change_type, amount, balance_type, balance_after,
                    available_after, frozen_after, locked_after, ref_type, ref_id)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(parse_u64_identifier("ledger.user_id", &entry.user_id)?)
            .bind(parse_u64_identifier("ledger.asset_id", &entry.asset_id)?)
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

    pub async fn list_ledger_by_ref_async(
        &self,
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
        .fetch_all(&self.pool)
        .await
        .map_err(map_wallet_sqlx_error)?;

        rows.into_iter().map(wallet_ledger_from_row).collect()
    }

    pub async fn insert_asset_lock_positions_async(
        &self,
        positions: Vec<NewAssetLockPosition>,
    ) -> Result<Vec<u64>, WalletServiceError> {
        let mut tx = self.pool.begin().await.map_err(map_wallet_sqlx_error)?;
        let mut ids = Vec::with_capacity(positions.len());

        for position in positions {
            let result = sqlx::query(
                r#"INSERT INTO asset_lock_positions
                   (user_id, asset_id, unlock_type, unlock_at, locked_amount,
                    remaining_amount, merge_key, status)
                   VALUES (?, ?, ?, ?, 0, 0, ?, 'active')
                   ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
            )
            .bind(position.user_id)
            .bind(position.asset_id)
            .bind(position.unlock_type)
            .bind(position.unlock_at.naive_utc())
            .bind(position.merge_key.clone())
            .execute(&mut *tx)
            .await
            .map_err(map_wallet_sqlx_error)?;

            let position_id = if result.last_insert_id() == 0 {
                sqlx::query_as::<_, (u64,)>(
                    "SELECT id FROM asset_lock_positions WHERE merge_key = ? LIMIT 1",
                )
                .bind(&position.merge_key)
                .fetch_one(&mut *tx)
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
                .bind(source.source_type)
                .bind(source.source_id)
                .bind(&source.source_amount)
                .bind(source.source_time.naive_utc())
                .execute(&mut *tx)
                .await
                .map_err(map_wallet_sqlx_error)?;

                if inserted.rows_affected() > 0 {
                    sqlx::query(
                        r#"UPDATE asset_lock_positions
                           SET locked_amount = locked_amount + ?,
                               remaining_amount = remaining_amount + ?
                           WHERE id = ?"#,
                    )
                    .bind(&source.source_amount)
                    .bind(&source.source_amount)
                    .bind(position_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(map_wallet_sqlx_error)?;
                }
            }

            ids.push(position_id);
        }

        tx.commit().await.map_err(map_wallet_sqlx_error)?;
        Ok(ids)
    }

    pub async fn count_lock_position_sources_async(
        &self,
        lock_position_id: u64,
    ) -> Result<u64, WalletServiceError> {
        let (count,) = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM asset_lock_position_sources WHERE lock_position_id = ?",
        )
        .bind(lock_position_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_wallet_sqlx_error)?;

        Ok(count as u64)
    }
}

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

#[derive(Debug, Clone)]
pub struct BalanceUpdateCommand {
    pub user_id: String,
    pub asset_id: String,
    pub change: BalanceChange,
    pub ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct FreezeBalanceCommand {
    pub user_id: String,
    pub asset_id: String,
    pub amount: BigDecimal,
    pub ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct UnfreezeBalanceCommand {
    pub user_id: String,
    pub asset_id: String,
    pub amount: BigDecimal,
    pub ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct SettleBalanceCommand {
    pub user_id: String,
    pub debit_frozen_asset_id: String,
    pub debit_frozen_amount: BigDecimal,
    pub credit_available_asset_id: String,
    pub credit_available_amount: BigDecimal,
    pub ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct LockPositionCreationCommand {
    pub user_id: String,
    pub asset_id: String,
    pub schedule: LockSchedule,
    pub sources: Vec<LockPositionSource>,
    pub ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct WalletService<R> {
    repository: R,
}

impl<R> WalletService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }

    pub fn into_repository(self) -> R {
        self.repository
    }
}

impl<R: WalletRepository> WalletService<R> {
    pub fn apply_balance_update(
        &mut self,
        command: BalanceUpdateCommand,
    ) -> Result<WalletAccount, WalletServiceError> {
        let mut account = self
            .repository
            .load_account(&command.user_id, &command.asset_id)?;
        account.apply_balance_change(command.change.clone())?;
        let ledger = LedgerBatch::from_account_change(&account, command.change, &command.ledger);
        self.repository
            .save_account_with_ledger(account.clone(), ledger)?;
        Ok(account)
    }

    pub fn freeze(
        &mut self,
        command: FreezeBalanceCommand,
    ) -> Result<WalletAccount, WalletServiceError> {
        ensure_positive_amount(&command.amount)?;
        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.asset_id,
            change: BalanceChange::new(
                -command.amount.clone(),
                command.amount,
                BigDecimal::from(0),
            ),
            ledger: command.ledger,
        })
    }

    pub fn unfreeze(
        &mut self,
        command: UnfreezeBalanceCommand,
    ) -> Result<WalletAccount, WalletServiceError> {
        ensure_positive_amount(&command.amount)?;
        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.asset_id,
            change: BalanceChange::new(
                command.amount.clone(),
                -command.amount,
                BigDecimal::from(0),
            ),
            ledger: command.ledger,
        })
    }

    pub fn settle(&mut self, command: SettleBalanceCommand) -> Result<(), WalletServiceError> {
        ensure_positive_amount(&command.debit_frozen_amount)?;
        ensure_positive_amount(&command.credit_available_amount)?;

        if command.debit_frozen_asset_id == command.credit_available_asset_id {
            self.apply_balance_update(BalanceUpdateCommand {
                user_id: command.user_id,
                asset_id: command.debit_frozen_asset_id,
                change: BalanceChange::new(
                    command.credit_available_amount,
                    -command.debit_frozen_amount,
                    BigDecimal::from(0),
                ),
                ledger: command.ledger,
            })?;
            return Ok(());
        }

        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id.clone(),
            asset_id: command.debit_frozen_asset_id,
            change: BalanceChange::new(
                BigDecimal::from(0),
                -command.debit_frozen_amount,
                BigDecimal::from(0),
            ),
            ledger: command.ledger.clone(),
        })?;
        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.credit_available_asset_id,
            change: BalanceChange::new(
                command.credit_available_amount,
                BigDecimal::from(0),
                BigDecimal::from(0),
            ),
            ledger: command.ledger,
        })?;
        Ok(())
    }

    pub fn create_lock_positions(
        &mut self,
        command: LockPositionCreationCommand,
    ) -> Result<Vec<LockPosition>, WalletServiceError> {
        let positions = create_lock_positions(
            &command.user_id,
            &command.asset_id,
            command.schedule,
            command.sources,
        )?;
        let total_locked = positions.iter().fold(BigDecimal::from(0), |sum, position| {
            sum + position.remaining_amount.clone()
        });
        ensure_positive_amount(&total_locked)?;

        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.asset_id,
            change: BalanceChange::new(-total_locked.clone(), BigDecimal::from(0), total_locked),
            ledger: command.ledger,
        })?;
        self.repository.insert_lock_positions(positions.clone())?;
        Ok(positions)
    }
}

#[derive(Debug, Clone)]
pub struct LockPosition {
    pub user_id: String,
    pub asset_id: String,
    pub unlock_type: String,
    pub unlock_at: chrono::DateTime<chrono::Utc>,
    pub remaining_amount: BigDecimal,
    pub merge_key: String,
    pub source_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LockPositionSource {
    pub source_id: String,
    pub amount: BigDecimal,
    pub unlock_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub enum LockSchedule {
    ImmediateOnListing {
        listed_at: chrono::DateTime<chrono::Utc>,
    },
    FixedTime {
        unlock_at: chrono::DateTime<chrono::Utc>,
    },
    RelativePeriod,
}

pub fn fixed_time_merge_key(
    user_id: &str,
    asset_id: &str,
    unlock_at: chrono::DateTime<chrono::Utc>,
) -> String {
    format!("fixed_time:{user_id}:{asset_id}:{}", unlock_at.timestamp())
}

pub fn immediate_on_listing_merge_key(
    user_id: &str,
    asset_id: &str,
    listed_at: chrono::DateTime<chrono::Utc>,
) -> String {
    format!(
        "immediate_on_listing:{user_id}:{asset_id}:{}",
        listed_at.timestamp()
    )
}

pub fn create_lock_positions(
    user_id: &str,
    asset_id: &str,
    schedule: LockSchedule,
    sources: Vec<LockPositionSource>,
) -> Result<Vec<LockPosition>, WalletDomainError> {
    match schedule {
        LockSchedule::ImmediateOnListing { listed_at } => merged_lock_position(
            user_id,
            asset_id,
            "immediate_on_listing",
            listed_at,
            immediate_on_listing_merge_key(user_id, asset_id, listed_at),
            sources,
        ),
        LockSchedule::FixedTime { unlock_at } => merged_lock_position(
            user_id,
            asset_id,
            "fixed_time",
            unlock_at,
            fixed_time_merge_key(user_id, asset_id, unlock_at),
            sources,
        ),
        LockSchedule::RelativePeriod => sources
            .into_iter()
            .map(|source| {
                ensure_positive_lock_amount(&source.amount)?;
                Ok(LockPosition {
                    user_id: user_id.to_owned(),
                    asset_id: asset_id.to_owned(),
                    unlock_type: "relative_period".to_owned(),
                    unlock_at: source.unlock_at,
                    remaining_amount: source.amount,
                    merge_key: relative_period_merge_key(user_id, asset_id, &source.source_id),
                    source_id: Some(source.source_id),
                })
            })
            .collect(),
    }
}

pub fn verify_locked_balance_invariant(
    account: &WalletAccount,
    active_positions: &[LockPosition],
) -> Result<(), WalletDomainError> {
    let active_remaining = active_positions
        .iter()
        .filter(|position| {
            position.user_id == account.user_id && position.asset_id == account.asset_id
        })
        .fold(BigDecimal::from(0), |sum, position| {
            sum + position.remaining_amount.clone()
        });

    if account.locked == active_remaining {
        Ok(())
    } else {
        Err(WalletDomainError::LockedBalanceInvariantMismatch {
            account_locked: account.locked.clone(),
            active_positions_remaining: active_remaining,
        })
    }
}

fn merged_lock_position(
    user_id: &str,
    asset_id: &str,
    unlock_type: &str,
    unlock_at: chrono::DateTime<chrono::Utc>,
    merge_key: String,
    sources: Vec<LockPositionSource>,
) -> Result<Vec<LockPosition>, WalletDomainError> {
    let remaining_amount = sources
        .into_iter()
        .try_fold(BigDecimal::from(0), |sum, source| {
            ensure_positive_lock_amount(&source.amount)?;
            Ok(sum + source.amount)
        })?;

    Ok(vec![LockPosition {
        user_id: user_id.to_owned(),
        asset_id: asset_id.to_owned(),
        unlock_type: unlock_type.to_owned(),
        unlock_at,
        remaining_amount,
        merge_key,
        source_id: None,
    }])
}

fn ensure_non_negative(
    amount: &BigDecimal,
    bucket: BalanceBucket,
) -> Result<(), WalletDomainError> {
    if amount < &BigDecimal::from(0) {
        Err(WalletDomainError::NegativeBalance { bucket })
    } else {
        Ok(())
    }
}

fn ensure_required_metadata_field(
    field: &'static str,
    value: &str,
) -> Result<(), WalletServiceError> {
    if value.trim().is_empty() {
        Err(WalletServiceError::MissingLedgerMetadata(field))
    } else {
        Ok(())
    }
}

fn ensure_positive_amount(amount: &BigDecimal) -> Result<(), WalletServiceError> {
    if amount <= &BigDecimal::from(0) {
        Err(WalletServiceError::NonPositiveAmount)
    } else {
        Ok(())
    }
}

fn push_ledger_entry(
    entries: &mut Vec<WalletLedgerEntry>,
    account: &WalletAccount,
    metadata: &LedgerMetadata,
    balance_type: BalanceBucket,
    amount: BigDecimal,
    balance_after: BigDecimal,
) {
    if amount == 0 {
        return;
    }

    entries.push(WalletLedgerEntry {
        user_id: account.user_id.clone(),
        asset_id: account.asset_id.clone(),
        change_type: metadata.change_type.clone(),
        amount,
        balance_type,
        balance_after,
        available_after: account.available.clone(),
        frozen_after: account.frozen.clone(),
        locked_after: account.locked.clone(),
        ref_type: metadata.ref_type.clone(),
        ref_id: metadata.ref_id.clone(),
    });
}

fn ensure_positive_lock_amount(amount: &BigDecimal) -> Result<(), WalletDomainError> {
    if amount <= &BigDecimal::from(0) {
        Err(WalletDomainError::NonPositiveLockAmount)
    } else {
        Ok(())
    }
}

fn relative_period_merge_key(user_id: &str, asset_id: &str, source_id: &str) -> String {
    format!("relative_period:{user_id}:{asset_id}:{source_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(seconds, 0).unwrap()
    }

    fn amount(value: i64) -> BigDecimal {
        BigDecimal::from(value)
    }

    fn source(
        id: &str,
        value: i64,
        unlock_at: chrono::DateTime<chrono::Utc>,
    ) -> LockPositionSource {
        LockPositionSource {
            source_id: id.to_owned(),
            amount: amount(value),
            unlock_at,
        }
    }

    fn account(available: i64, frozen: i64, locked: i64) -> WalletAccount {
        WalletAccount {
            user_id: "user-1".to_owned(),
            asset_id: "ASSET".to_owned(),
            available: amount(available),
            frozen: amount(frozen),
            locked: amount(locked),
        }
    }

    #[test]
    fn fixed_time_positions_share_aggregation_key() {
        let unlock_at = at(1_700_000_000);

        let positions = create_lock_positions(
            "user-1",
            "ASSET",
            LockSchedule::FixedTime { unlock_at },
            vec![
                source("order-1", 10, unlock_at),
                source("order-2", 15, unlock_at),
            ],
        )
        .unwrap();

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].unlock_type, "fixed_time");
        assert_eq!(positions[0].remaining_amount, amount(25));
        assert_eq!(
            positions[0].merge_key,
            fixed_time_merge_key("user-1", "ASSET", unlock_at)
        );
        assert_eq!(positions[0].source_id, None);
    }

    #[test]
    fn relative_period_positions_stay_split_by_source() {
        let unlock_at = at(1_700_000_000);

        let positions = create_lock_positions(
            "user-1",
            "ASSET",
            LockSchedule::RelativePeriod,
            vec![
                source("order-1", 10, unlock_at),
                source("order-2", 15, unlock_at),
            ],
        )
        .unwrap();

        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].unlock_type, "relative_period");
        assert_eq!(positions[0].source_id.as_deref(), Some("order-1"));
        assert_eq!(positions[0].unlock_at, unlock_at);
        assert_eq!(positions[0].remaining_amount, amount(10));
        assert_eq!(positions[1].source_id.as_deref(), Some("order-2"));
        assert_eq!(positions[1].unlock_at, unlock_at);
        assert_eq!(positions[1].remaining_amount, amount(15));
        assert_ne!(positions[0].merge_key, positions[1].merge_key);
    }

    #[test]
    fn balance_change_rejects_negative_bucket() {
        let mut account = account(10, 2, 3);

        let result =
            account.apply_balance_change(BalanceChange::new(amount(-11), amount(0), amount(0)));

        assert_eq!(
            result,
            Err(WalletDomainError::NegativeBalance {
                bucket: BalanceBucket::Available
            })
        );
        assert_eq!(account.available, amount(10));
        assert_eq!(account.frozen, amount(2));
        assert_eq!(account.locked, amount(3));
    }

    #[test]
    fn locked_balance_matches_active_lock_positions() {
        let account = account(10, 0, 25);
        let unlock_at = at(1_700_000_000);
        let active_positions = vec![
            LockPosition {
                user_id: "user-1".to_owned(),
                asset_id: "ASSET".to_owned(),
                unlock_type: "fixed_time".to_owned(),
                unlock_at,
                remaining_amount: amount(10),
                merge_key: "key-1".to_owned(),
                source_id: None,
            },
            LockPosition {
                user_id: "user-1".to_owned(),
                asset_id: "ASSET".to_owned(),
                unlock_type: "relative_period".to_owned(),
                unlock_at,
                remaining_amount: amount(15),
                merge_key: "key-2".to_owned(),
                source_id: Some("order-1".to_owned()),
            },
            LockPosition {
                user_id: "user-2".to_owned(),
                asset_id: "ASSET".to_owned(),
                unlock_type: "fixed_time".to_owned(),
                unlock_at,
                remaining_amount: amount(99),
                merge_key: "other-user".to_owned(),
                source_id: None,
            },
        ];

        assert_eq!(
            verify_locked_balance_invariant(&account, &active_positions),
            Ok(())
        );

        let mut mismatched = account;
        mismatched.locked = amount(26);

        assert!(matches!(
            verify_locked_balance_invariant(&mismatched, &active_positions),
            Err(WalletDomainError::LockedBalanceInvariantMismatch { .. })
        ));
    }
}
