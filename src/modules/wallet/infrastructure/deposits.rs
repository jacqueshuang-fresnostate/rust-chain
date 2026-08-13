//! 充币资产、地址池与链上充值事件持久化。
//!
//! 资金不变量：地址分配在事务中唯一；链事件以 network/tx_hash/event_index 幂等，确认入账或重组冲正必须与钱包及流水原子提交。

use super::shared::{
    fetch_admin_page, insert_wallet_ledger_in_tx, lock_wallet_balance, update_wallet_balance,
};
use crate::{
    error::{AppError, AppResult},
    modules::wallet::{
        WithdrawFeeTier, amount_fits_asset_precision,
        presentation::{
            DepositAddressResponse, DepositAssetResponse, DepositNetworkResponse,
            ObserveDepositRequest, WalletDepositEventResponse,
        },
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

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
#[derive(Debug, sqlx::FromRow)]
struct DepositTargetRow {
    user_id: u64,
    asset_id: u64,
    precision_scale: i32,
    min_deposit_amount: BigDecimal,
    required_confirmations: u32,
}
/// 列出状态启用且开放充值的资产及其精度、最小额和费率配置，按资产代码升序返回。
/// 只筛资产自身的启用与充值开关，不检查是否存在可用网络或地址库存，前端仍可能选到无地址可分配的资产。
/// 响应同时带出提现开关与提现费用，是资产维度的完整配置快照，本查询不触碰任何余额或地址池。
pub(crate) async fn list_deposit_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    let rows = sqlx::query_as::<_, DepositAssetRow>(&deposit_assets_sql(true))
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(deposit_asset_response).collect())
}

/// 列出状态启用且开放提现的资产及固定手续费、阶梯费率与精度配置，按资产代码升序返回。
/// 与充值清单共用同一套字段，仅把启用开关换成提现开关，因此同一资产可能只出现在其中一侧清单。
/// 阶梯费率此处按存量原样返回、未做重叠校验，真正下单时仍以服务端规范化后的规则重新计费。
pub(crate) async fn list_withdraw_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    let rows = sqlx::query_as::<_, DepositAssetRow>(&deposit_assets_sql(false))
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(deposit_asset_response).collect())
}

/// 按可选资产过滤启用的充值网络，并保留地址组共享配置。
/// 资产过滤同时接受两类网络：未配置资产白名单的通用网络，以及白名单中显式包含该资产的网络。
/// 结果按配置排序值再按主键升序，保证同一入参下网络顺序稳定，前端可直接用于默认选中项。
/// 查询只读网络元数据，不分配地址，也不预留或修改地址池库存。
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

/// 确认资产存在、启用且允许充值；不满足时在分配地址前终止请求。
/// 三种结果被明确区分：允许充值放行，资产存在但关闭充值返回校验错误，资产缺失或已停用返回未找到。
/// 关闭充值与资产不存在使用不同错误类型，便于前端区分“暂不支持”与“币种无效”两种提示。
/// 校验不创建钱包或地址记录，失败时地址池库存保持不变。
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

/// 加载启用网络并校验资产在其允许列表中，返回地址组分配键。
/// 网络必须处于启用状态，且要么没有资产白名单，要么白名单显式包含该资产，两者皆不满足即视为不支持。
/// 命中失败统一返回带资产与网络名的校验错误，而不是未找到，因为这是组合不被支持而非资源缺失。
/// 返回的地址组代码是后续锁定库存的唯一依据；地址分配必须使用配置的组代码，不得在应用层硬编码跨网络回退。
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

/// 按用户、资产和地址组读取既有分配，响应仍展示本次请求网络。
/// 同一地址组内的地址被视为跨网络通用，因此排序优先返回网络完全相同的行，其次才回退到同组其他网络的地址。
/// 只匹配状态为已分配的行，回收或停用的历史地址不会被复用；无既有分配时返回空值交由调用方走新分配流程。
/// 已分配地址直接复用且不轮换，查询不会锁定或占用新的地址池行。
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

/// 在调用方事务内锁定地址组的一条可用库存，防止并发重复分配。
/// 候选范围限定为同一地址组内状态可用、且资产白名单包含该资产或未限定资产的行。
/// 择优顺序为：网络完全匹配优先，其次白名单显式包含该资产的行优于仅按单资产列标注的行，最后按地址编号升序。
/// 排他行锁在调用方事务提交前一直持有，并发请求会在此串行等待，从而杜绝同一地址被分配给两个用户。
/// 库存为空时返回未找到而不是排队等待；后续步骤失败时事务回滚，被锁定的地址自动回到可用状态。
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

