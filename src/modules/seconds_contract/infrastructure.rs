//! seconds_contract bounded context infrastructure layer.
//!
//! 基础设施层：秒合约上下文所有 MySQL 与 Redis 访问的唯一出口，应用层不直接书写 SQL。
//! 本文件按数据归属分为四组：产品与周期配置的增删改查、订单的下单与结算写入、钱包余额行锁与资金流水、
//! 以及从行情缓存读取开仓价。函数分成两类调用形态，接收 `&Pool<MySql>` 的是只读读模型，
//! 不持锁不入事务，仅用于列表和详情展示；接收 `&mut Transaction` 的必须由调用方开启事务并负责提交回滚，
//! 本层内部任何函数都不会自行 commit。
//! 涉及资金的读取一律使用 `FOR UPDATE` 行锁，锁的获取顺序由应用层统一编排以避免死锁；
//! 本层不做业务规则判定，投注区间、胜负结果、赔付金额的计算都在 service 层完成。
//! 开仓价来自 Redis 行情缓存而非客户端上送，杜绝用户自选有利价格开仓。

use super::{
    presentation::{
        CachedTickerPayload, SecondsContractOrderResponse, SecondsContractProductCycleResponse,
        SecondsContractProductResponse,
    },
    repository::{
        SecondsContractAdminOrderFilter, SecondsContractOrderInsert, SecondsContractProductRow,
        SecondsContractProductRuleRow, SecondsContractProductWrite,
        SecondsContractSettlementPriceRow, SecondsContractWalletLedgerWrite,
        SecondsContractWalletRow,
    },
    service::{NormalizedSecondsContractProductCycle, optional_string},
};
use crate::{
    error::{AppError, AppResult},
    modules::market::market_ticker_redis_key,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

/// 分页排序必须带唯一列 id，否则同一排序值的行会在页间重复或丢失。
const SECONDS_CONTRACT_PRODUCT_ORDER_BY: &str = " ORDER BY products.id DESC";

/// 识别 MySQL 唯一键冲突，供秒合约开仓幂等恢复分支使用。
///
/// 同时兼容驱动错误码与旧版本消息文本；其他数据库故障保持原错误向上传递。
pub(crate) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    let Some(database_error) = error.as_database_error() else {
        return false;
    };
    matches!(database_error.code().as_deref(), Some("1062"))
        || database_error.message().contains("Duplicate entry")
}

/// 执行后台分页查询的公共尾段，把排序、LIMIT 和 OFFSET 追加到行查询后再取一次总数。
/// 行查询与 COUNT 查询必须由调用方用同一组过滤谓词构建，返回总数才能与当前筛选一致，
/// 否则前端会出现总数与实际可翻页数据对不上的情况。
/// `order_by` 必须包含唯一列，仅按时间等可重复列排序会让相邻页出现重复或漏行。
/// 两次查询分别独立执行且不在事务内，因此高并发写入时总数与行集可能存在短暂不一致，这是列表接口的可接受偏差。
async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

/// 读取秒合约产品目录并附加各自的周期集合，用于面向用户的产品列表。
/// `status` 为 `Some("active")` 时不仅筛选产品自身状态，还会连带要求交易对、质押资产以及交易对的
/// 基础资产和计价资产都处于启用状态，避免任一环节下架后产品仍可下单。
/// 结果按产品主键倒序并受 `limit` 限制，不返回总数；周期通过第二次批量查询附加，
/// 产品无周期记录时会用主记录字段兜底出一条默认周期。
/// 全程走连接池只读查询，不持锁不入事务，返回的价格与配置只可用于展示，下单时必须在写事务中重新锁定核对。
pub(crate) async fn list_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<SecondsContractProductResponse>> {
    let mut builder = seconds_contract_product_query();
    push_seconds_contract_product_filters(&mut builder, status);
    builder.push(SECONDS_CONTRACT_PRODUCT_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(limit as i64);

    let product_rows = builder
        .build_query_as::<SecondsContractProductRow>()
        .fetch_all(pool)
        .await?;
    attach_product_cycles_from_pool(pool, product_rows).await
}

/// 为后台产品管理页分页查询秒合约产品并返回符合筛选条件的总数。
/// 行查询与 COUNT 查询由同一段 `push_seconds_contract_product_filters` 追加谓词，总数才会跟随当前筛选；
/// 两者的 JOIN 结构也保持一致，否则筛选启用状态时两边的连带条件会不同步。
/// 与面向用户的目录相比，这里不强制只看启用产品，`status` 为 `None` 时会返回含已禁用产品的全量列表。
/// 周期集合在分页结果确定后统一批量加载，加载失败则整体返回错误而不返回缺周期的半完整产品。
pub(crate) async fn list_admin_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<SecondsContractProductResponse>, i64)> {
    let mut rows = seconds_contract_product_query();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           INNER JOIN assets pair_base_assets ON pair_base_assets.id = pairs.base_asset
           INNER JOIN assets pair_quote_assets ON pair_quote_assets.id = pairs.quote_asset"#,
    );
    for builder in [&mut rows, &mut total] {
        push_seconds_contract_product_filters(builder, status);
    }

    let (product_rows, total) = fetch_admin_page::<SecondsContractProductRow>(
        pool,
        rows,
        total,
        SECONDS_CONTRACT_PRODUCT_ORDER_BY,
        limit,
        offset,
    )
    .await?;
    Ok((
        attach_product_cycles_from_pool(pool, product_rows).await?,
        total,
    ))
}

