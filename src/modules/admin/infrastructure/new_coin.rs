use super::*;

#[derive(Debug)]
pub(crate) struct AdminNewCoinFlatListFilter {
    pub(crate) project_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinLockPositionListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinUnlockListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) fee_paid_status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinProjectInsert {
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) unlock_type: String,
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinUnlockRuleUpdate {
    pub(crate) unlock_type: String,
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinUnlockFeeRuleUpdate {
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinConvertRuleWrite {
    pub(crate) convert_pair_id: u64,
    pub(crate) rate_source: String,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) floating_rate_json: Option<Value>,
    pub(crate) status: String,
    pub(crate) admin_id: u64,
}

pub(crate) async fn list_admin_new_coin_projects(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<NewCoinProjectResponse>> {
    let mut builder = admin_new_coin_project_query();
    builder.push(" ORDER BY projects.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    Ok(builder
        .build_query_as::<NewCoinProjectResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_subscriptions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<Vec<NewCoinSubscriptionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, quote_asset, quote_amount, requested_quantity,
                  allocated_quantity, status, idempotency_key, created_at
           FROM new_coin_subscriptions
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = filter.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(
        &mut builder,
        filter.user_id,
        filter.email,
        filter.status,
    );
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinSubscriptionResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_distributions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<Vec<NewCoinDistributionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                  lock_position_id, status, idempotency_key, created_at
           FROM new_coin_distributions
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = filter.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(
        &mut builder,
        filter.user_id,
        filter.email,
        filter.status,
    );
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinDistributionResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_purchases(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<Vec<NewCoinPurchaseResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                  quote_amount, lock_position_id, status, idempotency_key, created_at
           FROM new_coin_purchase_orders
           WHERE 1 = 1"#,
    );
    if let Some(project_id) = filter.project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(project_id);
    }
    push_optional_user_and_status_filters(
        &mut builder,
        filter.user_id,
        filter.email,
        filter.status,
    );
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinPurchaseResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_lock_positions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinLockPositionListFilter,
) -> AppResult<Vec<NewCoinLockPositionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, unlock_type, unlock_at, locked_amount,
                  released_amount, remaining_amount, merge_key, status, created_at
           FROM asset_lock_positions
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinLockPositionResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_new_coin_unlocks(
    pool: &Pool<MySql>,
    filter: AdminNewCoinUnlockListFilter,
) -> AppResult<Vec<NewCoinUnlockResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
                  unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  unlock_fee_amount, fee_paid_status, status, idempotency_key, created_at
           FROM asset_unlock_records
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(asset_id) = filter.asset_id {
        builder.push(" AND asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    if let Some(fee_paid_status) = filter.fee_paid_status {
        builder.push(" AND fee_paid_status = ");
        builder.push_bind(fee_paid_status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    Ok(builder
        .build_query_as::<NewCoinUnlockResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn insert_admin_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminNewCoinProjectInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
            unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
            unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(input.asset_id)
    .bind(&input.symbol)
    .bind(&input.lifecycle_status)
    .bind(&input.total_supply)
    .bind(&input.issue_price)
    .bind(input.listed_at)
    .bind(&input.unlock_type)
    .bind(input.fixed_unlock_at)
    .bind(input.relative_unlock_seconds)
    .bind(input.unlock_fee_enabled)
    .bind(&input.unlock_fee_rate)
    .bind(input.unlock_fee_basis.as_deref())
    .bind(input.unlock_fee_asset)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_new_coin_project_lifecycle_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    lifecycle_status: &str,
    listed_at: Option<DateTime<Utc>>,
) -> AppResult<()> {
    sqlx::query("UPDATE new_coin_projects SET lifecycle_status = ?, listed_at = ? WHERE id = ?")
        .bind(lifecycle_status)
        .bind(listed_at)
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn update_admin_new_coin_project_unlock_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    input: AdminNewCoinUnlockRuleUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE new_coin_projects
           SET unlock_type = ?, listed_at = ?, fixed_unlock_at = ?, relative_unlock_seconds = ?
           WHERE id = ?"#,
    )
    .bind(&input.unlock_type)
    .bind(input.listed_at)
    .bind(input.fixed_unlock_at)
    .bind(input.relative_unlock_seconds)
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_new_coin_project_unlock_fee_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    input: AdminNewCoinUnlockFeeRuleUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE new_coin_projects
           SET unlock_fee_enabled = ?, unlock_fee_rate = ?, unlock_fee_basis = ?, unlock_fee_asset = ?
           WHERE id = ?"#,
    )
    .bind(input.unlock_fee_enabled)
    .bind(input.unlock_fee_rate.as_ref())
    .bind(input.unlock_fee_basis.as_deref())
    .bind(input.unlock_fee_asset)
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn activate_admin_new_coin_post_listing_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    sqlx::query("UPDATE trading_pairs SET status = 'active' WHERE id = ?")
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn enable_admin_new_coin_post_listing_purchase_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    pair_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE new_coin_projects SET post_listing_purchase_enabled = TRUE, post_listing_pair_id = ? WHERE id = ?",
    )
    .bind(pair_id)
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn disable_admin_new_coin_post_listing_purchase_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE new_coin_projects SET post_listing_purchase_enabled = FALSE, post_listing_pair_id = NULL WHERE id = ?",
    )
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn insert_admin_new_coin_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    user_id: u64,
    subscription_id: Option<u64>,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_position_id: Option<u64>,
    status: &str,
    idempotency_key: &str,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO new_coin_distributions
           (project_id, user_id, subscription_id, asset_id, quantity, lock_position_id,
            status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(subscription_id)
    .bind(asset_id)
    .bind(quantity)
    .bind(lock_position_id)
    .bind(status)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_distribution_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn insert_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: &AdminNewCoinConvertRuleWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO new_coin_convert_rules
           (convert_pair_id, rate_source, fixed_rate, floating_rate_json, status, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.convert_pair_id)
    .bind(&input.rate_source)
    .bind(&input.fixed_rate)
    .bind(input.floating_rate_json.clone().map(SqlxJson))
    .bind(&input.status)
    .bind(input.admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
    input: &AdminNewCoinConvertRuleWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE new_coin_convert_rules
           SET rate_source = ?, fixed_rate = ?, floating_rate_json = ?, status = ?, created_by = ?
           WHERE id = ?"#,
    )
    .bind(&input.rate_source)
    .bind(&input.fixed_rate)
    .bind(input.floating_rate_json.clone().map(SqlxJson))
    .bind(&input.status)
    .bind(input.admin_id)
    .bind(rule_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_admin_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<NewCoinProjectResponse> {
    let mut builder = admin_new_coin_project_query();
    builder.push(" WHERE projects.id = ");
    builder.push_bind(project_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<NewCoinProjectResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<NewCoinProjectResponse> {
    let mut builder = admin_new_coin_project_query();
    builder.push(" WHERE projects.id = ");
    builder.push_bind(project_id);
    builder.push(" LIMIT 1 FOR UPDATE");
    builder
        .build_query_as::<NewCoinProjectResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_admin_new_coin_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    distribution_id: u64,
) -> AppResult<NewCoinDistributionResponse> {
    sqlx::query_as::<_, NewCoinDistributionResponse>(
        r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                  lock_position_id, status, idempotency_key, created_at
           FROM new_coin_distributions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(distribution_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn load_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<NewCoinConvertRuleResponse> {
    sqlx::query_as::<_, NewCoinConvertRuleResponse>(
        r#"SELECT id, convert_pair_id, rate_source, fixed_rate, floating_rate_json, status, created_by
           FROM new_coin_convert_rules
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_new_coin_convert_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    convert_pair_id: u64,
) -> AppResult<Option<NewCoinConvertRuleResponse>> {
    Ok(sqlx::query_as::<_, NewCoinConvertRuleResponse>(
        r#"SELECT id, convert_pair_id, rate_source, fixed_rate, floating_rate_json, status, created_by
           FROM new_coin_convert_rules
           WHERE convert_pair_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(convert_pair_id)
    .fetch_optional(&mut **tx)
    .await?)
}

pub(crate) async fn ensure_admin_new_coin_post_listing_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    project_asset_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(pair_id)
    .bind(project_asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

pub(crate) async fn admin_new_coin_idempotency_key_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    table_name: &str,
    idempotency_key: &str,
) -> AppResult<bool> {
    let mut query = QueryBuilder::<MySql>::new("SELECT id FROM ");
    query
        .push(table_name)
        .push(" WHERE idempotency_key = ")
        .push_bind(idempotency_key)
        .push(" LIMIT 1 FOR UPDATE");
    let exists: Option<(u64,)> = query.build_query_as().fetch_optional(&mut **tx).await?;
    Ok(exists.is_some())
}

pub(crate) async fn insert_admin_new_coin_lifecycle_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    action: &str,
    payload_json: Value,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO new_coin_lifecycle_events (project_id, event_type, payload_json, created_by)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(project_id)
    .bind(action)
    .bind(SqlxJson(payload_json))
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn apply_admin_new_coin_subscription_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    project_id: u64,
    user_id: u64,
    quantity: &BigDecimal,
) -> AppResult<()> {
    let Some((requested_quantity, allocated_quantity)): Option<(BigDecimal, BigDecimal)> =
        sqlx::query_as(
            r#"SELECT requested_quantity, allocated_quantity
               FROM new_coin_subscriptions
               WHERE id = ? AND project_id = ? AND user_id = ?
               LIMIT 1
               FOR UPDATE"#,
        )
        .bind(subscription_id)
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
    else {
        return Err(AppError::NotFound);
    };

    let allocated_after = allocated_quantity + quantity.clone();
    if allocated_after > requested_quantity {
        return Err(AppError::Validation(
            "distribution quantity exceeds requested subscription quantity".to_owned(),
        ));
    }
    let status = if allocated_after == requested_quantity {
        "allocated"
    } else {
        "partial_allocated"
    };

    sqlx::query(
        "UPDATE new_coin_subscriptions SET allocated_quantity = ?, status = ? WHERE id = ?",
    )
    .bind(&allocated_after)
    .bind(status)
    .bind(subscription_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn apply_admin_new_coin_distribution_allocation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_positions: &[AdminNewCoinLockPositionWrite],
    ledger: AdminNewCoinLedgerWrite<'_>,
) -> AppResult<Option<u64>> {
    if lock_positions.is_empty() {
        credit_admin_wallet_available_in_tx(
            tx,
            user_id,
            asset_id,
            quantity,
            ledger.change_type,
            ledger.ref_type,
            ledger.ref_id,
        )
        .await?;
        return Ok(None);
    }

    let wallet = lock_or_create_admin_wallet_row_in_tx(tx, user_id, asset_id).await?;
    let locked_after = wallet.locked.clone() + quantity.clone();
    sqlx::query("UPDATE wallet_accounts SET locked = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&locked_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_admin_wallet_ledger_in_tx(
        tx,
        user_id,
        asset_id,
        quantity.clone(),
        "locked",
        &locked_after,
        &wallet.available,
        &wallet.frozen,
        &locked_after,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await?;

    let mut first_lock_position_id = None;
    for position in lock_positions {
        let position_id = upsert_admin_new_coin_lock_position(tx, position).await?;
        if first_lock_position_id.is_none() {
            first_lock_position_id = Some(position_id);
        }
    }
    Ok(first_lock_position_id)
}

async fn upsert_admin_new_coin_lock_position(
    tx: &mut Transaction<'_, MySql>,
    position: &AdminNewCoinLockPositionWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount,
            released_amount, remaining_amount, merge_key, status)
           VALUES (?, ?, ?, ?, 0, 0, 0, ?, 'active')
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(position.user_id)
    .bind(position.asset_id)
    .bind(&position.unlock_type)
    .bind(position.unlock_at.naive_utc())
    .bind(&position.merge_key)
    .execute(&mut **tx)
    .await?;

    let position_id = if result.last_insert_id() == 0 {
        sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_lock_positions WHERE merge_key = ? LIMIT 1 FOR UPDATE",
        )
        .bind(&position.merge_key)
        .fetch_one(&mut **tx)
        .await?
        .0
    } else {
        result.last_insert_id()
    };

    let inserted = sqlx::query(
        r#"INSERT IGNORE INTO asset_lock_position_sources
           (lock_position_id, source_type, source_id, source_amount, source_time)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(position_id)
    .bind(&position.source_type)
    .bind(&position.source_id)
    .bind(&position.amount)
    .bind(position.source_time.naive_utc())
    .execute(&mut **tx)
    .await?;

    if inserted.rows_affected() > 0 {
        sqlx::query(
            r#"UPDATE asset_lock_positions
               SET locked_amount = locked_amount + ?,
                   remaining_amount = remaining_amount + ?,
                   status = 'active'
               WHERE id = ?"#,
        )
        .bind(&position.amount)
        .bind(&position.amount)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(position_id)
}

fn admin_new_coin_project_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT projects.id, projects.asset_id, projects.symbol, projects.lifecycle_status,
                  projects.total_supply, projects.issue_price, projects.listed_at,
                  projects.unlock_type, projects.fixed_unlock_at, projects.relative_unlock_seconds,
                  projects.unlock_fee_enabled, projects.unlock_fee_rate, projects.unlock_fee_basis,
                  projects.unlock_fee_asset, projects.status, projects.post_listing_purchase_enabled,
                  projects.post_listing_pair_id, post_listing_pair.status AS post_listing_pair_status
           FROM new_coin_projects projects
           LEFT JOIN trading_pairs post_listing_pair ON post_listing_pair.id = projects.post_listing_pair_id"#,
    )
}

fn map_duplicate_distribution_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("new coin distribution has already been created".to_owned())
    } else {
        AppError::Database(error)
    }
}
