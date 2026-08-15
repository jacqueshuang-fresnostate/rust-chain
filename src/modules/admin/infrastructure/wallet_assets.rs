use super::*;

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminAssetSymbolRow {
    pub(crate) symbol: String,
    pub(crate) status: String,
}

#[derive(Debug)]
pub(crate) struct AdminAssetListFilter {
    pub(crate) symbol: Option<String>,
    pub(crate) asset_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminAssetInsert {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: bool,
    pub(crate) withdraw_enabled: bool,
    pub(crate) margin_transfer_enabled: bool,
    pub(crate) min_deposit_amount: BigDecimal,
    pub(crate) deposit_fee: BigDecimal,
    pub(crate) withdraw_fee: BigDecimal,
    pub(crate) withdraw_fee_tiers: Vec<WithdrawFeeTier>,
}

#[derive(Debug)]
pub(crate) struct AdminAssetUpdate {
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: bool,
    pub(crate) withdraw_enabled: bool,
    pub(crate) margin_transfer_enabled: bool,
    pub(crate) min_deposit_amount: BigDecimal,
    pub(crate) deposit_fee: BigDecimal,
    pub(crate) withdraw_fee: BigDecimal,
    pub(crate) withdraw_fee_tiers: Vec<WithdrawFeeTier>,
}

#[derive(Debug)]
pub(crate) struct AdminWalletAccountListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) include_empty: bool,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminWalletLedgerListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) change_type: Option<String>,
    pub(crate) ref_type: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositNetworkConfigListFilter {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositNetworkConfigWrite {
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: Vec<String>,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositAddressPoolListFilter {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) assigned_user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) address: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminDepositAddressPoolWrite {
    pub(crate) network: String,
    pub(crate) address_group_code: String,
    pub(crate) address: String,
    pub(crate) asset_symbols: Vec<String>,
    pub(crate) status: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminWalletRow {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct AdminWalletEmptyAssetRow {
    asset_id: u64,
    asset_symbol: String,
}

/// 在调用方事务中读取资产符号并确认其状态为 active，供人工充值等资金写入建立资产前置条件。
/// 查询不加行锁也不提交事务；资产缺失返回未找到，非启用资产返回校验错误，SQL 失败由上层回滚。
pub(crate) async fn load_active_asset_symbol_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetSymbolRow> {
    let asset = sqlx::query_as::<_, AdminAssetSymbolRow>(
        "SELECT symbol, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != "active" {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

/// 分页查询资产，返回符合调用方筛选条件的记录及相同谓词下的总数。
/// 资产列表与计数通过连接池分别执行且均不加锁；并发写入可能造成页数据与总数快照不同，SQL 或字段映射失败直接返回错误。
pub(crate) async fn list_admin_assets(
    pool: &Pool<MySql>,
    filter: AdminAssetListFilter,
) -> AppResult<(Vec<AdminAssetResponse>, i64)> {
    let mut rows = admin_asset_query();
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM assets");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(symbol) = filter.symbol.clone() {
            builder.push(" AND symbol = ");
            builder.push_bind(symbol);
        }
        if let Some(asset_type) = filter.asset_type.clone() {
            builder.push(" AND asset_type = ");
            builder.push_bind(asset_type);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按传入主键或筛选条件从连接池读取资产并映射为应用层所需的完整记录。
/// 资产不追加行锁，查询不创建事务；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_admin_asset(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE id = ");
    builder.push_bind(asset_id);
    builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中插入资产并返回或保留数据库写入结果。
/// 资产数据库唯一键冲突会映射为业务冲突；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
pub(crate) async fn insert_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAssetInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO assets
              (symbol, name, logo_url, precision_scale, asset_type, status, deposit_enabled, withdraw_enabled,
               margin_transfer_enabled, min_deposit_amount, deposit_fee, withdraw_fee, withdraw_fee_tiers_json)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.symbol)
    .bind(&input.name)
    .bind(&input.logo_url)
    .bind(input.precision_scale)
    .bind(&input.asset_type)
    .bind(&input.status)
    .bind(input.deposit_enabled)
    .bind(input.withdraw_enabled)
    .bind(input.margin_transfer_enabled)
    .bind(&input.min_deposit_amount)
    .bind(&input.deposit_fee)
    .bind(&input.withdraw_fee)
    .bind(SqlxJson(input.withdraw_fee_tiers))
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_asset_error)?;
    Ok(result.last_insert_id())
}

/// 在调用方事务中按传入主键或筛选条件更新资产，写入应用层已决定的目标字段。
/// 资产更新不检查受影响行数；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
pub(crate) async fn update_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
    input: AdminAssetUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE assets
           SET name = ?,
               logo_url = ?,
               precision_scale = ?,
               asset_type = ?,
               status = ?,
               deposit_enabled = ?,
               withdraw_enabled = ?,
               margin_transfer_enabled = ?,
               min_deposit_amount = ?,
               deposit_fee = ?,
               withdraw_fee = ?,
               withdraw_fee_tiers_json = ?
           WHERE id = ?"#,
    )
    .bind(&input.name)
    .bind(&input.logo_url)
    .bind(input.precision_scale)
    .bind(&input.asset_type)
    .bind(&input.status)
    .bind(input.deposit_enabled)
    .bind(input.withdraw_enabled)
    .bind(input.margin_transfer_enabled)
    .bind(&input.min_deposit_amount)
    .bind(&input.deposit_fee)
    .bind(&input.withdraw_fee)
    .bind(SqlxJson(input.withdraw_fee_tiers))
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中删除资产，调用前应已锁定资源并完成引用与余额等业务约束检查。
/// 本函数不提交事务或级联补偿；受影响行为空或 SQL 失败返回错误，审计由应用层同事务追加。
pub(crate) async fn delete_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 按传入主键或筛选条件从调用方事务快照读取资产并映射为应用层所需的完整记录。
/// 资产不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE id = ");
    builder.push_bind(asset_id);
    builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中按传入主键或筛选条件以 `FOR UPDATE` 锁定资产并返回一致的修改前快照。
/// 资产锁由调用方事务持有至结束；函数不自行提交，记录缺失返回未找到，SQL 或解码失败交由外层回滚。
pub(crate) async fn lock_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE id = ");
    builder.push_bind(asset_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中为当前 users 表内每个用户补建指定资产的零余额钱包账户。
/// `INSERT IGNORE ... SELECT` 使已存在的用户资产账户保持不变，并发唯一键冲突被忽略；函数不检查插入数量，调用方负责与资产创建及审计统一提交。
pub(crate) async fn create_wallet_accounts_for_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           SELECT id, ?, 0, 0, 0
           FROM users"#,
    )
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中删除指定资产下可用、冻结和锁定余额均为零的钱包账户。
/// 删除允许影响零行或多行且不级联账本；调用方须先锁定并禁用资产，随后检查剩余引用并与资产删除、审计统一提交。
pub(crate) async fn delete_zero_balance_wallet_accounts_for_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"DELETE FROM wallet_accounts
           WHERE asset_id = ? AND available = 0 AND frozen = 0 AND locked = 0"#,
    )
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在删除资产的调用方事务中，检查钱包、账本及各交易业务是否仍引用该资产。
/// 调用方须先锁定并禁用目标资产，且先清理允许删除的零余额空钱包账户。
/// 检查复用当前事务快照但不自行加业务表锁；发现任一资金或订单引用即拒绝删除。
/// 本函数只读且可重复调用；查询失败或存在引用均由调用方回滚，绝不级联删除历史账务。
pub(crate) async fn ensure_asset_has_no_references_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let (has_references,): (i64,) = sqlx::query_as(
        r#"SELECT CASE WHEN
                  EXISTS(SELECT 1 FROM wallet_accounts WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM wallet_ledger WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM asset_lock_positions WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM asset_unlock_records WHERE asset_id = ? OR unlock_fee_asset = ?)
               OR EXISTS(SELECT 1 FROM deposit_records WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM withdraw_records WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM trading_pairs WHERE base_asset = ? OR quote_asset = ?)
               OR EXISTS(SELECT 1 FROM spot_orders WHERE reserved_asset = ?)
               OR EXISTS(SELECT 1 FROM new_coin_projects WHERE asset_id = ? OR unlock_fee_asset = ?)
               OR EXISTS(SELECT 1 FROM new_coin_subscriptions WHERE quote_asset = ?)
               OR EXISTS(SELECT 1 FROM new_coin_distributions WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM new_coin_purchase_orders WHERE base_asset = ? OR quote_asset = ?)
               OR EXISTS(SELECT 1 FROM convert_pairs WHERE from_asset = ? OR to_asset = ?)
               OR EXISTS(SELECT 1 FROM convert_quotes WHERE from_asset = ? OR to_asset = ?)
               OR EXISTS(SELECT 1 FROM convert_orders WHERE from_asset = ? OR to_asset = ?)
               OR EXISTS(SELECT 1 FROM seconds_contract_products WHERE stake_asset = ?)
               OR EXISTS(SELECT 1 FROM seconds_contract_orders WHERE stake_asset = ?)
               OR EXISTS(SELECT 1 FROM margin_products WHERE margin_asset = ?)
               OR EXISTS(SELECT 1 FROM margin_positions WHERE margin_asset = ?)
               OR EXISTS(SELECT 1 FROM margin_liquidation_records WHERE margin_asset = ?)
               OR EXISTS(SELECT 1 FROM earn_products WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM earn_subscriptions WHERE asset_id = ?)
               OR EXISTS(SELECT 1 FROM quick_recharge_orders WHERE asset_id = ?)
             THEN 1 ELSE 0 END AS has_references"#,
    )
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await?;

    if has_references != 0 {
        return Err(AppError::Validation(
            "asset with related records cannot be deleted".to_owned(),
        ));
    }
    Ok(())
}