/// 构造秒合约产品列表的公共 SELECT 骨架，供用户目录与后台列表共用同一套字段与连接结构。
/// 除产品自身字段外，额外连接交易对取展示用交易对符号，连接质押资产取资产符号，
/// 并预先连接交易对的基础资产与计价资产，使调用方可以按这两张别名表追加启用状态过滤。
/// 返回的 builder 尚未包含 WHERE、ORDER BY 与分页子句，调用方必须自行补齐后再执行。
fn seconds_contract_product_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           INNER JOIN assets pair_base_assets ON pair_base_assets.id = pairs.base_asset
           INNER JOIN assets pair_quote_assets ON pair_quote_assets.id = pairs.quote_asset"#,
    )
}

/// 向产品查询追加统一的过滤谓词，保证行查询与 COUNT 查询筛选口径完全一致。
/// 先固定写入恒真的 `WHERE 1 = 1`，使后续条件可以无差别地以 `AND` 拼接，避免判断是否首个条件。
/// `status` 为 `None` 时不加任何过滤；给出状态时按产品状态绑定参数化查询。
/// 仅当筛选 `active` 时才额外要求交易对、质押资产以及交易对基础/计价资产同时启用，
/// 因为查询已下架产品的后台场景不应被上游资产状态连带过滤掉。
fn push_seconds_contract_product_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    status: Option<&str>,
) {
    builder.push(" WHERE 1 = 1");
    if let Some(status) = status {
        builder.push(" AND products.status = ");
        builder.push_bind(status.to_owned());
        if status == "active" {
            builder.push(
                " AND pairs.status = 'active' AND assets.status = 'active' AND pair_base_assets.status = 'active' AND pair_quote_assets.status = 'active'",
            );
        }
    }
}

/// 从连接池读取秒合约产品、交易对/结算资产展示字段及全部可选周期，产品缺失返回 NotFound。
/// 不按状态过滤，已下架产品同样可查，因此该入口服务于后台详情而非用户下单前的可用性判断。
/// 产品与周期分两次查询且都不持有事务或行锁，两次之间理论上可被并发改配置，
/// 因此返回值只用于展示，下单仍须在写事务中重新锁定并验证启用状态的产品快照。
pub(crate) async fn load_product_by_id_from_pool(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    let product = sqlx::query_as::<_, SecondsContractProductRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let cycles = load_product_cycles_from_pool(pool, product_id).await?;
    Ok(product_response_from_row(product, cycles))
}