/// 在地址分配事务中读取用户邮箱，用于把联系方式冗余写进地址池行便于运营核对。
/// 用户行缺失返回未找到并终止整次分配；用户存在但邮箱为空则正常返回空值，不阻断地址绑定。
/// 该读取不加行锁，只是在同一事务中顺带取值，因此不改变已持有的地址行锁顺序。
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

/// 把已锁定地址绑定给用户和资产，并将库存状态改为已分配。
/// 同时写入用户编号、冗余邮箱、实际分配的资产代码和数据库当前时间作为分配时刻。
/// 更新按地址主键定位，安全性依赖调用方此前已持有该行的排他锁，本函数自身不再重复校验状态。
/// 调用方拥有事务；条件更新未命中视为并发冲突，分配与回读必须一起回滚。
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

/// 在当前事务中回读刚分配的充值地址及备注信息，作为返回给用户的权威结果。
/// 回读取的是绑定更新之后的同事务视图，因此资产代码与分配时刻一定是本次写入的值。
/// 备注字段用于需要 memo 的链路，为空表示该地址无需附加标识，调用方不得自行编造。
/// 回读失败会使调用方回滚绑定更新，避免地址已占用却无法返回给用户。
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

#[derive(Debug)]
pub struct NewWalletChainEventDeadLetter<'a> {
    pub gateway_id: u64,
    pub network: &'a str,
    pub event_kind: &'a str,
    pub dedup_key: String,
    pub request_id: Option<String>,
    pub tx_hash: Option<String>,
    pub event_index: Option<u32>,
    pub payload_json: String,
    pub failure_reason: String,
}

/// 记录无法处理的链事件，保留原始载荷和最后一次失败原因供人工追查。
/// 以去重键为唯一身份做插入或更新，同一事件反复失败只会覆盖载荷与失败原因，不会堆积重复死信行。
/// 覆盖语义意味着历史失败原因会被最新一次替换，需要完整失败序列的场景应另行采集日志。
/// 写入走连接池独立执行、不参与调用方事务，因此死信记录不会随业务回滚而消失。
/// 死信写入不代表资金成功入账，也不得替代原链事件的幂等身份判断。
pub async fn insert_wallet_chain_event_dead_letter(
    pool: &Pool<MySql>,
    record: &NewWalletChainEventDeadLetter<'_>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_chain_event_dead_letters
              (gateway_id, network, event_kind, dedup_key, request_id, tx_hash, event_index,
               payload_json, failure_reason)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
             payload_json = VALUES(payload_json),
             failure_reason = VALUES(failure_reason)"#,
    )
    .bind(record.gateway_id)
    .bind(record.network)
    .bind(record.event_kind)
    .bind(&record.dedup_key)
    .bind(&record.request_id)
    .bind(&record.tx_hash)
    .bind(record.event_index)
    .bind(&record.payload_json)
    .bind(&record.failure_reason)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub struct WalletChainEventDeadLetterRecord {
    pub id: u64,
    pub gateway_id: u64,
    pub network: String,
    pub event_kind: String,
    pub request_id: Option<String>,
    pub tx_hash: Option<String>,
    pub event_index: Option<u32>,
    pub payload_json: SqlxJson<serde_json::Value>,
    pub failure_reason: String,
    pub created_at: DateTime<Utc>,
}

