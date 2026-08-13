//! 已实现收益事实与 USDT 估值查询。
//!
//! 资金不变量：四类终态事实共用同一 UTC 聚合公式；历史缺价不得回退当前价，异常行情只传播 partial，绝不伪造收益。

use crate::{
    error::AppResult,
    modules::market::{ValidatedMarketSymbol, kline_collection_name, market_ticker_redis_key},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use mongodb::{
    Database,
    bson::{DateTime as BsonDateTime, Document, doc},
};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Deserialize;
use sqlx::{MySql, Pool};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

const TODAY_RETURN_TICKER_MAX_AGE_SECONDS: i64 = 60;
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct TodayReturnAssetActivityRow {
    pub(crate) asset_symbol: String,
    pub(crate) amount: BigDecimal,
    pub(crate) basis_amount: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct ReturnHistoryAssetActivityRow {
    pub(crate) activity_day: NaiveDate,
    pub(crate) asset_symbol: String,
    pub(crate) amount: BigDecimal,
    pub(crate) basis_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct TodayReturnTickerPayload {
    symbol: String,
    last_price: BigDecimal,
    #[serde(with = "crate::time::unix_millis")]
    observed_at: DateTime<Utc>,
}

/// 聚合指定 UTC 时段内已实现收益与对应本金基数，按资产返回活动快照。
/// 内部直接复用历史聚合查询并丢弃其日期维度，因此当传入的时段跨越多个自然日时，同一资产会返回多条同名行。
/// 调用方通常只传入当日零点到计算时刻的区间，从而让每个资产恰好对应一行，本函数自身不做跨日合并。
/// 查询只读取可审计结算来源，不把充值、提现或内部划转误计为收益，也不锁定钱包或写入任何流水。
pub(crate) async fn load_today_return_asset_activity(
    pool: &Pool<MySql>,
    user_id: u64,
    period_start_at: DateTime<Utc>,
    calculated_at: DateTime<Utc>,
) -> AppResult<Vec<TodayReturnAssetActivityRow>> {
    let rows =
        load_return_history_asset_activity(pool, user_id, period_start_at, calculated_at).await?;
    Ok(rows
        .into_iter()
        .map(|row| TodayReturnAssetActivityRow {
            asset_symbol: row.asset_symbol,
            amount: row.amount,
            basis_amount: row.basis_amount,
        })
        .collect())
}

/// 按 UTC 自然日和资产聚合 Seconds、Prediction、Margin 与 Earn 终态事实的已实现收益及本金基数。
/// 四路子查询各自定义收益：秒合约赢按本金乘赔率、输按本金取负；预测按赔付加退款加费用退回减本金减手续费。
/// 杠杆取已实现盈亏减利息，本金基数取保证金；理财取赎回流水金额减申购本金，并只认同一申购的首条赎回流水以防重复计收益。
/// 日期维度分别取各业务的结算、平仓或赎回时刻，时间过滤为左闭右开，因此边界时刻只会归入一个自然日。
/// 时间戳按 UTC 朴素时刻绑定，日期由数据库直接截取，与应用层的 UTC 自然日口径保持一致。
/// 公式与 today-return 口径一致；查询不包含充值、提现、内部划转、未结算订单或未实现盈亏，也不锁钱包。
pub(crate) async fn load_return_history_asset_activity(
    pool: &Pool<MySql>,
    user_id: u64,
    period_start_at: DateTime<Utc>,
    calculated_at: DateTime<Utc>,
) -> AppResult<Vec<ReturnHistoryAssetActivityRow>> {
    let period_start_at = period_start_at.naive_utc();
    let calculated_at = calculated_at.naive_utc();
    let rows = sqlx::query_as::<_, ReturnHistoryAssetActivityRow>(
        r#"SELECT activity.activity_day,
                  activity.asset_symbol,
                  SUM(activity.amount) AS amount,
                  SUM(activity.basis_amount) AS basis_amount
           FROM (
               SELECT DATE(orders.settled_at) AS activity_day,
                      assets.symbol AS asset_symbol,
                      SUM(CASE
                              WHEN orders.result = 'win'
                                  THEN orders.stake_amount * orders.payout_rate
                              WHEN orders.result = 'loss'
                                  THEN -orders.stake_amount
                              ELSE 0
                          END) AS amount,
                      SUM(orders.stake_amount) AS basis_amount
               FROM seconds_contract_orders orders
               INNER JOIN assets ON assets.id = orders.stake_asset
               WHERE orders.user_id = ?
                 AND orders.status = 'settled'
                 AND orders.settled_at >= ?
                 AND orders.settled_at < ?
               GROUP BY DATE(orders.settled_at), assets.symbol

               UNION ALL

               SELECT DATE(orders.settled_at) AS activity_day,
                      assets.symbol AS asset_symbol,
                      SUM(orders.payout_amount + orders.refund_amount
                          + orders.fee_refund_amount - orders.stake_amount
                          - orders.fee_amount) AS amount,
                      SUM(orders.stake_amount + orders.fee_amount) AS basis_amount
               FROM prediction_orders orders
               INNER JOIN assets ON assets.id = orders.asset_id
               WHERE orders.user_id = ?
                 AND orders.status IN ('settled', 'refunded')
                 AND orders.settled_at >= ?
                 AND orders.settled_at < ?
               GROUP BY DATE(orders.settled_at), assets.symbol

               UNION ALL

               SELECT DATE(positions.closed_at) AS activity_day,
                      assets.symbol AS asset_symbol,
                      SUM(COALESCE(positions.realized_pnl, 0) - positions.interest_amount) AS amount,
                      SUM(positions.margin_amount) AS basis_amount
               FROM margin_positions positions
               INNER JOIN assets ON assets.id = positions.margin_asset
               WHERE positions.user_id = ?
                 AND positions.status IN ('closed', 'liquidated')
                 AND positions.closed_at >= ?
                 AND positions.closed_at < ?
               GROUP BY DATE(positions.closed_at), assets.symbol

               UNION ALL

               SELECT DATE(subscriptions.redeemed_at) AS activity_day,
                      assets.symbol AS asset_symbol,
                      SUM(ledger.amount - subscriptions.amount) AS amount,
                      SUM(subscriptions.amount) AS basis_amount
               FROM earn_subscriptions subscriptions
               INNER JOIN wallet_ledger ledger
                   ON ledger.user_id = subscriptions.user_id
                  AND ledger.asset_id = subscriptions.asset_id
                  AND ledger.change_type = 'earn_redeem'
                  AND ledger.ref_type = 'earn_subscription'
                  AND ledger.ref_id = CAST(subscriptions.id AS CHAR)
               INNER JOIN assets ON assets.id = subscriptions.asset_id
               WHERE subscriptions.user_id = ?
                 AND subscriptions.status = 'redeemed'
                 AND subscriptions.redeemed_at >= ?
                 AND subscriptions.redeemed_at < ?
                 AND NOT EXISTS (
                     SELECT 1
                     FROM wallet_ledger earlier_ledger
                     WHERE earlier_ledger.user_id = ledger.user_id
                       AND earlier_ledger.asset_id = ledger.asset_id
                       AND earlier_ledger.change_type = ledger.change_type
                       AND earlier_ledger.ref_type = ledger.ref_type
                       AND earlier_ledger.ref_id = ledger.ref_id
                       AND earlier_ledger.id < ledger.id
                 )
               GROUP BY DATE(subscriptions.redeemed_at), assets.symbol
           ) activity
           GROUP BY activity.activity_day, activity.asset_symbol
           ORDER BY activity.activity_day ASC, activity.asset_symbol ASC"#,
    )
    .bind(user_id)
    .bind(period_start_at)
    .bind(calculated_at)
    .bind(user_id)
    .bind(period_start_at)
    .bind(calculated_at)
    .bind(user_id)
    .bind(period_start_at)
    .bind(calculated_at)
    .bind(user_id)
    .bind(period_start_at)
    .bind(calculated_at)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// 为每个非稳定币和活动日期读取 `{ASSET}USDT` 集合中 open_time 精确等于 UTC 零点的 1d close。
/// 逐资产按其请求日期的最早与最晚构造一次左闭右开范围查询，再在游标中逐条筛出真正被请求的日期，避免为每天单独往返。
/// 交易对拼接后需通过行情符号校验，校验不过的资产直接跳过，不会让整批历史估值失败。
/// 单条 K 线文档损坏或价格非法时只跳过该条，对应日期表现为缺价而非抛出反序列化错误。
/// 本函数不为稳定币造价，也不回退相邻 K 线；缺失或非法价格留空，由应用层标记对应日期为 partial。
pub(crate) async fn load_historical_usdt_daily_closes(
    database: &Database,
    requested_days: &BTreeMap<String, BTreeSet<NaiveDate>>,
) -> AppResult<BTreeMap<(NaiveDate, String), BigDecimal>> {
    let mut prices = BTreeMap::new();
    for (asset_symbol, days) in requested_days {
        let Some(first_day) = days.first().copied() else {
            continue;
        };
        let Some(last_day) = days.last().copied() else {
            continue;
        };
        let Ok(symbol) = ValidatedMarketSymbol::from_raw(&format!("{asset_symbol}USDT")) else {
            continue;
        };
        let start_at = first_day
            .and_hms_opt(0, 0, 0)
            .expect("UTC calendar day start is always valid")
            .and_utc();
        let end_at = last_day
            .succ_opt()
            .expect("return-history dates are within chrono range")
            .and_hms_opt(0, 0, 0)
            .expect("UTC calendar day start is always valid")
            .and_utc();
        let collection = database.collection::<Document>(&kline_collection_name(&symbol));
        let filter = doc! {
            "interval": "1d",
            "open_time": {
                "$gte": BsonDateTime::from_millis(start_at.timestamp_millis()),
                "$lt": BsonDateTime::from_millis(end_at.timestamp_millis()),
            },
        };
        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "open_time": 1 })
            .build();
        let mut cursor = collection.find(filter).with_options(options).await?;
        while cursor.advance().await? {
            let document = cursor.deserialize_current()?;
            let Some((day, price)) = return_history_kline_document_close_if_valid(&document, days)
            else {
                continue;
            };
            prices.insert((day, asset_symbol.clone()), price);
        }
    }
    Ok(prices)
}

/// 从 Mongo 的 K 线文档中取出开盘时刻与收盘价字段，再交由日期与价格校验决定是否可用。
/// 字段缺失或类型不符时返回空值而非报错，因此单条损坏 K 线只表现为该日缺价，不能把整个历史接口升级成反序列化 5xx。
pub(crate) fn return_history_kline_document_close_if_valid(
    document: &Document,
    requested_days: &BTreeSet<NaiveDate>,
) -> Option<(NaiveDate, BigDecimal)> {
    let open_time = document.get_datetime("open_time").ok()?;
    let close = document.get_str("close").ok()?;
    return_history_historical_close_if_valid(open_time.timestamp_millis(), close, requested_days)
}

/// 历史估值拒绝错日、非日初、非法和非正 close，缺失由应用层统一传播为 partial。
/// 先要求开盘毫秒时刻能被一整天整除，从而排除非日初的分钟或小时级 K 线被误当作日线收盘价。
/// 再要求换算出的 UTC 日期确实在请求集合中，避免范围查询带回的相邻日期污染估值结果。
/// 收盘价按去空白后的十进制解析，解析失败或结果不大于零一律返回空值，绝不用零价或负价参与收益换算。
/// 从历史 K 线文档提取严格为正的收盘价及其业务日期，返回空时由应用层统一标记该日为 partial。
pub(crate) fn return_history_historical_close_if_valid(
    open_time_millis: i64,
    close: &str,
    requested_days: &BTreeSet<NaiveDate>,
) -> Option<(NaiveDate, BigDecimal)> {
    if open_time_millis.rem_euclid(86_400_000) != 0 {
        return None;
    }
    let open_time = DateTime::<Utc>::from_timestamp_millis(open_time_millis)?;
    let day = open_time.date_naive();
    if !requested_days.contains(&day) {
        return None;
    }
    let price = BigDecimal::from_str(close.trim()).ok()?;
    (price > 0).then_some((day, price))
}

/// 批量读取非稳定币 `{ASSET}USDT` Redis ticker，仅收集交易对匹配、正数且相对计算时刻 60 秒内的价格。
/// 资产列表为空时直接返回空表，不与 Redis 交互；否则按列表顺序拼出行情键并用一次批量读取取回全部快照。
/// 返回结果与入参逐项对齐后逐个校验，未通过校验的资产在结果中直接缺席，而不是以零价或旧价占位。
/// 缺失、字段异常、过期或未来时间快照均按缺价留空；函数不改写缓存，由应用层传播 partial。
pub(crate) async fn load_current_usdt_prices(
    redis: &ConnectionManager,
    asset_symbols: &[String],
    calculated_at: DateTime<Utc>,
) -> AppResult<BTreeMap<String, BigDecimal>> {
    if asset_symbols.is_empty() {
        return Ok(BTreeMap::new());
    }

    let keys = asset_symbols
        .iter()
        .map(|asset_symbol| market_ticker_redis_key(&format!("{asset_symbol}USDT")))
        .collect::<Vec<_>>();
    let mut connection = redis.clone();
    let payloads: Vec<Option<String>> = connection.mget(keys).await?;
    let prices = asset_symbols
        .iter()
        .zip(payloads)
        .filter_map(|(asset_symbol, payload)| {
            let payload = payload?;
            today_return_ticker_price_if_current(asset_symbol, &payload, calculated_at)
                .map(|price| (asset_symbol.clone(), price))
        })
        .collect();

    Ok(prices)
}

/// 今日收益只接受与 Redis key 对应、价格为正且时间新鲜的行情，异常缓存统一按缺价处理。
/// 校验行情快照的交易对、时间戳和正价格后返回当前估值价。
/// 超过允许陈旧窗口或字段异常时返回空，避免旧行情进入当日收益。
pub(crate) fn today_return_ticker_price_if_current(
    asset_symbol: &str,
    payload: &str,
    calculated_at: DateTime<Utc>,
) -> Option<BigDecimal> {
    let ticker = serde_json::from_str::<TodayReturnTickerPayload>(payload).ok()?;
    let expected_symbol = format!("{}USDT", asset_symbol.trim().to_ascii_uppercase());
    if ticker.symbol.trim().to_ascii_uppercase() != expected_symbol || ticker.last_price <= 0 {
        return None;
    }

    let allowed_clock_distance = chrono::TimeDelta::seconds(TODAY_RETURN_TICKER_MAX_AGE_SECONDS);
    if ticker.observed_at < calculated_at - allowed_clock_distance
        || ticker.observed_at > calculated_at
    {
        return None;
    }

    Some(ticker.last_price)
}
