use super::*;
use crate::modules::new_coin::service::{
    calculate_unlock_fee_fields, new_coin_unlock_idempotency_key, quantize_unlock_fee_amount,
};

#[derive(Debug)]
pub(crate) struct AdminNewCoinFlatListFilter {
    pub(crate) project_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinLockPositionListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinUnlockListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) fee_paid_status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewCoinProjectInsert {
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    pub(crate) quote_asset_id: u64,
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

/// 分页查询新币项目，返回符合调用方筛选条件的记录及相同谓词下的总数。
/// 新币项目列表与计数通过连接池分别执行且均不加锁；并发写入可能造成页数据与总数快照不同，SQL 或字段映射失败直接返回错误。
pub(crate) async fn list_admin_new_coin_projects(
    pool: &Pool<MySql>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<NewCoinProjectResponse>, i64)> {
    let total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM new_coin_projects projects");
    fetch_admin_page(
        pool,
        admin_new_coin_project_query(),
        total,
        " ORDER BY projects.id DESC",
        limit,
        offset,
    )
    .await
}

/// 按项目、用户、邮箱和状态筛选新币认购记录，分页返回认购、已派发数量及总数。
/// 列表与 COUNT 分别通过连接池执行且不锁认购行；并发认购或派发可能造成页数据与总数快照不同，SQL/映射失败返回错误。
pub(crate) async fn list_admin_new_coin_subscriptions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<(Vec<NewCoinSubscriptionResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, quote_asset, quote_amount, requested_quantity,
                  allocated_quantity, status, idempotency_key, created_at
           FROM new_coin_subscriptions"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM new_coin_subscriptions");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(project_id) = filter.project_id {
            builder.push(" AND project_id = ");
            builder.push_bind(project_id);
        }
        push_optional_user_and_status_filters(
            builder,
            filter.user_id,
            filter.email.clone(),
            filter.status.clone(),
        );
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

/// 按项目、用户、邮箱和状态筛选新币派发记录，分页返回数量、业务幂等键、锁仓编号及总数。
/// 两条连接池查询共用谓词并按记录 ID 倒序但不加锁；并发派发可能导致本页与总数不处于同一快照。
pub(crate) async fn list_admin_new_coin_distributions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<(Vec<NewCoinDistributionResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                  lock_position_id, status, idempotency_key, created_at
           FROM new_coin_distributions"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM new_coin_distributions");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(project_id) = filter.project_id {
            builder.push(" AND project_id = ");
            builder.push_bind(project_id);
        }
        push_optional_user_and_status_filters(
            builder,
            filter.user_id,
            filter.email.clone(),
            filter.status.clone(),
        );
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

/// 按项目、用户、邮箱和状态筛选新币购买记录，分页返回支付/购得数量、锁仓编号和总数。
/// 列表及计数读取不锁购买、项目或钱包记录；并发购买可能改变计数口径，数据库或十进制字段解码失败直接返回错误。
pub(crate) async fn list_admin_new_coin_purchases(
    pool: &Pool<MySql>,
    filter: AdminNewCoinFlatListFilter,
) -> AppResult<(Vec<NewCoinPurchaseResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                  quote_amount, lock_position_id, status, idempotency_key, created_at
           FROM new_coin_purchase_orders"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM new_coin_purchase_orders");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(project_id) = filter.project_id {
            builder.push(" AND project_id = ");
            builder.push_bind(project_id);
        }
        push_optional_user_and_status_filters(
            builder,
            filter.user_id,
            filter.email.clone(),
            filter.status.clone(),
        );
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

/// 按项目、用户、邮箱和状态筛选新币锁仓头寸，分页返回总量、已解锁量、规则快照及总数。
/// 查询不锁锁仓或钱包，列表与 COUNT 可能受并发解锁影响而来自不同快照；JSON/数值映射或 SQL 失败返回错误。
pub(crate) async fn list_admin_new_coin_lock_positions(
    pool: &Pool<MySql>,
    filter: AdminNewCoinLockPositionListFilter,
) -> AppResult<(Vec<NewCoinLockPositionResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, unlock_type, unlock_at, locked_amount,
                  released_amount, remaining_amount, merge_key, status, created_at
           FROM asset_lock_positions"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM asset_lock_positions");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "user_id", user_id);
        }
        push_user_email_filter(builder, "user_id", filter.email.clone());
        if let Some(asset_id) = filter.asset_id {
            builder.push(" AND asset_id = ");
            builder.push_bind(asset_id);
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

/// 按项目、用户、邮箱和锁仓编号筛选新币解锁流水，分页返回净额、手续费、业务键及总数。
/// 列表和 COUNT 使用同一谓词、按解锁记录 ID 倒序且不锁资金行；并发解锁可能造成快照差异，SQL/金额映射失败返回错误。
pub(crate) async fn list_admin_new_coin_unlocks(
    pool: &Pool<MySql>,
    filter: AdminNewCoinUnlockListFilter,
) -> AppResult<(Vec<NewCoinUnlockResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
                  unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  unlock_fee_amount, fee_paid_status, status, idempotency_key, created_at
           FROM asset_unlock_records"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM asset_unlock_records");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "user_id", user_id);
        }
        push_user_email_filter(builder, "user_id", filter.email.clone());
        if let Some(asset_id) = filter.asset_id {
            builder.push(" AND asset_id = ");
            builder.push_bind(asset_id);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(fee_paid_status) = filter.fee_paid_status.clone() {
            builder.push(" AND fee_paid_status = ");
            builder.push_bind(fee_paid_status);
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

/// 在调用方事务中插入新币项目并返回或保留数据库写入结果。
/// 新币项目函数不提供独立幂等保证，约束冲突沿用数据库错误；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
pub(crate) async fn insert_admin_new_coin_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminNewCoinProjectInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, quote_asset_id,
            reserved_supply, allocated_supply, remaining_supply, listed_at,
            unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
            unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(input.asset_id)
    .bind(&input.symbol)
    .bind(&input.lifecycle_status)
    .bind(&input.total_supply)
    .bind(&input.issue_price)
    .bind(input.quote_asset_id)
    .bind(&input.total_supply)
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

/// 在调用方事务中更新新币项目生命周期状态及上市时间，字段值由应用层迁移规则预先决定。
/// 函数不提交事务或写审计；SQL 失败由上层连同生命周期事件和其他变更整体回滚。
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

/// 在调用方事务中更新新币项目的解锁类型、上市时间及固定或相对解锁时点。
/// 函数不验证规则形状、不提交或写审计；SQL 失败由应用层连同项目变更整体回滚。
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

/// 在调用方事务中更新新币解锁手续费开关、费率、计费基数和收费资产。
/// 函数不校验费率组合、不提交或写审计；SQL 失败由应用层连同项目变更整体回滚。
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

/// 在调用方事务中把指定上市后交易对状态直接设为 active。
/// 更新不检查受影响行数；调用方须先确认交易对 base_asset 匹配项目资产并持有其行锁，函数不修改项目购买开关。
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

/// 在调用方事务中启用项目上市后购买并保存其交易对 ID。
/// 更新不检查受影响行数；调用方须先锁定 listed 项目和匹配交易对，并把交易对激活、项目回读及审计统一提交。
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

/// 在调用方事务中关闭项目上市后购买并清空 post_listing_pair_id。
/// 更新不检查受影响行数，也不会停用此前交易对；调用方须先锁定 listed 项目并负责回读、审计和提交。
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

#[allow(clippy::too_many_arguments)] // SQL 行写入字段与表结构一一对应，聚合会掩盖审计列语义。
/// 在调用方事务中插入新币派发记录并返回或保留数据库写入结果。
/// 新币派发记录数据库唯一键冲突会映射为业务冲突；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
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

/// 在调用方事务中插入新币闪兑规则并返回或保留数据库写入结果。
/// 新币闪兑规则函数不提供独立幂等保证，约束冲突沿用数据库错误；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
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

/// 在调用方事务中按传入主键或筛选条件更新新币闪兑规则，写入应用层已决定的目标字段。
/// 新币闪兑规则更新不检查受影响行数；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
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

/// 按传入主键或筛选条件从调用方事务快照读取新币项目并映射为应用层所需的完整记录。
/// 新币项目不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
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

/// 在调用方事务中按传入主键或筛选条件以 `FOR UPDATE` 锁定新币项目并返回一致的修改前快照。
/// 新币项目锁由调用方事务持有至结束；函数不自行提交，记录缺失返回未找到，SQL 或解码失败交由外层回滚。
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

/// 按传入主键或筛选条件从调用方事务快照读取新币派发记录并映射为应用层所需的完整记录。
/// 新币派发记录不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
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

/// 按传入主键或筛选条件从调用方事务快照读取新币闪兑规则并映射为应用层所需的完整记录。
/// 新币闪兑规则不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
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

/// 在调用方事务中按传入主键或筛选条件以 `FOR UPDATE` 锁定新币闪兑规则并返回一致的修改前快照。
/// 新币闪兑规则锁由调用方事务持有至结束；函数不自行提交，记录缺失按可选结果返回，SQL 或解码失败交由外层回滚。
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

/// 在调用方事务中锁定指定交易对并确认其 base_asset 正是新币项目资产。
/// `FOR UPDATE` 只锁满足 ID 与 base_asset 条件的交易对；不匹配返回未找到，函数不要求交易对 active，调用方负责后续启用和审计。
pub(crate) async fn ensure_admin_new_coin_post_listing_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    project_asset_id: u64,
    project_quote_asset_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ? AND quote_asset = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(pair_id)
    .bind(project_asset_id)
    .bind(project_quote_asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

/// 在调用方事务中锁定查询指定业务表的幂等键，判断派发或规则写入是否已经执行。
/// 表名仅由内部受控调用方传入，幂等键使用绑定参数；函数不提交事务，SQL 失败由上层回滚。
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

/// 在调用方事务中插入新币生命周期事件并返回或保留数据库写入结果。
/// 新币生命周期事件函数不提供独立幂等保证，约束冲突沿用数据库错误；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
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

/// 锁定指定新币认购记录，累加本次分配量并按是否足额更新为部分派发或已派发状态。
/// 分配后数量不得超过认购数量；函数不提交事务，记录缺失、超额或 SQL 失败由派发用例整体回滚。
pub(crate) async fn apply_admin_new_coin_subscription_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    project_id: u64,
    user_id: u64,
    quantity: &BigDecimal,
) -> AppResult<BigDecimal> {
    let Some((requested_quantity, allocated_quantity, issue_price)): Option<(
        BigDecimal,
        BigDecimal,
        BigDecimal,
    )> = sqlx::query_as(
        r#"SELECT requested_quantity, allocated_quantity, issue_price
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
    Ok(issue_price * quantity.clone())
}

/// 在调用方事务中把新币分配量记入可用余额，或记入锁定余额并建立锁仓来源。
/// 调用方须先校验正数数量、锁仓计划总量与分配量一致，并拦截已处理的业务幂等键。
/// 账户行先锁定后更新，钱包余额、账后快照和锁仓明细必须由同一事务一起提交。
/// 无锁仓计划时返回空值并直接加可用余额；有计划时返回首个锁仓仓位编号。
/// 锁仓来源依靠唯一键防重复，但钱包加账不独立幂等；失败须由调用方回滚整个分配事务。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_admin_new_coin_distribution_allocation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_positions: &[AdminNewCoinLockPositionWrite],
    project: &NewCoinProjectResponse,
    purchase_cost: &BigDecimal,
    unlock_fee_precision: Option<i32>,
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
        ensure_admin_new_coin_unlock_record_in_tx(
            tx,
            user_id,
            asset_id,
            position_id,
            &position.amount,
            project,
            purchase_cost,
            unlock_fee_precision,
            &position.source_id,
        )
        .await?;
        if first_lock_position_id.is_none() {
            first_lock_position_id = Some(position_id);
        }
    }
    Ok(first_lock_position_id)
}

/// 派发计算相对解禁起点前先取得目标钱包锁，避免资金锁等待被误计入用户锁仓周期。
pub(crate) async fn lock_admin_new_coin_distribution_wallet_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    lock_or_create_admin_wallet_row_in_tx(tx, user_id, asset_id).await?;
    Ok(())
}

