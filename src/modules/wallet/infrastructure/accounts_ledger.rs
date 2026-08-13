//! 账户、三桶余额、锁仓与流水持久化。
//!
//! 资金不变量：available/frozen/locked 不得为负；账户快照与每笔流水必须描述同一事务后的余额，失败整体回滚。
//! 本文件同时承载三类职责：钱包仓储适配器、锁仓与来源明细落库、以及面向用户和后台的账本查询。
//! 账本分类由 change_type 的精确值或受控前缀推导，共十类，分类只影响筛选与展示，绝不参与金额计算。
//! 查询入口一律不持有资金行锁，返回值仅供审计与展示；真正的扣款必须走持有行锁的调用方事务。

use crate::{
    error::AppResult,
    modules::wallet::{
        BalanceBucket, LedgerBatch, LockPosition, WalletAccount, WalletLedgerEntry,
        WalletRepository, WalletServiceError,
        presentation::{
            WalletAccountResponse, WalletLedgerEntryResponse, WalletLedgerPageResponse,
            WalletLedgerResponse,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, QueryBuilder, Transaction};

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
    /// 用 MySQL 连接池构造钱包仓储适配器，池按引用计数克隆，不额外建立连接。
    /// 构造不校验表结构、不预热连接，也不代表数据库当前可用，首次查询才会暴露连接故障。
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    /// 借出内部 MySQL 连接池，供调用方自行开启事务以串联本适配器之外的资金步骤。
    /// 通过该池发起的写入不受适配器的账务编排约束，非负校验与镜像流水需要调用方自己保证。
    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    /// 幂等确保用户资产账户存在并加载三桶快照。
    /// 创建冲突不会覆盖余额；查询失败或创建后仍不可见时返回仓储错误。
    pub async fn get_or_create_account_async(
        &self,
        user_id: u64,
        asset_id: u64,
    ) -> Result<WalletAccount, WalletServiceError> {
        // SQL 细节已下沉到 infrastructure，仓储对象专注持久化编排。
        get_or_create_account_async(&self.pool, user_id, asset_id).await
    }

    /// 按用户与资产读取钱包账户三桶快照，账户不存在时返回空值而非零余额或错误。
    /// 该读取走连接池普通查询、不加行锁，返回值只能用于展示或前置判断，不得作为扣款依据。
    pub async fn load_account_async(
        &self,
        user_id: u64,
        asset_id: u64,
    ) -> Result<Option<WalletAccount>, WalletServiceError> {
        load_account_async(&self.pool, user_id, asset_id).await
    }

    /// 在基础设施自有事务中保存传入的三桶绝对快照，并逐条写入同批镜像流水。
    /// 该入口不先锁账户，也不校验业务引用唯一性；并发覆盖与重放控制由调用方负责，SQL 失败回滚本批账户和流水。
    pub async fn save_account_with_ledger_async(
        &self,
        account: WalletAccount,
        ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError> {
        // 与领域服务共享账务规则前置条件：真正的写库逻辑在这里执行。
        save_account_with_ledger_async(&self.pool, account, ledger).await
    }

    /// 按业务引用类型与编号读取账本条目并保持自增写入顺序，用于幂等核验和资金审计。
    /// 空结果表示该业务引用尚无流水，调用方据此判断是首次执行还是重放，但本方法不加锁防并发写入。
    pub async fn list_ledger_by_ref_async(
        &self,
        ref_type: &str,
        ref_id: &str,
    ) -> Result<Vec<WalletLedgerEntry>, WalletServiceError> {
        // 基础设施返回持久化后的领域实体，供领域服务消费。
        list_ledger_by_ref_async(&self.pool, ref_type, ref_id).await
    }

    /// 批量写入锁仓及来源明细，并在该批自有事务提交后返回全部锁仓编号。
    /// merge_key 和来源唯一约束让重放只累计新来源；任一 SQL 失败回滚本批锁仓，但不会撤销调用前已提交的账户 locked 变化。
    pub async fn insert_asset_lock_positions_async(
        &self,
        positions: Vec<NewAssetLockPosition>,
    ) -> Result<Vec<u64>, WalletServiceError> {
        // 锁仓来源与冻结量更新都在基础设施层做幂等落库，保障并发安全。
        insert_asset_lock_positions_async(&self.pool, positions).await
    }

    /// 统计指定锁仓记录已落库的来源明细条数，用于核对合并写入后的来源完整性。
    /// 计数不区分来源类型和金额，也不校验锁仓剩余额是否与来源之和相符，更不会修改任何余额。
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
    /// 同步仓储端口不执行异步 SQL，固定返回“需使用 async SQLx 方法”的仓储错误。
    /// 不读取账户、不加锁，也不把缺失账户伪装成零余额。
    fn load_account(
        &mut self,
        _user_id: &str,
        _asset_id: &str,
    ) -> Result<WalletAccount, WalletServiceError> {
        Err(WalletServiceError::Repository(
            "MySqlWalletRepository requires async SQLx methods".to_owned(),
        ))
    }

    /// 同步仓储端口不执行异步账户/流水事务，固定返回仓储错误且不产生资金写入。
    /// 账户三桶与账本批次原样丢弃，既不落库也不缓存，调用方必须改用异步适配器方法完成保存。
    fn save_account_with_ledger(
        &mut self,
        _account: WalletAccount,
        _ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError> {
        Err(WalletServiceError::Repository(
            "MySqlWalletRepository requires async SQLx methods".to_owned(),
        ))
    }

    /// 同步仓储端口不写锁仓明细，固定返回仓储错误；调用方应使用异步适配器方法。
    /// 传入的锁仓集合不会被部分写入，因此该失败路径不会让账户 locked 与锁仓明细产生偏差。
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
pub(super) struct WalletLedgerEntryRow {
    pub(super) id: u64,
    pub(super) user_id: u64,
    pub(super) asset_id: u64,
    pub(super) symbol: String,
    pub(super) change_type: String,
    pub(super) amount: BigDecimal,
    pub(super) balance_type: String,
    pub(super) balance_after: BigDecimal,
    pub(super) available_after: BigDecimal,
    pub(super) frozen_after: BigDecimal,
    pub(super) locked_after: BigDecimal,
    pub(super) fee: BigDecimal,
    pub(super) ref_type: String,
    pub(super) ref_id: String,
    pub(super) created_at: DateTime<Utc>,
}
#[derive(Debug)]
pub(crate) struct WalletLedgerFilter {
    pub(crate) asset_id: Option<u64>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) change_type: Option<String>,
    pub(crate) category: Option<WalletLedgerCategory>,
    pub(crate) ref_type: Option<String>,
    pub(crate) ref_id: Option<String>,
    pub(crate) start_time: Option<String>,
    pub(crate) end_time: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WalletLedgerCategory {
    Funding,
    Spot,
    Margin,
    Seconds,
    Convert,
    Earn,
    NewCoin,
    Loan,
    Prediction,
    Other,
}

impl WalletLedgerCategory {
    pub(crate) const ALL: [Self; 10] = [
        Self::Funding,
        Self::Spot,
        Self::Margin,
        Self::Seconds,
        Self::Convert,
        Self::Earn,
        Self::NewCoin,
        Self::Loan,
        Self::Prediction,
        Self::Other,
    ];

    /// 返回钱包流水分类对外暴露的稳定字符串，同时用作查询入参取值和响应字段取值。
    /// 该映射是 API 契约的一部分，改动会同时破坏前端筛选与历史数据的分类含义，不得随实现调整。
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Funding => "funding",
            Self::Spot => "spot",
            Self::Margin => "margin",
            Self::Seconds => "seconds",
            Self::Convert => "convert",
            Self::Earn => "earn",
            Self::NewCoin => "new_coin",
            Self::Loan => "loan",
            Self::Prediction => "prediction",
            Self::Other => "other",
        }
    }

    /// 将外部传入的分类字符串反解为枚举，只接受与对外契约完全一致的取值。
    /// 匹配区分大小写且不裁剪空白，未知取值返回空，由调用方转换成校验错误而非静默忽略筛选条件。
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.as_str() == value)
    }
}