/// 在管理事务内确认目标交易对存在，把外键冲突提前转成明确的业务错误。
/// 查询只取主键且不加锁，因此不会阻塞行情侧对交易对的并发更新，也不校验交易对是否处于启用状态。
/// 记录缺失时返回 `AppError::NotFound`，调用方须据此回滚整个产品与审计事务，不留下半截配置。
pub(crate) async fn ensure_pair_exists(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM trading_pairs WHERE id = ? LIMIT 1")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// 在管理事务内确认作为质押币种的资产存在，避免产品指向不存在的资产导致下单时无法定位钱包。
/// 与交易对检查同样只取主键、不加行锁、不判断资产启用状态，缺失时返回 `AppError::NotFound`。
/// 校验失败时产品与订单都不会被创建，调用方负责回滚同事务内的其他写入。
pub(crate) async fn ensure_asset_exists(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// 在删除产品前统计其历史订单数量，只要存在任意一笔就拒绝物理删除。
/// 统计范围覆盖全部状态的订单，包括已结算和已过期的，因为删除产品会破坏订单外键并让历史成交无从追溯。
/// 命中时返回 `AppError::Validation` 提示应改用禁用而非删除；计数在调用方事务内执行，
/// 但不对订单表加锁，因此与并发下单之间的最终一致性由订单表的外键约束兜底。
pub(crate) async fn ensure_product_has_no_orders(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<()> {
    let order_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE product_id = ?",
    )
    .bind(product_id)
    .fetch_one(&mut **tx)
    .await?;
    if order_count > 0 {
        return Err(AppError::Validation(
            "seconds contract product with orders cannot be deleted".to_owned(),
        ));
    }
    Ok(())
}

/// 在管理事务内插入秒合约产品主记录并返回自增主键，供调用方继续写周期与审计。
/// 主记录上的 `duration_seconds`、`payout_rate`、`min_stake`、`max_stake` 是旧版单周期字段，
/// 新版多周期配置写在周期子表，这里保留主记录取值用于兼容未按周期下单的旧客户端。
/// 本函数不提交事务，也不移动任何用户资金；插入失败或后续周期与审计写入失败，
/// 由调用方整体回滚，不会留下没有周期配置的孤立产品。
pub(crate) async fn insert_product(
    tx: &mut Transaction<'_, MySql>,
    write: &SecondsContractProductWrite,
) -> AppResult<u64> {
    let product_id = sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, logo_url, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(write.pair_id)
    .bind(write.stake_asset)
    .bind(&write.logo_url)
    .bind(write.duration_seconds)
    .bind(&write.payout_rate)
    .bind(&write.min_stake)
    .bind(&write.max_stake)
    .bind(&write.status)
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    Ok(product_id)
}

/// 在管理事务内整体覆盖秒合约产品主字段，包含交易对、质押资产、图标、默认周期参数和上下架状态。
/// 更新为全字段写入而非部分更新，调用方必须传入完整的目标配置，缺省字段会把原值覆盖掉。
/// 本函数不校验记录是否存在，目标产品已被删除时 UPDATE 影响行数为零且不报错，
/// 存在性由调用方先行加锁读取来保证。
/// 已开仓订单在下单时已固化自己的周期与赔率快照，因此改配置不影响存量订单的结算口径；
/// 本函数不提交事务，周期替换与 before/after 审计由调用方在同一事务内追加。
pub(crate) async fn update_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    write: &SecondsContractProductWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE seconds_contract_products
           SET pair_id = ?, stake_asset = ?, logo_url = ?, duration_seconds = ?, payout_rate = ?,
               min_stake = ?, max_stake = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(write.pair_id)
    .bind(write.stake_asset)
    .bind(&write.logo_url)
    .bind(write.duration_seconds)
    .bind(&write.payout_rate)
    .bind(&write.min_stake)
    .bind(&write.max_stake)
    .bind(&write.status)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在管理事务内更新秒合约产品启停状态，受影响记录不存在时沿用既有错误语义。
/// 状态变更不处理既有订单；调用方负责同事务写管理审计并提交。
pub(crate) async fn update_product_status(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE seconds_contract_products SET status = ? WHERE id = ?")
        .bind(status)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 删除已禁用且无历史订单的秒合约产品；前置约束由应用事务锁定后保证。
/// 本函数只删除产品记录，不结算订单或修改钱包；调用方负责审计和提交。
pub(crate) async fn delete_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM seconds_contract_products WHERE id = ?")
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在调用方已开启的事务内按主键回读产品及其周期集合，通常用于写操作前后取 before/after 审计快照。
/// 查询不带 `FOR UPDATE`，只读取事务当前可见的版本，因此不提供并发保护，
/// 需要排他快照的场景必须改用 `lock_product_by_id`。
/// 产品记录缺失返回 `AppError::NotFound`；周期为空时由行转换逻辑用主记录字段兜底出一条默认周期，
/// 保证返回结构始终至少含一个可下单周期。
pub(crate) async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    let product = sqlx::query_as::<_, SecondsContractProductRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let cycles = load_product_cycles(tx, product_id).await?;
    Ok(product_response_from_row(product, cycles))
}

/// 以 `FOR UPDATE` 排他锁读取待修改的秒合约产品，用于后台改配置、改状态和删除前固定并发快照。
/// 与下单路径上的 `lock_active_product` 不同，这里不过滤状态，已禁用产品同样可以被锁定和修改，
/// 返回的也是含完整周期集合的展示结构而非单周期规则行。
/// 锁在调用方事务提交或回滚时才释放，期间并发的同产品管理操作会阻塞，从而串行化后台改配置。
/// 产品不存在返回 `AppError::NotFound`；锁获取失败或记录缺失时不得继续任何资金与状态写入。
pub(crate) async fn lock_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    let product = sqlx::query_as::<_, SecondsContractProductRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.logo_url,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let cycles = load_product_cycles(tx, product_id).await?;
    Ok(product_response_from_row(product, cycles))
}

/// 逐条写入已通过校验的产品周期配置，并把切片下标直接作为 `sort_order` 落库。
/// 因此周期的展示顺序完全由调用方传入的切片顺序决定，service 层已按时长升序排好，
/// 其中第一条会被读取路径当作该产品的默认周期。
/// 入参要求时长在同一产品内唯一，重复时长在写库阶段可能触发唯一约束错误而非被静默合并。
/// 循环内任一条插入失败即中断并向上返回，本函数不提交事务，已写入的前几条由调用方回滚一并撤销。
pub(crate) async fn insert_product_cycles(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    cycles: &[NormalizedSecondsContractProductCycle],
) -> AppResult<()> {
    for (index, cycle) in cycles.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO seconds_contract_product_cycles
               (product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order)
               VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(product_id)
        .bind(cycle.duration_seconds)
        .bind(&cycle.payout_rate)
        .bind(&cycle.min_stake)
        .bind(&cycle.max_stake)
        .bind(index as u32)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// 以先全删后全插的方式整体替换某产品的周期集合，实现更新请求的覆盖语义。
/// 删除与插入必须处在调用方的同一事务内，否则并发读者会看到周期为空的中间态；
/// 替换会重建周期主键，历史订单只引用产品编号和自身固化的周期快照，因此不受影响。
pub(crate) async fn replace_product_cycles(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    cycles: &[NormalizedSecondsContractProductCycle],
) -> AppResult<()> {
    sqlx::query("DELETE FROM seconds_contract_product_cycles WHERE product_id = ?")
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    insert_product_cycles(tx, product_id, cycles).await
}

/// 读取指定用户的秒合约订单历史，用户编号来自鉴权上下文并强制写入 WHERE 条件，查询绝不跨用户返回记录。
/// 结果同时包含未到期的持仓单与已结算单，按创建时间倒序，并以订单主键倒序作为同一时刻的稳定次级排序。
/// `email` 字段固定选为 NULL，用户侧接口不需要也不应回显账号邮箱。
/// 只读走连接池、不加锁不入事务，返回的 `settlement_price` 与 `result` 对未到期订单为空，
/// 本函数不触发任何结算动作。
pub(crate) async fn list_user_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<Vec<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  NULL AS email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price,
                  orders.settlement_price_tick_id, orders.settlement_price_source,
                  orders.settlement_price_observed_at, orders.settlement_price_generation,
                  orders.settlement_price_version, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.user_id = ?
           ORDER BY orders.created_at DESC, orders.id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 为后台风控与客服查询秒合约订单并返回匹配总数，支持按用户编号、账号邮箱和订单状态组合筛选。
/// 三个筛选项互相独立且都是可选的，同时给出时按 AND 叠加；邮箱按精确相等匹配而非模糊搜索。
/// 行查询与 COUNT 查询在同一个循环里追加谓词，保证总数严格跟随当前筛选。
/// 与用户侧列表不同，这里连接 users 表回显邮箱，便于后台核对订单归属。
/// 排序为创建时间倒序加订单主键倒序，主键作为唯一列参与排序以防同一时间戳的订单在翻页时重复或漏出。
pub(crate) async fn list_admin_orders(
    pool: &Pool<MySql>,
    filter: SecondsContractAdminOrderFilter,
) -> AppResult<(Vec<SecondsContractOrderResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  users.email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price,
                  orders.settlement_price_tick_id, orders.settlement_price_source,
                  orders.settlement_price_observed_at, orders.settlement_price_generation,
                  orders.settlement_price_version, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset"#,
    );
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM seconds_contract_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset"#,
    );
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            builder.push(" AND orders.user_id = ");
            builder.push_bind(user_id);
        }
        if let Some(email) = filter.email.clone() {
            builder.push(" AND users.email = ");
            builder.push_bind(email);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND orders.status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY orders.created_at DESC, orders.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按订单主键从连接池读取单笔秒合约订单详情，含邮箱、交易对与资产符号等展示字段。
/// 查询不带用户维度过滤，调用方必须自行校验归属，否则会造成越权查看他人订单。
/// 记录不存在返回 `AppError::NotFound`；只读不加锁，也不触发任何结算或状态流转。
pub(crate) async fn load_order_by_id_from_pool(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(seconds_contract_order_by_id_sql())
        .bind(order_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在开仓事务内以 `FOR UPDATE` 查找该用户同一幂等键的既有订单，是防止重复下单扣款的核心一步。
/// 按用户编号加幂等键两列定位，与订单表上的唯一约束口径一致，因此幂等键只在单个用户范围内唯一。
/// 命中时返回既有订单，应用层须逐字段核对产品、方向与金额后直接复用，不得再次扣款；
/// 未命中时返回 `None`，此时行锁会退化为对唯一索引间隙的锁定，从而阻塞并发的同键插入。
/// 本函数不做一致性判定也不写任何数据，锁在调用方事务结束时释放。
pub(crate) async fn existing_order_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  NULL AS email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price,
                  orders.settlement_price_tick_id, orders.settlement_price_source,
                  orders.settlement_price_observed_at, orders.settlement_price_generation,
                  orders.settlement_price_version, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.user_id = ? AND orders.idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在开启事务之前先用只读查询探测同一幂等键的既有订单，作为重放请求的快速返回通道。
/// 命中即可直接回放原订单，无需读取当前行情价、无需开启事务、也不会再次扣款，
/// 这一点很关键：重放请求发生在行情缓存过期或价格已变动之后仍应成功返回原单。
/// 与事务内版本相比这里不加 `FOR UPDATE`，因此不排斥并发写入，未命中时仍须进入事务再锁一次确认。
pub(crate) async fn existing_order_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  NULL AS email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price,
                  orders.settlement_price_tick_id, orders.settlement_price_source,
                  orders.settlement_price_observed_at, orders.settlement_price_generation,
                  orders.settlement_price_version, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.user_id = ? AND orders.idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 从 `market:ticker:{symbol}` 的 Redis 缓存读取 `last_price` 作为服务端认定的开仓价。
/// 开仓价只取自服务端缓存，绝不接受客户端上送，避免用户挑选对自己有利的价格开仓。
/// 未配置 Redis 连接时直接返回校验错误而不是回落到数据库或默认价，宁可拒绝下单也不用不可信价格锁定盈亏。
/// 价格必须为正数；`observed_at` 早于当前时间 60 秒即判定为陈旧行情并拒绝，
/// 使开仓价的时间锚定被限制在一分钟内，防止行情推送中断期间按旧价成交。
/// 缓存缺失或 JSON 解析失败同样报错。本函数只负责开仓一侧，不参与到期时刻的结算取价与胜负判定。
pub(crate) async fn cached_entry_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<BigDecimal> {
    let redis = redis.ok_or_else(|| {
        AppError::Validation(
            "fresh cached ticker is required to open seconds contract orders".to_owned(),
        )
    })?;
    let ticker = cached_ticker_price(redis, symbol).await?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(format!(
            "seconds contract entry price must be positive for pair {pair_id}"
        )));
    }
    if ticker.observed_at < Utc::now() - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(format!(
            "seconds contract entry ticker is stale for pair {pair_id}"
        )));
    }
    Ok(ticker.last_price)
}