/// 按管理员筛选条件分页读取钱包账户及三类余额快照，并同步计算持久化总数。
/// 调用方负责管理员权限；默认排除内部用户，可按用户、邮箱和资产收窄范围。
/// 该查询不启事务、不锁账户且不修改资金；主键排序避免余额更新时间变化造成分页漂移。
/// `include_empty` 仅为当前页补齐未建账的零余额资产并修正返回总数；查询失败无副作用。
pub(crate) async fn list_admin_wallet_accounts(
    pool: &Pool<MySql>,
    filter: AdminWalletAccountListFilter,
) -> AppResult<(Vec<AdminWalletAccountResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT accounts.id, accounts.user_id, account_users.email AS user_email,
                  accounts.asset_id, assets.symbol AS asset_symbol,
                  accounts.available, accounts.frozen, accounts.locked, TRUE AS account_exists, accounts.updated_at
           FROM wallet_accounts accounts
           INNER JOIN users account_users ON account_users.id = accounts.user_id
           INNER JOIN assets ON assets.id = accounts.asset_id"#,
    );
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM wallet_accounts accounts
           INNER JOIN users account_users ON account_users.id = accounts.user_id
           INNER JOIN assets ON assets.id = accounts.asset_id"#,
    );
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if !filter.include_internal {
            push_exclude_internal_user_email(builder, "account_users.email");
        }
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "accounts.user_id", user_id);
        }
        push_user_email_filter(builder, "accounts.user_id", filter.email.clone());
        if let Some(asset_id) = filter.asset_id {
            builder.push(" AND accounts.asset_id = ");
            builder.push_bind(asset_id);
        }
    }

    let (mut accounts, mut total) = fetch_admin_page::<AdminWalletAccountResponse>(
        pool,
        rows,
        total,
        // 按主键排序：updated_at 每笔余额变动都会改，分页时行会在页间跳动。
        " ORDER BY accounts.id DESC",
        filter.limit,
        filter.offset,
    )
    .await?;
    if filter.include_empty {
        // 补齐的空账户是内存拼接结果，总数按本页补齐条数累加，保持与返回行一致。
        let persisted = accounts.len();
        append_empty_wallet_accounts(pool, &filter, &mut accounts).await?;
        total += (accounts.len() - persisted) as i64;
    }
    Ok((accounts, total))
}