struct WalletLedgerCategoryRule {
    category: WalletLedgerCategory,
    exact_change_types: &'static [&'static str],
    change_type_prefixes: &'static [&'static str],
}

const WALLET_LEDGER_CATEGORY_RULES: &[WalletLedgerCategoryRule] = &[
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Funding,
        exact_change_types: &["deposit", "admin_recharge", "quick_recharge"],
        change_type_prefixes: &["deposit_", "withdrawal_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Spot,
        exact_change_types: &[],
        change_type_prefixes: &["spot_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Margin,
        exact_change_types: &[],
        change_type_prefixes: &["margin_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Seconds,
        exact_change_types: &[],
        change_type_prefixes: &["seconds_contract_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Convert,
        exact_change_types: &[],
        change_type_prefixes: &["convert_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Earn,
        exact_change_types: &[],
        change_type_prefixes: &["earn_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::NewCoin,
        exact_change_types: &[],
        change_type_prefixes: &["new_coin_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Loan,
        exact_change_types: &[],
        change_type_prefixes: &["loan_"],
    },
    WalletLedgerCategoryRule {
        category: WalletLedgerCategory::Prediction,
        exact_change_types: &[],
        change_type_prefixes: &["prediction_"],
    },
];

/// 按精确 change_type 或受控前缀归类钱包流水，未命中时归入 other。
/// 分类只影响查询与展示，不改变原始 change_type、业务引用或任何账本金额。
pub(crate) fn classify_wallet_ledger_change_type(change_type: &str) -> WalletLedgerCategory {
    WALLET_LEDGER_CATEGORY_RULES
        .iter()
        .find(|rule| {
            rule.exact_change_types.contains(&change_type)
                || rule
                    .change_type_prefixes
                    .iter()
                    .any(|prefix| change_type.starts_with(prefix))
        })
        .map(|rule| rule.category)
        .unwrap_or(WalletLedgerCategory::Other)
}
/// 通过幂等插入确保钱包账户存在，再回读三桶快照。
/// 插入命中唯一键时只空转更新时间戳，既不覆盖已有余额，也不把已有账户重置为零。
/// 插入与回读是两次独立语句、不共享事务，因此并发调用可能读到对方刚建好的账户，这在语义上是允许的。
/// 回读为空说明账户在插入后又被删除或复制延迟，此时返回仓储错误而不是伪造零余额账户。
/// 该入口不锁定账户供资金更新使用，资金写入仍须在调用方事务中执行行锁。
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

/// 读取指定用户资产的钱包三桶余额，不存在时返回空值。
/// 与创建入口不同，本函数绝不隐式建账，缺失账户如实表达为空，交由调用方决定报错还是按未开通处理。
/// 数值列以定点类型原样读出，不做精度截断或负零归一化，返回值与数据库当前存储完全一致。
/// 该普通查询不持有行锁，资金更新必须改用调用方事务内的锁定原语。
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

/// 将钱包三桶余额快照与同批账本条目放入同一 MySQL 事务持久化。
/// 调用方必须提供已通过领域规则校验的账户与镜像流水；任一账户更新或流水插入失败都会回滚整批数据。
/// 账户与账本中的用户和资产标识都以字符串传入，解析为整数失败时在事务内立即报错并回滚，不做部分写入。
/// 账户更新写的是三桶绝对值而非增量，且执行前不锁行，因此并发调用会形成后写覆盖，调用方必须自行串行化。
/// 流水按批次给定顺序逐条插入，balance_after 与三桶 after 原样落库，不在此重新计算任何金额。
/// 此入口不负责生成业务幂等键，调用方需确保同一业务引用不会被重复保存。
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

/// 按业务引用读取账本并保留写入顺序，便于重放判断和资金审计。
/// 结果按自增主键升序返回，因此同一次余额变更产生的多条桶级流水次序与写入时一致，可直接还原变更过程。
/// 行内 balance_type 会被反解为余额桶枚举，出现未登记的桶名时整次查询失败，避免把损坏流水当作有效审计依据。
/// 查询只读流水快照，不据此修改账户；缺失条目由上层按账务异常处理。
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

/// 在自有事务中逐项插入锁仓及来源映射，全部成功后统一提交。
/// 返回的编号与入参锁仓一一对应且顺序一致，合并键命中既有记录时返回的是既有锁仓编号而非新编号。
/// 任一写入失败都会回滚，调用方不得假设返回前已有部分锁仓生效。
/// 本函数只维护锁仓侧数据，不触碰账户 locked 桶，账户与锁仓明细的一致性需由调用链另行保证。
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

/// 读取锁仓记录当前持久化的来源数量。
/// 该统计用于核对来源完整性，不调整账户 locked 桶或锁仓剩余额。
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

/// 在调用方事务中按合并键落地一条锁仓聚合，并把新增来源逐笔累加到锁定额与剩余额。
/// 锁仓行以合并键幂等插入，初始锁定额与剩余额都写零；命中既有记录时回查其编号，金额一律由来源累加得出。
/// 来源插入使用忽略重复语义，只有真正新增的来源才触发锁仓金额自增，因此同一来源重复投递不会重复放大 locked。
/// 累加使用数据库端的自增表达式而非先读后写，配合调用方事务避免并发投递互相覆盖。
/// 任一步失败向上抛出并由调用方回滚，不会留下锁仓已建但来源缺失或金额少算的中间状态。
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

/// 把账户查询行装配为领域账户实体，用户与资产的数字标识在此转成领域侧使用的字符串形式。
/// 三桶金额原样搬运，不做精度截断、符号归一或缺省补零，转换过程不会改变任何余额。
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

/// 把账本查询行装配为领域流水实体，其中余额桶名需反解成枚举，未登记的桶名直接返回仓储错误。
/// 变更金额、本桶账后余额与三桶 after 全部原样搬运，本函数不重算差额也不校验它们是否自洽。
/// 用户与资产标识转为字符串以对齐领域模型，业务引用类型和编号保持数据库原值，供上层判定重放。
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

/// 把余额桶枚举编码为账本表 balance_type 列存储的字面量，是流水落库时的唯一取值来源。
/// 该编码与历史数据强绑定，改动会让既有流水无法反解，因此必须与解析函数保持严格互逆。
fn balance_bucket_as_str(bucket: BalanceBucket) -> &'static str {
    match bucket {
        BalanceBucket::Available => "available",
        BalanceBucket::Frozen => "frozen",
        BalanceBucket::Locked => "locked",
    }
}

/// 把账本表存储的 balance_type 字面量反解为余额桶枚举，未知取值返回携带原值的仓储错误。
/// 这里刻意不做兜底归类，因为把无法识别的桶静默当成可用余额会让审计结论出现方向性错误。
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

/// 把领域侧字符串形式的用户或资产标识解析为数据库整数主键，失败时回填字段名与原值便于定位。
/// 解析失败一律视为仓储错误并中断当前资金事务，绝不退化成零值继续写库。
fn parse_u64_identifier(field: &str, value: &str) -> Result<u64, WalletServiceError> {
    value.parse::<u64>().map_err(|error| {
        WalletServiceError::Repository(format!("invalid numeric {field} `{value}`: {error}"))
    })
}

/// 把 SQLx 底层错误折叠成仓储错误字符串，使领域与服务层不必依赖具体数据库驱动类型。
/// 折叠会丢失唯一键冲突等结构化错误码，需要区分冲突与其他失败的调用方应改用保留原始错误的路径。
fn map_wallet_sqlx_error(error: sqlx::Error) -> WalletServiceError {
    WalletServiceError::Repository(error.to_string())
}

/// 按资产代码排序读取用户全部钱包账户及三桶余额快照，并联出资产符号与图标供前端直接渲染。
/// 使用内连接资产表，因此资产记录缺失的历史账户不会出现在结果中，返回集合可能少于账户表实际行数。
/// 结果不做零余额过滤，用户开通过但已清空的资产仍会返回，前端需要自行决定是否隐藏。
/// 查询不获取资金行锁，返回值仅用于展示，不可作为后续扣款依据。
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

/// 按同一过滤条件查询用户钱包流水和总数，并补充关联业务手续费。
/// fee 仅从闪兑订单、现货成交、提现申请/记录关联补充；流水 amount 和三桶 after 直接取 wallet_ledger，不重算资金。
/// 该入口只读账本快照，不锁余额；分页总数、分类规则与返回行使用一致谓词。
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
        total.div_ceil(filter.limit as u64) as u32
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

/// 统计当前筛选下用户账本的总行数，供分页计算总页数。
/// 计数查询复用与行查询完全相同的用户条件和过滤谓词构造器，两者结果因此描述同一筛选集合。
/// 计数只关联资产表以支持按资产符号筛选，不关联补充手续费用的业务表，避免多值连接放大行数。
/// 数据库返回的有符号计数会被下限钳到零再转为无符号，防止异常值让分页出现负数页码。
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

/// 把资产、分类、引用和时间条件同时追加到流水行查询或计数查询。
/// 所有可选条件都以并且关系叠加，未提供的条件不追加谓词，因此空过滤器等价于只按用户筛选。
/// 资产符号按大写比较，调用方需先完成归一化；变更类型、引用类型和引用编号一律按精确值匹配，不支持模糊查询。
/// 起止时间直接以字符串绑定并与创建时间做闭区间比较，时区与格式由调用方保证，本函数不做解析或校验。
/// 调用方必须对行与总数复用该构造器，确保分页统计与返回数据一致。
pub(super) fn push_wallet_ledger_filters<'args>(
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
    if let Some(category) = filter.category {
        push_wallet_ledger_category_filter(builder, category);
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

/// 把分类筛选翻译成 SQL 谓词，使分类查询与内存分类函数得到完全一致的归类结果。
/// 其他分类是补集语义：对全部已登记规则做析取后整体取反，因此新增规则会自动从其他分类中移除对应流水。
/// 具名分类直接命中其唯一规则，找不到规则时立即中止，避免静默退化成不带条件的全量查询。
fn push_wallet_ledger_category_filter<'args>(
    builder: &mut QueryBuilder<'args, MySql>,
    category: WalletLedgerCategory,
) {
    builder.push(" AND ");
    if category == WalletLedgerCategory::Other {
        builder.push("NOT (");
        for (index, rule) in WALLET_LEDGER_CATEGORY_RULES.iter().enumerate() {
            if index > 0 {
                builder.push(" OR ");
            }
            builder.push("(");
            push_wallet_ledger_category_rule(builder, rule);
            builder.push(")");
        }
        builder.push(")");
        return;
    }

    let rule = WALLET_LEDGER_CATEGORY_RULES
        .iter()
        .find(|rule| rule.category == category)
        .expect("every non-other wallet ledger category has a SQL rule");
    builder.push("(");
    push_wallet_ledger_category_rule(builder, rule);
    builder.push(")");
}

/// 把单条分类规则展开成若干或关系谓词：精确变更类型逐个等值比较，前缀规则按长度截取后比较。
/// 比较统一加二进制修饰以走区分大小写的字节匹配，防止排序规则差异让相近变更类型被误归到同一分类。
/// 规则至少要有一个谓词，空规则会在调试断言中暴露，因为它展开后是空条件，会让整条分类筛选失效。
fn push_wallet_ledger_category_rule<'args>(
    builder: &mut QueryBuilder<'args, MySql>,
    rule: &'static WalletLedgerCategoryRule,
) {
    let mut has_predicate = false;
    for change_type in rule.exact_change_types {
        if has_predicate {
            builder.push(" OR ");
        }
        builder.push("BINARY wl.change_type = ");
        builder.push_bind(*change_type);
        has_predicate = true;
    }
    for prefix in rule.change_type_prefixes {
        if has_predicate {
            builder.push(" OR ");
        }
        builder.push("LEFT(BINARY wl.change_type, ");
        builder.push_bind(prefix.len() as i64);
        builder.push(") = ");
        builder.push_bind(*prefix);
        has_predicate = true;
    }
    debug_assert!(
        has_predicate,
        "wallet ledger category rule must not be empty"
    );
}

/// 返回用户账本行查询的固定前缀，末尾停在用户条件的绑定位，供调用方继续追加过滤、排序与分页。
/// 手续费列由多个左连接按引用类型择一取值：闪兑取订单费、现货取成交费、提现按新旧两张表分别取费，都取不到时归零。
/// 现货连接需要把引用编号按冒号拆成买卖单编号再匹配，提现连接额外比对用户与资产，避免跨用户串账。
/// 补充手续费只影响展示字段，流水金额与三桶 after 仍原样取自账本表，本查询不重算任何资金数值。
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

/// 把账户查询行整体搬运为账户列表响应项，保留资产符号与图标地址供前端直接展示。
/// 三桶余额按定点原值输出，不合并成总额也不做单位换算，前端需要总资产时须自行相加。
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

/// 将数据库流水行映射为 API 条目，并按 change_type 补充稳定业务分类。
/// 分类在此按内存规则现算而非读取存量列，与 SQL 侧分类筛选共用同一套规则，保证筛选结果与展示标签一致。
/// 手续费取自查询阶段左连接的择一结果，未匹配业务单据时为零，该字段是展示补充而非账本自身的资金腿。
/// 映射保留三桶账后快照和业务引用，不重新计算或改变任何资金金额。
pub(super) fn wallet_ledger_entry_response(row: WalletLedgerEntryRow) -> WalletLedgerEntryResponse {
    let category = classify_wallet_ledger_change_type(&row.change_type)
        .as_str()
        .to_owned();
    WalletLedgerEntryResponse {
        id: row.id,
        user_id: row.user_id,
        asset_id: row.asset_id,
        symbol: row.symbol,
        change_type: row.change_type,
        category,
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