/// 下单路径上锁定可交易的秒合约产品，并解析出本次下单实际适用的那条周期规则。
/// 产品必须自身启用，且交易对、质押资产、交易对的基础资产与计价资产全部启用，任一环节下架都返回
/// `AppError::NotFound`，对外表现为产品不存在而不泄露下架细节。
/// `duration_seconds` 为 `None` 时回落到产品主记录上的默认时长，解析后的时长为零一律拒绝。
/// 周期规则优先取周期子表中时长匹配的那一行并同样加 `FOR UPDATE`；只有在调用方未指定时长、
/// 且默认时长在子表中查不到对应记录时，才回落到产品主记录上的旧版单周期字段，
/// 这条兼容路径专门服务尚未迁移到多周期配置的存量产品；其余找不到匹配周期的情况一律返回 NotFound。
/// 返回的规则行携带质押资产精度，供后续投注额精度校验与赔付截断使用；赔率与投注区间取自选中的那条周期，
/// 而非产品主记录，因此不同周期可以有各自的赔率和限额。
/// 两处行锁在调用方事务提交或回滚时释放，锁序由应用层统一编排；未成功取到规则前不得进行任何资金写入。
pub(crate) async fn lock_active_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    duration_seconds: Option<u32>,
) -> AppResult<SecondsContractProductRuleRow> {
    let product = sqlx::query_as::<_, SecondsContractProductRuleRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.precision_scale AS stake_asset_precision,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
           INNER JOIN assets pair_base_assets ON pair_base_assets.id = pairs.base_asset
           INNER JOIN assets pair_quote_assets ON pair_quote_assets.id = pairs.quote_asset
           WHERE products.id = ? AND products.status = 'active'
             AND pairs.status = 'active' AND assets.status = 'active'
             AND pair_base_assets.status = 'active' AND pair_quote_assets.status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let requested_duration = duration_seconds.unwrap_or(product.duration_seconds);
    if requested_duration == 0 {
        return Err(AppError::Validation(
            "seconds contract duration_seconds must be positive".to_owned(),
        ));
    }

    let cycle = sqlx::query_as::<_, SecondsContractProductCycleResponse>(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id = ? AND duration_seconds = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .bind(requested_duration)
    .fetch_optional(&mut **tx)
    .await?;
    let (duration_seconds, payout_rate, min_stake, max_stake) = if let Some(cycle) = cycle {
        (
            cycle.duration_seconds,
            cycle.payout_rate,
            cycle.min_stake,
            cycle.max_stake,
        )
    } else if duration_seconds.is_none() && requested_duration == product.duration_seconds {
        (
            product.duration_seconds,
            product.payout_rate.clone(),
            product.min_stake.clone(),
            product.max_stake.clone(),
        )
    } else {
        return Err(AppError::NotFound);
    };

    Ok(SecondsContractProductRuleRow {
        id: product.id,
        pair_id: product.pair_id,
        symbol: product.symbol,
        stake_asset: product.stake_asset,
        stake_asset_precision: product.stake_asset_precision,
        duration_seconds,
        payout_rate,
        min_stake,
        max_stake,
        status: product.status,
    })
}

/// 在事务内读取资产的小数位精度，供投注额合法性校验与赔付金额截断使用。
/// 资产不存在时返回 `AppError::NotFound` 从而中断整笔资金操作，绝不回落到某个默认小数位，
/// 因为按错误精度截断会直接造成派奖金额偏差。查询只读不加锁。
pub(crate) async fn load_asset_precision_scale(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<i32> {
    sqlx::query_scalar::<_, i32>("SELECT precision_scale FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在开仓事务内插入订单行并返回自增主键，状态硬编码为 `opened`，结算价与结果留空待到期填写。
/// 写入的是下单当刻的快照：服务端开仓价、选定周期时长、该周期的赔率以及到期时间，
/// 这些值一旦落库就不再随产品配置变更而改变，保证结算口径与用户下单时看到的一致。
/// 用户与幂等键的唯一约束在这一步占位，且发生在钱包扣款之前，因此并发同键请求最多只有一笔能插入成功。
/// 返回类型刻意保留原始 `sqlx::Error` 而不转成 `AppError`，让调用方能识别唯一键冲突并转入回读原单的
/// 幂等分支；本函数不提交事务，也不扣减余额。
pub(crate) async fn insert_open_order(
    tx: &mut Transaction<'_, MySql>,
    order: &SecondsContractOrderInsert,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            duration_seconds, payout_rate, entry_price, status, idempotency_key, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'opened', ?, ?)"#,
    )
    .bind(order.user_id)
    .bind(order.product_id)
    .bind(order.pair_id)
    .bind(order.stake_asset)
    .bind(&order.direction)
    .bind(&order.stake_amount)
    .bind(order.duration_seconds)
    .bind(&order.payout_rate)
    .bind(&order.entry_price)
    .bind(&order.idempotency_key)
    .bind(order.expires_at)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
}

/// 以 `FOR UPDATE` 锁定用户在质押资产上的钱包行，固定开仓扣款或结算入账期间的余额快照。
/// 同时返回可用、冻结和锁定三部分余额，调用方据此计算变更后的余额并写入资金流水的各项快照字段。
/// 钱包账户不存在时返回校验错误而不是自动开户，避免在资金路径上隐式创建账户。
/// 行锁持续到调用方事务结束，因此同一用户同一资产的并发下单会被串行化；本函数自身不改余额也不写流水。
pub(crate) async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<SecondsContractWalletRow> {
    sqlx::query_as::<_, SecondsContractWalletRow>(
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
    .ok_or_else(|| {
        AppError::Validation("wallet account is required for seconds contract".to_owned())
    })
}

/// 在已持有钱包行锁的事务内把可用余额直接覆盖为计算好的目标值，而不是做增量加减。
/// 覆盖写要求调用方先经 `lock_wallet_row` 取到快照并在锁保护下算出 `available_after`，
/// 否则并发写会丢失更新。冻结与锁定余额不在此处改动，秒合约本金直接从可用余额扣减。
/// 本函数不写流水也不改订单状态，调用方必须在同一事务内一并完成，禁止把余额变更与结算记录分开提交。
pub(crate) async fn update_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    available_after: &BigDecimal,
) -> AppResult<()> {
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在开仓或结算事务内追加一条秒合约资金流水，`ref_type` 固定为 `seconds_contract_order`，
/// `ref_id` 为订单主键，因此对账时可由订单反查全部资金变动。
/// `balance_type` 固定为 `available`，`balance_after` 与 `available_after` 绑定同一个值，
/// 说明秒合约只操作可用余额这一个余额桶；冻结与锁定余额原样记录当时快照。
/// 金额与各项余额快照必须与同一次钱包更新严格对应，调用方要在持有钱包行锁期间计算并写入。
/// 事务失败时流水与余额一起回滚；同一订单的重放路径不得再次调用本函数，否则会产生重复流水。
pub(crate) async fn insert_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    entry: SecondsContractWalletLedgerWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, 'seconds_contract_order', ?)"#,
    )
    .bind(entry.user_id)
    .bind(entry.asset_id)
    .bind(entry.change_type)
    .bind(&entry.amount)
    .bind(&entry.available_after)
    .bind(&entry.available_after)
    .bind(&entry.frozen_after)
    .bind(&entry.locked_after)
    .bind(entry.ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务内按主键回读订单最新快照，用于结算写入后组装响应体和审计 after 镜像。
/// 由于处在同一事务中，能读到本事务尚未提交的结算结果与状态变更。
/// 不加 `FOR UPDATE`，调用方通常已在更早的步骤持有该订单行锁；记录缺失返回 `AppError::NotFound`。
pub(crate) async fn load_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(seconds_contract_order_by_id_sql())
        .bind(order_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 以 `FOR UPDATE` 锁定待结算订单，是结算幂等的第一道保障：定时结算与后台人工结算并发时只有一方能推进。
/// 返回快照含当前状态与既有结果，调用方据此判断是首次结算还是重放，重放时必须比对结果一致后直接复用。
/// 锁定订单行的动作应排在锁定钱包行之前，全局保持一致的加锁顺序以避免与其他资金路径互相死锁。
/// 订单不存在返回 `AppError::NotFound`；未成功持锁前不得写入任何余额、流水或状态。
pub(crate) async fn lock_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  users.email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
                  orders.direction, orders.stake_amount, orders.duration_seconds,
                  orders.payout_rate, orders.entry_price, orders.settlement_price,
                  orders.settlement_price_tick_id, orders.settlement_price_source,
                  orders.settlement_price_observed_at, orders.settlement_price_generation,
                  orders.settlement_price_version, orders.status, orders.result,
                  orders.idempotency_key, orders.expires_at, orders.created_at
           FROM seconds_contract_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 把已由调用方持锁并确认 opened 的订单更新为 settled，写入结果与数据库当前结算时间。
/// 本函数不写结算价格或赔付字段，也不自行附加 opened 条件；调用方事务负责钱包、流水和审计的一致提交。
pub(crate) async fn mark_order_settled(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    result: &str,
    snapshot: &SecondsContractSettlementPriceRow,
) -> AppResult<()> {
    let update = sqlx::query(
        r#"UPDATE seconds_contract_orders
           SET status = 'settled', result = ?, settlement_price = ?,
               settlement_price_tick_id = ?, settlement_price_source = ?,
               settlement_price_observed_at = ?, settlement_price_generation = ?,
               settlement_price_version = ?, settled_at = CURRENT_TIMESTAMP(6)
           WHERE id = ? AND status = 'opened'"#,
    )
    .bind(result)
    .bind(&snapshot.price)
    .bind(snapshot.id)
    .bind(&snapshot.source)
    .bind(snapshot.observed_at.naive_utc())
    .bind(snapshot.generation)
    .bind(&snapshot.source_version)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    if update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "seconds contract order changed during settlement".to_owned(),
        ));
    }
    Ok(())
}