/// 按管理员筛选条件分页读取不可变钱包流水及每笔变动后的完整余额快照。
/// 调用方负责管理员权限；用户、邮箱、资产、变动类型和业务引用条件同时作用于列表与计数。
/// 该只读查询不启事务、不加资金锁，按时间和主键倒序稳定返回，且不得重新计算账务金额。
/// 数据库错误直接返回且无副作用；空结果保留真实总数和分页语义。
pub(crate) async fn list_admin_wallet_ledger(
    pool: &Pool<MySql>,
    filter: AdminWalletLedgerListFilter,
) -> AppResult<(Vec<AdminWalletLedgerResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT ledger.id, ledger.user_id, ledger_users.email AS user_email,
                  ledger.asset_id, assets.symbol AS asset_symbol,
                  ledger.change_type, ledger.amount, ledger.balance_type, ledger.balance_after,
                  ledger.available_after, ledger.frozen_after, ledger.locked_after,
                  ledger.ref_type, ledger.ref_id, ledger.created_at
           FROM wallet_ledger ledger
           INNER JOIN users ledger_users ON ledger_users.id = ledger.user_id
           INNER JOIN assets ON assets.id = ledger.asset_id"#,
    );
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM wallet_ledger ledger
           INNER JOIN users ledger_users ON ledger_users.id = ledger.user_id
           INNER JOIN assets ON assets.id = ledger.asset_id"#,
    );
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if !filter.include_internal {
            push_exclude_internal_user_email(builder, "ledger_users.email");
        }
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "ledger.user_id", user_id);
        }
        push_user_email_filter(builder, "ledger.user_id", filter.email.clone());
        if let Some(asset_id) = filter.asset_id {
            builder.push(" AND ledger.asset_id = ");
            builder.push_bind(asset_id);
        }
        if let Some(change_type) = optional_string(filter.change_type.clone()) {
            builder.push(" AND ledger.change_type = ");
            builder.push_bind(change_type);
        }
        if let Some(ref_type) = optional_string(filter.ref_type.clone()) {
            builder.push(" AND ledger.ref_type = ");
            builder.push_bind(ref_type);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY ledger.created_at DESC, ledger.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 分页查询充值网络配置，返回符合调用方筛选条件的记录及相同谓词下的总数。
/// 充值网络配置列表与计数通过连接池分别执行且均不加锁；并发写入可能造成页数据与总数快照不同，SQL 或字段映射失败直接返回错误。
pub(crate) async fn list_admin_deposit_network_configs(
    pool: &Pool<MySql>,
    filter: AdminDepositNetworkConfigListFilter,
) -> AppResult<(Vec<AdminDepositNetworkConfigResponse>, i64)> {
    let mut rows = admin_deposit_network_config_query();
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM deposit_network_configs");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(network) = filter.network.clone() {
            builder.push(" AND network = ");
            builder.push_bind(network);
        }
        if let Some(address_group_code) = filter.address_group_code.clone() {
            builder.push(" AND address_group_code = ");
            builder.push_bind(address_group_code);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(asset_symbol) = filter.asset_symbol.clone() {
            builder.push(
                " AND (asset_symbols_json IS NULL OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(",
            );
            builder.push_bind(asset_symbol);
            builder.push(")))");
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY sort_order ASC, id ASC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按网络代码从连接池读取充值网络配置并映射为应用层所需的完整记录。
/// 充值网络配置不追加行锁，查询不创建事务；记录缺失时返回空值，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_deposit_network_config_by_network(
    pool: &Pool<MySql>,
    network: &str,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE network = ");
    builder.push_bind(network.to_owned());
    builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Validation("deposit network config is missing".to_owned()))
}

/// 在调用方事务中插入充值网络配置并返回或保留数据库写入结果。
/// 充值网络配置数据库唯一键冲突会映射为业务冲突；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
pub(crate) async fn insert_admin_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminDepositNetworkConfigWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO deposit_network_configs
           (network, display_name, address_group_code, address_group_name, asset_symbols_json, status, sort_order)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.network)
    .bind(&input.display_name)
    .bind(&input.address_group_code)
    .bind(&input.address_group_name)
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(input.sort_order)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_network_config_error)?;
    Ok(result.last_insert_id())
}