/// 按可选网络筛选读取链事件死信，结果以主键倒序返回最近失败的记录。
/// 返回条数会被钳制在一到五百之间，防止调用方传零或超大值把整表拉进内存。
/// 返回载荷是原始网关字段的结构化副本，可用于人工重放，但字段可信度不高于当初的网关响应。
/// 查询只读、不修改任何处理标记，也不能据返回结果直接补写钱包余额或充值流水。
pub async fn list_wallet_chain_event_dead_letters(
    pool: &Pool<MySql>,
    network: Option<&str>,
    limit: u32,
) -> AppResult<Vec<WalletChainEventDeadLetterRecord>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, gateway_id, network, event_kind, request_id, tx_hash, event_index,
                  payload_json, failure_reason, created_at
           FROM wallet_chain_event_dead_letters"#,
    );
    if let Some(network) = network {
        builder.push(" WHERE network = ");
        builder.push_bind(network);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(i64::from(limit.clamp(1, 500)));
    builder
        .build_query_as::<WalletChainEventDeadLetterRecord>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 以 network、tx_hash、event_index 作为链事件唯一身份，记录确认数并在达到阈值时入账。
/// 事务先锁已分配地址/配置目标，再插入或更新事件并锁事件；事件达到确认阈值时才锁钱包。
/// 地址、资产、金额、memo 与既有事件不一致会冲突；金额必须符合资产精度和最小充值额。
/// 首次确认入账只增加 available，frozen/locked 不变，并写一条引用 deposit event 的正向 available 流水；事件、余额和流水同事务提交。
/// 入账后的 available 统一按 18 位定点写回，账本的 balance_after 与三桶 after 取同一账后快照，不做二次舍入。
/// 锁顺序固定为地址与资产配置、链事件行、钱包账户行三级递进，与提现路径的先单据后钱包保持同向，避免交叉死锁。
/// 重放只单调更新确认数，credited 状态不再增加余额；任一步失败回滚本次事件进度及资金写入。
pub(crate) async fn observe_deposit_event(
    pool: &Pool<MySql>,
    request: &ObserveDepositRequest,
) -> AppResult<WalletDepositEventResponse> {
    let mut tx = pool.begin().await?;
    let target = sqlx::query_as::<_, DepositTargetRow>(
        r#"SELECT pool.assigned_user_id AS user_id, assets.id AS asset_id,
                  assets.precision_scale, assets.min_deposit_amount,
                  configs.required_confirmations
           FROM deposit_address_pool pool
           INNER JOIN assets ON assets.symbol = pool.assigned_asset_symbol
           INNER JOIN deposit_network_configs configs
                   ON configs.network = ? AND configs.status = 'active'
                  AND configs.address_group_code = pool.address_group_code
                  AND (
                      configs.asset_symbols_json IS NULL
                      OR JSON_CONTAINS(
                          configs.asset_symbols_json,
                          JSON_QUOTE(pool.assigned_asset_symbol)
                      )
                  )
           WHERE pool.address = ? AND pool.status = 'assigned'
             AND pool.assigned_asset_symbol = ? AND assets.status = 'active'
             AND assets.deposit_enabled = TRUE
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(&request.network)
    .bind(&request.address)
    .bind(&request.asset_symbol)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if !amount_fits_asset_precision(&request.amount, target.precision_scale) {
        return Err(AppError::Validation(format!(
            "deposit amount supports at most {} decimal places",
            target.precision_scale
        )));
    }
    if request.amount < target.min_deposit_amount {
        return Err(AppError::Validation(format!(
            "deposit amount is below minimum {}",
            target.min_deposit_amount
        )));
    }
    sqlx::query(
        r#"INSERT INTO wallet_deposit_events
              (user_id, asset_id, asset_symbol, network, address, memo, tx_hash, event_index,
               amount, block_height, confirmations, required_confirmations, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'observed')
           ON DUPLICATE KEY UPDATE
             confirmations = GREATEST(confirmations, VALUES(confirmations)),
             block_height = COALESCE(VALUES(block_height), block_height)"#,
    )
    .bind(target.user_id)
    .bind(target.asset_id)
    .bind(&request.asset_symbol)
    .bind(&request.network)
    .bind(&request.address)
    .bind(&request.memo)
    .bind(&request.tx_hash)
    .bind(request.event_index)
    .bind(&request.amount)
    .bind(request.block_height)
    .bind(request.confirmations)
    .bind(target.required_confirmations)
    .execute(&mut *tx)
    .await?;
    let event = load_deposit_event_by_external_key_for_update(
        &mut tx,
        &request.network,
        &request.tx_hash,
        request.event_index,
    )
    .await?;
    if event.user_id != target.user_id
        || event.asset_id != target.asset_id
        || event.address != request.address
        || event.memo != request.memo
        || event.amount != request.amount
    {
        return Err(AppError::Conflict(
            "deposit event identity was already used with different parameters".to_owned(),
        ));
    }
    if event.status == "observed" && event.confirmations >= event.required_confirmations {
        credit_deposit_event_in_tx(&mut tx, &event).await?;
    }
    let event = load_deposit_event_by_id_in_tx(&mut tx, event.id).await?;
    tx.commit().await?;
    Ok(event)
}

/// 锁定已入账事件后处理链重组冲正；已 reversed 时直接返回，其他非 credited 状态冲突。
/// available 足额时扣回原充值额，frozen/locked 不变，并写一条 `deposit_reorg_reverse` 负向 available 流水后标记 reversed。
/// 冲正金额恒等于原事件金额，扣减后的 available 按 18 位定点写回，流水以事件编号为业务引用，与入账条目共用同一身份。
/// 锁顺序与入账一致：先按事件主键取排他锁，再锁钱包账户行，因此正向入账与反向冲正不会互相形成环路等待。
/// available 不足时不扣任何余额、不写冲正流水，而是提交 manual_review 与失败原因，保留人工处置事实。
/// 状态更新均带原状态条件，配合事件行锁保证并发重放中最多一次生效；事件、余额和流水由该函数自有事务提交。
/// 重复调用不会二次扣款，任一 SQL 失败整体回滚，不会留下余额已扣但状态未改的中间态。
pub(crate) async fn reverse_deposit_event(
    pool: &Pool<MySql>,
    deposit_id: u64,
    reason: &str,
) -> AppResult<WalletDepositEventResponse> {
    let mut tx = pool.begin().await?;
    let event = load_deposit_event_by_id_for_update(&mut tx, deposit_id).await?;
    if event.status == "reversed" {
        tx.commit().await?;
        return Ok(event);
    }
    if event.status != "credited" {
        return Err(AppError::Conflict(format!(
            "deposit cannot be reversed from status {}",
            event.status
        )));
    }
    let wallet = lock_wallet_balance(&mut tx, event.user_id, event.asset_id).await?;
    if wallet.available < event.amount {
        sqlx::query(
            r#"UPDATE wallet_deposit_events
               SET status = 'manual_review', failure_reason = ?
               WHERE id = ? AND status = 'credited'"#,
        )
        .bind(reason)
        .bind(event.id)
        .execute(&mut *tx)
        .await?;
        let event = load_deposit_event_by_id_in_tx(&mut tx, event.id).await?;
        tx.commit().await?;
        return Ok(event);
    }
    let available_after = (wallet.available.clone() - event.amount.clone()).with_scale(18);
    update_wallet_balance(
        &mut tx,
        event.user_id,
        event.asset_id,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
    )
    .await?;
    insert_wallet_ledger_in_tx(
        &mut tx,
        event.user_id,
        event.asset_id,
        "deposit_reorg_reverse",
        &(-event.amount.clone()),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        "wallet_deposit_event",
        &event.id.to_string(),
    )
    .await?;
    sqlx::query(
        r#"UPDATE wallet_deposit_events
           SET status = 'reversed', failure_reason = ?, reversed_at = CURRENT_TIMESTAMP(6)
           WHERE id = ? AND status = 'credited'"#,
    )
    .bind(reason)
    .bind(event.id)
    .execute(&mut *tx)
    .await?;
    let event = load_deposit_event_by_id_in_tx(&mut tx, event.id).await?;
    tx.commit().await?;
    Ok(event)
}

/// 后台充值事件列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
/// 按后台筛选条件查询充值链事件并使用同一谓词计算总数。
/// 该只读入口不触发入账、冲正或游标推进，分页结果应与总数保持一致。
pub(crate) async fn list_deposit_events(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<WalletDepositEventResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(wallet_deposit_select_sql());
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM wallet_deposit_events events");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = user_id {
            builder.push(" AND events.user_id = ");
            builder.push_bind(user_id);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY events.id DESC",
        limit.clamp(1, 200),
        offset,
    )
    .await
}

/// 生成资产配置清单 SQL，按入参在充值开关和提现开关之间二选一作为过滤列。
/// 开关列名由代码内布尔量决定而非外部输入拼接，因此不存在注入风险；两种模式的选择列与排序完全一致。
/// 阶梯费率列在为空时兜底成空 JSON 数组，避免反序列化阶段因空值报错而使整张清单不可用。
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

/// 返回启用充值网络查询的固定前缀，条件停在状态过滤处，供调用方继续追加资产白名单与排序。
/// 资产白名单列为空时兜底成空 JSON 数组，使无白名单的通用网络也能安全反序列化。
fn deposit_networks_sql() -> &'static str {
    r#"SELECT network,
              display_name,
              address_group_code,
              address_group_name,
              COALESCE(asset_symbols_json, JSON_ARRAY()) AS asset_symbols
       FROM deposit_network_configs
       WHERE status = 'active'"#
}