/// 读取当前数据库时间，供人工结算与本地到期边界统一使用；调用方不能用应用进程时钟替代。
pub(crate) async fn database_now(tx: &mut Transaction<'_, MySql>) -> AppResult<DateTime<Utc>> {
    let now = sqlx::query_scalar::<_, chrono::NaiveDateTime>("SELECT CURRENT_TIMESTAMP(6)")
        .fetch_one(&mut **tx)
        .await?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(now, Utc))
}

/// 选择到期事件窗口 `[expires_at, expires_at + 5s)` 内第一条不可变 ticker。
/// 排序先按事件时间，再按固定供应商优先级、源版本和主键，处理早晚不会改变选择结果。
/// 窗口内没有历史行时返回 `None`，调用方必须保持 pending，禁止读取 Redis 最新价兜底。
pub(crate) async fn select_settlement_price_snapshot(
    tx: &mut Transaction<'_, MySql>,
    symbol: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<Option<SecondsContractSettlementPriceRow>> {
    let snapshot = sqlx::query_as::<_, SecondsContractSettlementPriceRow>(
        r#"SELECT id, symbol, price, source, observed_at, generation, source_version
           FROM market_price_ticks
           WHERE symbol = REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND observed_at >= ?
             AND observed_at < DATE_ADD(?, INTERVAL 5 SECOND)
           ORDER BY observed_at ASC,
                    CASE source
                        WHEN 'bitget' THEN 0
                        WHEN 'htx' THEN 1
                        WHEN 'coinbase' THEN 2
                        WHEN 'strategy' THEN 3
                        ELSE 9
                    END ASC,
                    source_version ASC,
                    id ASC
           LIMIT 1"#,
    )
    .bind(symbol)
    .bind(expires_at.naive_utc())
    .bind(expires_at.naive_utc())
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(snapshot) = snapshot.as_ref() {
        super::service::validate_settlement_price_snapshot(snapshot, symbol, expires_at)?;
    }
    Ok(snapshot)
}

#[allow(clippy::too_many_arguments)] // 审计字段与数据库列稳定对应，调用方事务负责原子提交。
/// 在产品配置或人工结算的同一事务内写入后台审计记录，使操作人、动作、前后镜像与原因原子留痕。
/// `target_id` 以字符串形式落库以兼容审计表对不同业务主键类型的统一存储；
/// `before_json` 与 `after_json` 为可选，创建类操作没有前镜像、删除类操作没有后镜像。
/// `reason` 经 `optional_string` 裁剪，纯空白会被存为 NULL 而不是空串。
/// 审计写入失败必须由调用方回滚对应的配置变更或资金变更，不允许业务成功而审计缺失。
pub(crate) async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    let request_context = crate::infra::admin_request_context::current_admin_request_context();
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, request_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(optional_string(reason))
    .bind(
        request_context
            .as_ref()
            .and_then(|context| context.source_ip.as_deref()),
    )
    .bind(
        request_context
            .as_ref()
            .map(|context| context.request_id.as_str()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按行情模块约定的键名从 Redis 取出该交易对的 ticker 快照并反序列化。
/// 键缺失返回校验错误，提示必须先有行情缓存才能开仓；JSON 结构不匹配则返回 `AppError::Internal`，
/// 因为那属于行情写入方与本模块的数据契约破损而非用户输入问题。
/// 本函数只做读取与解码，价格正负和新鲜度判断留给调用方。
async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
) -> AppResult<CachedTickerPayload> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let payload = payload.ok_or_else(|| {
        AppError::Validation("cached ticker is required to open seconds contract orders".to_owned())
    })?;
    serde_json::from_str::<CachedTickerPayload>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))
}