/// 在调用方事务中按传入主键或筛选条件更新充值网络配置，写入应用层已决定的目标字段。
/// 充值网络配置更新不检查受影响行数，唯一键冲突映射为业务冲突；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
pub(crate) async fn update_admin_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
    input: AdminDepositNetworkConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE deposit_network_configs
           SET network = ?,
               display_name = ?,
               address_group_code = ?,
               address_group_name = ?,
               asset_symbols_json = ?,
               status = ?,
               sort_order = ?
           WHERE id = ?"#,
    )
    .bind(&input.network)
    .bind(&input.display_name)
    .bind(&input.address_group_code)
    .bind(&input.address_group_name)
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(input.sort_order)
    .bind(config_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_network_config_error)?;
    Ok(())
}

/// 按传入主键或筛选条件从调用方事务快照读取充值网络配置并映射为应用层所需的完整记录。
/// 充值网络配置不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE id = ");
    builder.push_bind(config_id);
    builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中按传入主键或筛选条件以 `FOR UPDATE` 锁定充值网络配置并返回一致的修改前快照。
/// 充值网络配置锁由调用方事务持有至结束；函数不自行提交，记录缺失返回未找到，SQL 或解码失败交由外层回滚。
pub(crate) async fn lock_deposit_network_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE id = ");
    builder.push_bind(config_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中检查资产符号的实际 SQL 前置条件，阻止不符合约束的后续写入。
/// 校验只读取当前事务快照而不加行锁且不自行提交；条件不满足按实现返回校验/冲突/未找到，SQL 失败由调用方连同后续写入一起回滚。
pub(crate) async fn ensure_asset_symbols_exist(
    pool: &Pool<MySql>,
    symbols: &[String],
) -> AppResult<()> {
    for symbol in symbols {
        ensure_asset_symbol_exists(pool, symbol).await?;
    }
    Ok(())
}

/// 按网络、地址组、状态、资产、用户和地址片段分页检索充值地址池。
/// 调用方负责管理员权限；资产筛选同时覆盖地址默认资产、已分配资产及 JSON 白名单。
/// 列表和计数复用同一组谓词，不启事务、不锁定或回收地址，也不改变任何分配关系。
/// 查询失败无副作用；结果按更新时间和主键稳定倒序，供后台审计与运维查看。
pub(crate) async fn list_admin_deposit_address_pool(
    pool: &Pool<MySql>,
    filter: AdminDepositAddressPoolListFilter,
) -> AppResult<(Vec<AdminDepositAddressPoolResponse>, i64)> {
    let mut rows = admin_deposit_address_pool_query();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM deposit_address_pool addresses
           LEFT JOIN users assigned_users ON assigned_users.id = addresses.assigned_user_id"#,
    );
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(network) = filter.network.clone() {
            builder.push(" AND addresses.network = ");
            builder.push_bind(network);
        }
        if let Some(address_group_code) = filter.address_group_code.clone() {
            builder.push(" AND addresses.address_group_code = ");
            builder.push_bind(address_group_code);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND addresses.status = ");
            builder.push_bind(status);
        }
        if let Some(asset_symbol) = filter.asset_symbol.clone() {
            builder.push(" AND (addresses.asset_symbol = ");
            builder.push_bind(asset_symbol.clone());
            builder.push(" OR addresses.assigned_asset_symbol = ");
            builder.push_bind(asset_symbol.clone());
            builder.push(" OR JSON_CONTAINS(addresses.asset_symbols_json, JSON_QUOTE(");
            builder.push_bind(asset_symbol);
            builder.push("))");
            builder.push(")");
        }
        if let Some(user_id) = filter.assigned_user_id {
            push_user_id_filter(builder, "addresses.assigned_user_id", user_id);
        }
        push_user_email_filter(builder, "addresses.assigned_user_id", filter.email.clone());
        if let Some(address) = filter.address.clone() {
            builder.push(" AND addresses.address LIKE ");
            builder.push_bind(format!("%{address}%"));
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY addresses.updated_at DESC, addresses.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按传入主键或筛选条件从连接池读取充值地址池并映射为应用层所需的完整记录。
/// 充值地址池不追加行锁，查询不创建事务；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_deposit_address_pool(
    pool: &Pool<MySql>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE addresses.id = ");
    builder.push_bind(address_id);
    builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中插入充值地址池并返回或保留数据库写入结果。
/// 充值地址池数据库唯一键冲突会映射为业务冲突；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
pub(crate) async fn insert_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminDepositAddressPoolWrite,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let result = sqlx::query(
        r#"INSERT INTO deposit_address_pool
           (network, address_group_code, address, asset_symbol, asset_symbols_json, status, memo, remark)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.network)
    .bind(&input.address_group_code)
    .bind(&input.address)
    .bind(deposit_address_pool_legacy_asset_symbol(&input.asset_symbols))
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(&input.memo)
    .bind(&input.remark)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_address_error)?;
    load_deposit_address_pool_in_tx(tx, result.last_insert_id()).await
}