/// 后台派发产生锁仓时同步固化解禁应收；之后调整项目费率不会改写该批记录。
#[allow(clippy::too_many_arguments)]
async fn ensure_admin_new_coin_unlock_record_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    quantity: &BigDecimal,
    project: &NewCoinProjectResponse,
    purchase_cost: &BigDecimal,
    unlock_fee_precision: Option<i32>,
    source_id: &str,
) -> AppResult<()> {
    let (mut fee_paid_status, mut unlock_fee_amount) = calculate_unlock_fee_fields(
        project.unlock_fee_enabled,
        project.unlock_fee_rate.as_ref(),
        project.unlock_fee_basis.as_deref(),
        project.unlock_fee_asset,
        quantity,
        &project.issue_price,
        purchase_cost,
    )?;
    if let (Some(_), Some(raw_fee_amount)) = (project.unlock_fee_asset, unlock_fee_amount.as_ref())
    {
        let precision = unlock_fee_precision.ok_or_else(|| {
            AppError::Internal("unlock fee asset precision was not locked".to_owned())
        })?;
        let quantized = quantize_unlock_fee_amount(raw_fee_amount, precision)?;
        fee_paid_status = if quantized > 0 {
            "pending"
        } else {
            "not_required"
        };
        unlock_fee_amount = Some(quantized);
    }

    let unlock_idempotency_key =
        new_coin_unlock_idempotency_key("new_coin_distribution", source_id)?;
    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(lock_position_id)
    .bind(quantity)
    .bind(&project.issue_price)
    .bind(project.unlock_fee_enabled)
    .bind(&project.unlock_fee_rate)
    .bind(&project.unlock_fee_basis)
    .bind(project.unlock_fee_asset)
    .bind(&unlock_fee_amount)
    .bind(fee_paid_status)
    .bind(unlock_idempotency_key)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 新币资金事务以行锁读取 active 资产精度，调用方按资产主键顺序调用以统一锁序。
pub(crate) async fn load_active_new_coin_asset_precision_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<i32> {
    let Some((precision_scale, status)): Option<(i32, String)> = sqlx::query_as(
        "SELECT precision_scale, status FROM assets WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Err(AppError::NotFound);
    };
    if status != "active" {
        return Err(AppError::Validation(
            "new coin asset must be active".to_owned(),
        ));
    }
    Ok(precision_scale)
}

/// 在已锁定项目的派发事务内预留供给，剩余数量不足时拒绝且不动钱包。
pub(crate) async fn reserve_admin_new_coin_supply_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    quantity: &BigDecimal,
) -> AppResult<()> {
    let updated = sqlx::query(
        r#"UPDATE new_coin_projects
           SET reserved_supply = reserved_supply + ?, remaining_supply = remaining_supply - ?
           WHERE id = ? AND remaining_supply >= ?"#,
    )
    .bind(quantity)
    .bind(quantity)
    .bind(project_id)
    .bind(quantity)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Validation(
            "new coin project remaining supply is insufficient".to_owned(),
        ));
    }
    Ok(())
}

/// 派发钱包与锁仓落库后把预留供给转入已分配，失败由外层事务回滚全部资金腿。
pub(crate) async fn finalize_admin_new_coin_supply_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    quantity: &BigDecimal,
) -> AppResult<()> {
    let updated = sqlx::query(
        r#"UPDATE new_coin_projects
           SET reserved_supply = reserved_supply - ?, allocated_supply = allocated_supply + ?
           WHERE id = ? AND reserved_supply >= ?"#,
    )
    .bind(quantity)
    .bind(quantity)
    .bind(project_id)
    .bind(quantity)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Internal(
            "new coin distribution supply reservation could not be finalized".to_owned(),
        ));
    }
    Ok(())
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
                  projects.total_supply, projects.issue_price, projects.quote_asset_id,
                  projects.reserved_supply, projects.allocated_supply, projects.remaining_supply,
                  projects.listed_at,
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