/// 为一批产品行一次性补齐周期集合，用单条 `IN` 查询取回所有周期后在内存中按产品编号分组。
/// 这样把周期加载从每产品一次查询压成整批一次，避免产品列表出现 N+1 查询。
/// 产品集合为空时直接短路返回，不发出无谓的空 `IN` 查询。
/// 周期按产品编号、排序号、时长、主键排序，保证同一产品内的周期顺序稳定且默认周期始终落在首位。
/// 某个产品没有任何周期记录时不会被剔除，行转换会用其主记录字段兜底出一条默认周期。
async fn attach_product_cycles_from_pool(
    pool: &Pool<MySql>,
    product_rows: Vec<SecondsContractProductRow>,
) -> AppResult<Vec<SecondsContractProductResponse>> {
    if product_rows.is_empty() {
        return Ok(Vec::new());
    }
    let product_ids = product_rows
        .iter()
        .map(|product| product.id)
        .collect::<Vec<_>>();
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id IN ("#,
    );
    let mut separated = builder.separated(", ");
    for product_id in &product_ids {
        separated.push_bind(product_id);
    }
    separated.push_unseparated(") ORDER BY product_id, sort_order, duration_seconds, id");
    let cycle_rows = builder
        .build_query_as::<SecondsContractProductCycleResponse>()
        .fetch_all(pool)
        .await?;

    Ok(product_rows
        .into_iter()
        .map(|product| {
            let cycles = cycle_rows
                .iter()
                .filter(|cycle| cycle.product_id == product.id)
                .cloned()
                .collect::<Vec<_>>();
            product_response_from_row(product, cycles)
        })
        .collect())
}