/// 在调用方事务中按传入主键或筛选条件更新充值地址池，写入应用层已决定的目标字段。
/// 充值地址池更新不检查受影响行数，唯一键冲突映射为业务冲突；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
pub(crate) async fn update_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
    input: AdminDepositAddressPoolWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE deposit_address_pool
           SET network = ?, address_group_code = ?, address = ?, asset_symbol = ?, asset_symbols_json = ?, status = ?, memo = ?, remark = ?
           WHERE id = ?"#,
    )
    .bind(&input.network)
    .bind(&input.address_group_code)
    .bind(&input.address)
    .bind(deposit_address_pool_legacy_asset_symbol(&input.asset_symbols))
    .bind(deposit_asset_symbols_json(&input.asset_symbols))
    .bind(&input.status)
    .bind(&input.memo)
    .bind(&input.remark)
    .bind(address_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_deposit_address_error)?;
    Ok(())
}

/// 在调用方事务中针对地址回收更新充值地址池，写入应用层已决定的目标字段。
/// 充值地址池更新不检查受影响行数；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
pub(crate) async fn reclaim_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE deposit_address_pool
           SET status = 'available',
               assigned_user_id = NULL,
               assigned_user_email = NULL,
               assigned_asset_symbol = NULL,
               assigned_at = NULL
           WHERE id = ?"#,
    )
    .bind(address_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按传入主键或筛选条件从调用方事务快照读取充值地址池并映射为应用层所需的完整记录。
/// 充值地址池不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE addresses.id = ");
    builder.push_bind(address_id);
    builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中按传入主键或筛选条件以 `FOR UPDATE` 锁定充值地址池并返回一致的修改前快照。
/// 充值地址池锁由调用方事务持有至结束；函数不自行提交，记录缺失返回未找到，SQL 或解码失败交由外层回滚。
pub(crate) async fn lock_deposit_address_pool_in_tx(
    tx: &mut Transaction<'_, MySql>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE addresses.id = ");
    builder.push_bind(address_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中锁定或创建代理钱包账户，增加可用余额并追加对应后台钱包流水。
/// 余额与流水必须同事务提交且金额须为正；同一引用由上层保证幂等，SQL 失败会使整笔入账回滚。
pub(crate) async fn credit_admin_wallet_available_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_or_create_admin_wallet_row_in_tx(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_admin_wallet_ledger_in_tx(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

/// 在调用方事务中确保用户资产钱包存在，并以 `FOR UPDATE` 返回账户余额快照供后续入账。
/// 创建零余额行和加锁读取不提交事务；调用方须继续在同一事务写余额及流水，SQL 失败整体回滚。
pub(crate) async fn lock_or_create_admin_wallet_row_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<AdminWalletRow> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    load_admin_wallet_row_in_tx(tx, user_id, asset_id).await
}

/// 在调用方事务中追加后台钱包流水，固化变更金额、余额桶及 available/frozen/locked 账后快照。
/// 流水必须与对应余额更新同事务提交；函数不自行去重或写后台审计，SQL 失败由用例整体回滚。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_admin_wallet_ledger_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_admin_wallet_row_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<AdminWalletRow> {
    sqlx::query_as::<_, AdminWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required".to_owned()))
}

fn admin_asset_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id,
                  symbol,
                  name,
                  logo_url,
                  precision_scale,
                  asset_type,
                  status,
                  deposit_enabled,
                  withdraw_enabled,
                  margin_transfer_enabled,
                  min_deposit_amount,
                  deposit_fee,
                  withdraw_fee,
                  COALESCE(withdraw_fee_tiers_json, JSON_ARRAY()) AS withdraw_fee_tiers,
                  created_at
           FROM assets"#,
    )
}

fn admin_deposit_network_config_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id,
                  network,
                  display_name,
                  address_group_code,
                  address_group_name,
                  COALESCE(asset_symbols_json, JSON_ARRAY()) AS asset_symbols,
                  status,
                  sort_order,
                  created_at,
                  updated_at
           FROM deposit_network_configs"#,
    )
}