/// 返回充值链事件的统一选择列与来源表，供列表分页、外部键锁定和主键回读三条路径复用同一投影。
/// 共用投影保证幂等判定所依赖的地址、金额、备注、确认数和状态字段在各入口取值口径完全一致。
fn wallet_deposit_select_sql() -> &'static str {
    r#"SELECT events.id, events.user_id, events.asset_id, events.asset_symbol,
              events.network, events.address, events.memo, events.tx_hash, events.event_index,
              events.amount, events.block_height, events.confirmations,
              events.required_confirmations, events.status, events.failure_reason,
              events.credited_at, events.reversed_at, events.created_at
       FROM wallet_deposit_events events"#
}

/// 以链上外部身份三元组定位事件行并加排他锁，是幂等入账的串行化关键点。
/// 三元组为网络、交易哈希与事件序号，与数据库唯一约束一致，因此同一链上事件的并发处理只能有一个进入临界区。
/// 该锁必须在锁钱包之前获取，调用方据此保证充值路径的锁顺序始终是先事件后账户。
/// 行不存在返回未找到，通常意味着上游幂等插入未生效，属于异常而非可忽略情形。
async fn load_deposit_event_by_external_key_for_update(
    tx: &mut Transaction<'_, MySql>,
    network: &str,
    tx_hash: &str,
    event_index: u32,
) -> AppResult<WalletDepositEventResponse> {
    sqlx::query_as::<_, WalletDepositEventResponse>(&format!(
        "{} WHERE events.network = ? AND events.tx_hash = ? AND events.event_index = ? LIMIT 1 FOR UPDATE",
        wallet_deposit_select_sql()
    ))
    .bind(network)
    .bind(tx_hash)
    .bind(event_index)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按事件主键加排他锁读取充值事件，供后台冲正等以内部编号发起的操作串行化。
/// 与按外部键加锁的入口作用相同但定位方式不同，两者都必须先于钱包账户锁获取以维持同向锁序。
async fn load_deposit_event_by_id_for_update(
    tx: &mut Transaction<'_, MySql>,
    event_id: u64,
) -> AppResult<WalletDepositEventResponse> {
    sqlx::query_as::<_, WalletDepositEventResponse>(&format!(
        "{} WHERE events.id = ? LIMIT 1 FOR UPDATE",
        wallet_deposit_select_sql()
    ))
    .bind(event_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在事务内按主键回读充值事件的最新状态，用于把入账或冲正后的结果返回给调用方。
/// 该读取不加锁，因为调用方此前已持有同一行的排他锁，重复加锁只会增加等待而无额外保护。
async fn load_deposit_event_by_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    event_id: u64,
) -> AppResult<WalletDepositEventResponse> {
    sqlx::query_as::<_, WalletDepositEventResponse>(&format!(
        "{} WHERE events.id = ? LIMIT 1",
        wallet_deposit_select_sql()
    ))
    .bind(event_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在调用方事务中完成一笔充值的实际入账：锁钱包、增加 available、写正向流水并把事件推进为已入账。
/// 资金只进 available，frozen 与 locked 原值回写；入账后的可用余额按 18 位定点计算，避免不同链精度污染账本。
/// 流水变更类型固定为 deposit_confirm，业务引用指向充值事件编号，因此重复入账可由引用维度直接甄别。
/// 状态更新带上原状态为已观测的条件，受影响行数不为一即判定并发抢先，返回冲突让整个事务回滚。
/// 该函数不做确认数是否达标的判断，也不校验金额精度，前置条件全部由调用方在同一事务内完成。
async fn credit_deposit_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    event: &WalletDepositEventResponse,
) -> AppResult<()> {
    let wallet = lock_wallet_balance(tx, event.user_id, event.asset_id).await?;
    let available_after = (wallet.available.clone() + event.amount.clone()).with_scale(18);
    update_wallet_balance(
        tx,
        event.user_id,
        event.asset_id,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
    )
    .await?;
    insert_wallet_ledger_in_tx(
        tx,
        event.user_id,
        event.asset_id,
        "deposit_confirm",
        &event.amount,
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        "wallet_deposit_event",
        &event.id.to_string(),
    )
    .await?;
    let update = sqlx::query(
        r#"UPDATE wallet_deposit_events
           SET status = 'credited', credited_at = CURRENT_TIMESTAMP(6)
           WHERE id = ? AND status = 'observed'"#,
    )
    .bind(event.id)
    .execute(&mut **tx)
    .await?;
    if update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "deposit event status changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

/// 把资产配置行搬运为充提资产响应项，同时带出充值与提现两侧开关供前端判断可用动作。
/// 精度、最小充值额与两类费用按定点原值输出，阶梯费率从 JSON 列展开为结构体数组，本转换不校验阶梯是否重叠。
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

/// 把充值网络配置行搬运为网络响应项，附带地址组代码与组名以便前端说明地址复用范围。
/// 资产白名单从 JSON 列展开为字符串数组，空数组表示该网络对所有资产通用而非不支持任何资产。
fn deposit_network_response(row: DepositNetworkRow) -> DepositNetworkResponse {
    DepositNetworkResponse {
        network: row.network,
        display_name: row.display_name,
        address_group_code: row.address_group_code,
        address_group_name: row.address_group_name,
        asset_symbols: row.asset_symbols.0,
    }
}

/// 把地址池行搬运为充值地址响应项，输出地址、备注、所属网络与分配时刻。
/// 这里的网络取地址池自身取值，调用方在地址组跨网络复用时会用请求网络覆盖它，以贴合用户本次选择。
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