/// 走连接池读取单个产品的全部周期配置，用于产品详情这类不在事务中的只读场景。
/// 排序键依次为排序号、时长和主键，其中排序号来自写入时的切片下标，因此首条即业务上的默认周期；
/// 后两个键用于排序号重复时的稳定兜底，避免同一份数据在多次查询间顺序漂移。
/// 产品无周期记录时返回空切片而非错误，由上层决定如何兜底。
async fn load_product_cycles_from_pool(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<Vec<SecondsContractProductCycleResponse>> {
    sqlx::query_as::<_, SecondsContractProductCycleResponse>(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id = ?
           ORDER BY sort_order, duration_seconds, id"#,
    )
    .bind(product_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 在调用方事务内读取单个产品的周期配置，与连接池版本 SQL 完全相同但执行在事务连接上。
/// 因此能看到本事务中刚刚替换但尚未提交的周期集合，管理路径取 after 审计快照时依赖这一点。
/// 查询不加行锁，排他性由调用方在产品主行上持有的锁提供。
async fn load_product_cycles(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<Vec<SecondsContractProductCycleResponse>> {
    sqlx::query_as::<_, SecondsContractProductCycleResponse>(
        r#"SELECT id, product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order
           FROM seconds_contract_product_cycles
           WHERE product_id = ?
           ORDER BY sort_order, duration_seconds, id"#,
    )
    .bind(product_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 把产品主行与周期行合并成对外响应结构，并确保响应中始终至少含一条可下单周期。
/// 周期集合为空时用产品主记录上的旧版单周期字段合成一条虚拟周期，其主键与排序号填 0 表示并非真实子表记录，
/// 这条兼容路径服务于尚未迁移到多周期配置的存量产品。
/// 响应顶层的时长、赔率与投注上下限取自周期集合的首条即默认周期，只有在集合确实为空时才回落到主记录取值，
/// 使旧客户端读顶层字段与新客户端读周期集合看到的默认档位一致。
/// 本函数为纯内存转换，不查询数据库也不做任何校验。
fn product_response_from_row(
    product: SecondsContractProductRow,
    cycles: Vec<SecondsContractProductCycleResponse>,
) -> SecondsContractProductResponse {
    let cycles = if cycles.is_empty() {
        vec![SecondsContractProductCycleResponse {
            id: 0,
            product_id: product.id,
            duration_seconds: product.duration_seconds,
            payout_rate: product.payout_rate.clone(),
            min_stake: product.min_stake.clone(),
            max_stake: product.max_stake.clone(),
            sort_order: 0,
        }]
    } else {
        cycles
    };
    let default_cycle = cycles.first();
    SecondsContractProductResponse {
        id: product.id,
        pair_id: product.pair_id,
        symbol: product.symbol,
        stake_asset: product.stake_asset,
        stake_asset_symbol: product.stake_asset_symbol,
        logo_url: product.logo_url,
        duration_seconds: default_cycle
            .map(|cycle| cycle.duration_seconds)
            .unwrap_or(product.duration_seconds),
        payout_rate: default_cycle
            .map(|cycle| cycle.payout_rate.clone())
            .unwrap_or(product.payout_rate),
        min_stake: default_cycle
            .map(|cycle| cycle.min_stake.clone())
            .unwrap_or(product.min_stake),
        max_stake: default_cycle
            .map(|cycle| cycle.max_stake.clone())
            .unwrap_or(product.max_stake),
        cycles,
        status: product.status,
    }
}

/// 返回按主键查询单笔订单的公共 SQL 文本，连接池版本与事务版本共用同一份以保证字段集合完全一致。
/// 语句带一个订单主键占位符，连接 users、交易对与资产表以补齐邮箱和符号等展示字段，
/// 不含 `FOR UPDATE`，需要加锁的结算路径使用各自独立的语句。
fn seconds_contract_order_by_id_sql() -> &'static str {
    r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
              users.email, pairs.symbol, orders.stake_asset, assets.symbol AS stake_asset_symbol,
              orders.direction, orders.stake_amount, orders.duration_seconds,
              orders.payout_rate, orders.entry_price, orders.settlement_price,
                  orders.settlement_price_tick_id, orders.settlement_price_source,
                  orders.settlement_price_observed_at, orders.settlement_price_generation,
                  orders.settlement_price_version, orders.status, orders.result,
              orders.idempotency_key, orders.expires_at, orders.created_at
       FROM seconds_contract_orders orders
       INNER JOIN users ON users.id = orders.user_id
       INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
       INNER JOIN assets ON assets.id = orders.stake_asset
       WHERE orders.id = ?
       LIMIT 1"#
}