fn admin_deposit_address_pool_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT addresses.id,
                  addresses.network,
                  addresses.address_group_code,
                  addresses.address,
                  addresses.asset_symbol,
                  COALESCE(
                      addresses.asset_symbols_json,
                      IF(addresses.asset_symbol IS NULL, JSON_ARRAY(), JSON_ARRAY(addresses.asset_symbol))
                  ) AS asset_symbols,
                  addresses.status,
                  addresses.assigned_user_id,
                  COALESCE(addresses.assigned_user_email, assigned_users.email) AS assigned_user_email,
                  addresses.assigned_asset_symbol,
                  addresses.assigned_at,
                  addresses.memo,
                  addresses.remark,
                  addresses.created_at,
                  addresses.updated_at
           FROM deposit_address_pool addresses
           LEFT JOIN users assigned_users ON assigned_users.id = addresses.assigned_user_id"#,
    )
}

fn map_duplicate_asset_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("asset already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_deposit_network_config_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("deposit network config already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_deposit_address_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("deposit address already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

async fn ensure_asset_symbol_exists(pool: &Pool<MySql>, symbol: &str) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM assets WHERE symbol = ? AND status = 'active'",
    )
    .bind(symbol)
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

fn deposit_address_pool_legacy_asset_symbol(asset_symbols: &[String]) -> Option<String> {
    match asset_symbols {
        [symbol] => Some(symbol.clone()),
        _ => None,
    }
}

fn deposit_asset_symbols_json(asset_symbols: &[String]) -> Option<SqlxJson<Vec<String>>> {
    if asset_symbols.is_empty() {
        None
    } else {
        Some(SqlxJson(asset_symbols.to_vec()))
    }
}

async fn append_empty_wallet_accounts(
    pool: &Pool<MySql>,
    filter: &AdminWalletAccountListFilter,
    accounts: &mut Vec<AdminWalletAccountResponse>,
) -> AppResult<()> {
    let Some(user_id) = resolve_user_id_filter(pool, filter.user_id, filter.email.clone()).await?
    else {
        return Ok(());
    };
    let Some(user_email) =
        sqlx::query_scalar::<_, Option<String>>("SELECT email FROM users WHERE id = ? LIMIT 1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
    else {
        return Ok(());
    };
    if !filter.include_internal && user_email.as_deref().is_some_and(is_internal_user_email) {
        return Ok(());
    }
    let existing_asset_ids = accounts
        .iter()
        .map(|account| account.asset_id)
        .collect::<BTreeSet<_>>();
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id AS asset_id, symbol AS asset_symbol
           FROM assets
           WHERE status = 'active'"#,
    );
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND id = ");
        builder.push_bind(asset_id);
    }
    builder.push(" ORDER BY symbol ASC LIMIT ");
    builder.push_bind(filter.limit as i64);

    let assets = builder
        .build_query_as::<AdminWalletEmptyAssetRow>()
        .fetch_all(pool)
        .await?;
    let zero = BigDecimal::from(0).with_scale(18);
    let now = Utc::now();
    accounts.extend(
        assets
            .into_iter()
            .filter(|asset| !existing_asset_ids.contains(&asset.asset_id))
            .map(|asset| AdminWalletAccountResponse {
                id: None,
                user_id,
                user_email: user_email.clone(),
                asset_id: asset.asset_id,
                asset_symbol: asset.asset_symbol,
                available: zero.clone(),
                frozen: zero.clone(),
                locked: zero.clone(),
                account_exists: false,
                updated_at: now,
            }),
    );
    Ok(())
}
