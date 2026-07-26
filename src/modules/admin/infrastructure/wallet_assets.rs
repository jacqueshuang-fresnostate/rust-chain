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
}

#[derive(Debug)]
pub(crate) struct AdminDepositNetworkConfigListFilter {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) limit: u32,
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

pub(crate) async fn list_admin_assets(
    pool: &Pool<MySql>,
    filter: AdminAssetListFilter,
) -> AppResult<Vec<AdminAssetResponse>> {
    let mut builder = admin_asset_query();
    builder.push(" WHERE 1 = 1");
    if let Some(symbol) = filter.symbol {
        builder.push(" AND symbol = ");
        builder.push_bind(symbol);
    }
    if let Some(asset_type) = filter.asset_type {
        builder.push(" AND asset_type = ");
        builder.push_bind(asset_type);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminAssetResponse>()
        .fetch_all(pool)
        .await?)
}

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

pub(crate) async fn insert_admin_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAssetInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO assets
              (symbol, name, logo_url, precision_scale, asset_type, status, deposit_enabled, withdraw_enabled,
               min_deposit_amount, deposit_fee, withdraw_fee, withdraw_fee_tiers_json)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.symbol)
    .bind(&input.name)
    .bind(&input.logo_url)
    .bind(input.precision_scale)
    .bind(&input.asset_type)
    .bind(&input.status)
    .bind(input.deposit_enabled)
    .bind(input.withdraw_enabled)
    .bind(&input.min_deposit_amount)
    .bind(&input.deposit_fee)
    .bind(&input.withdraw_fee)
    .bind(SqlxJson(input.withdraw_fee_tiers))
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_asset_error)?;
    Ok(result.last_insert_id())
}

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
    .bind(&input.min_deposit_amount)
    .bind(&input.deposit_fee)
    .bind(&input.withdraw_fee)
    .bind(SqlxJson(input.withdraw_fee_tiers))
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

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

pub(crate) async fn list_admin_wallet_accounts(
    pool: &Pool<MySql>,
    filter: AdminWalletAccountListFilter,
) -> AppResult<Vec<AdminWalletAccountResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT accounts.id, accounts.user_id, account_users.email AS user_email,
                  accounts.asset_id, assets.symbol AS asset_symbol,
                  accounts.available, accounts.frozen, accounts.locked, TRUE AS account_exists, accounts.updated_at
           FROM wallet_accounts accounts
           INNER JOIN users account_users ON account_users.id = accounts.user_id
           INNER JOIN assets ON assets.id = accounts.asset_id
           WHERE 1 = 1"#,
    );
    if !filter.include_internal {
        push_exclude_internal_user_email(&mut builder, "account_users.email");
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "accounts.user_id", user_id);
    }
    push_user_email_filter(&mut builder, "accounts.user_id", filter.email.clone());
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND accounts.asset_id = ");
        builder.push_bind(asset_id);
    }
    builder.push(" ORDER BY accounts.updated_at DESC, accounts.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    let mut accounts = builder
        .build_query_as::<AdminWalletAccountResponse>()
        .fetch_all(pool)
        .await?;
    if filter.include_empty {
        append_empty_wallet_accounts(pool, &filter, &mut accounts).await?;
    }
    Ok(accounts)
}

pub(crate) async fn list_admin_wallet_ledger(
    pool: &Pool<MySql>,
    filter: AdminWalletLedgerListFilter,
) -> AppResult<Vec<AdminWalletLedgerResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT ledger.id, ledger.user_id, ledger_users.email AS user_email,
                  ledger.asset_id, assets.symbol AS asset_symbol,
                  ledger.change_type, ledger.amount, ledger.balance_type, ledger.balance_after,
                  ledger.available_after, ledger.frozen_after, ledger.locked_after,
                  ledger.ref_type, ledger.ref_id, ledger.created_at
           FROM wallet_ledger ledger
           INNER JOIN users ledger_users ON ledger_users.id = ledger.user_id
           INNER JOIN assets ON assets.id = ledger.asset_id
           WHERE 1 = 1"#,
    );
    if !filter.include_internal {
        push_exclude_internal_user_email(&mut builder, "ledger_users.email");
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "ledger.user_id", user_id);
    }
    push_user_email_filter(&mut builder, "ledger.user_id", filter.email);
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND ledger.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(change_type) = optional_string(filter.change_type) {
        builder.push(" AND ledger.change_type = ");
        builder.push_bind(change_type);
    }
    if let Some(ref_type) = optional_string(filter.ref_type) {
        builder.push(" AND ledger.ref_type = ");
        builder.push_bind(ref_type);
    }
    builder.push(" ORDER BY ledger.created_at DESC, ledger.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminWalletLedgerResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_deposit_network_configs(
    pool: &Pool<MySql>,
    filter: AdminDepositNetworkConfigListFilter,
) -> AppResult<Vec<AdminDepositNetworkConfigResponse>> {
    let mut builder = admin_deposit_network_config_query();
    builder.push(" WHERE 1 = 1");
    if let Some(network) = filter.network {
        builder.push(" AND network = ");
        builder.push_bind(network);
    }
    if let Some(address_group_code) = filter.address_group_code {
        builder.push(" AND address_group_code = ");
        builder.push_bind(address_group_code);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(asset_symbol) = filter.asset_symbol {
        builder.push(
            " AND (asset_symbols_json IS NULL OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(",
        );
        builder.push_bind(asset_symbol);
        builder.push(")))");
    }
    builder.push(" ORDER BY sort_order ASC, id ASC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminDepositNetworkConfigResponse>()
        .fetch_all(pool)
        .await?)
}

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

pub(crate) async fn ensure_asset_symbols_exist(
    pool: &Pool<MySql>,
    symbols: &[String],
) -> AppResult<()> {
    for symbol in symbols {
        ensure_asset_symbol_exists(pool, symbol).await?;
    }
    Ok(())
}

pub(crate) async fn list_admin_deposit_address_pool(
    pool: &Pool<MySql>,
    filter: AdminDepositAddressPoolListFilter,
) -> AppResult<Vec<AdminDepositAddressPoolResponse>> {
    let mut builder = admin_deposit_address_pool_query();
    builder.push(" WHERE 1 = 1");
    if let Some(network) = filter.network {
        builder.push(" AND addresses.network = ");
        builder.push_bind(network);
    }
    if let Some(address_group_code) = filter.address_group_code {
        builder.push(" AND addresses.address_group_code = ");
        builder.push_bind(address_group_code);
    }
    if let Some(status) = filter.status {
        builder.push(" AND addresses.status = ");
        builder.push_bind(status);
    }
    if let Some(asset_symbol) = filter.asset_symbol {
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
        push_user_id_filter(&mut builder, "addresses.assigned_user_id", user_id);
    }
    push_user_email_filter(&mut builder, "addresses.assigned_user_id", filter.email);
    if let Some(address) = filter.address {
        builder.push(" AND addresses.address LIKE ");
        builder.push_bind(format!("%{address}%"));
    }
    builder.push(" ORDER BY addresses.updated_at DESC, addresses.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminDepositAddressPoolResponse>()
        .fetch_all(pool)
        .await?)
}

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
