//! 策略盘口与展示成交的原子缓存；数据不进入撮合、持仓或账本。
use super::{
    MarketCacheError, MarketCacheWriteOutcome, MarketDepthCacheEntry, MarketTickerCacheEntry,
    RedisMarketCache, cache_write_outcome, market_depth_redis_key, market_ticker_redis_key,
};
use crate::modules::market::{MarketTradeTick, sanitize_symbol};
use redis::{AsyncCommands, Script};
use std::sync::LazyLock;

static SAVE_DETAILS: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local ticker = redis.call('GET', KEYS[1])
if ticker ~= ARGV[1] then return 0 end
local previous = redis.call('GET', KEYS[4])
local newest = redis.call('LRANGE', KEYS[3], 0, 0)
local insert_trade = ARGV[3] ~= ''
if insert_trade and #newest > 0 then
    local previous_trade = cjson.decode(newest[1])
    local incoming_trade = cjson.decode(ARGV[3])
    if previous_trade.trade_id == incoming_trade.trade_id then
        if newest[1] ~= ARGV[3] then return 0 end
        insert_trade = false
    elseif previous_trade.traded_at > incoming_trade.traded_at then
        return 0
    end
end
if previous == ARGV[2] then
    if insert_trade then return 0 end
    return 2
end
redis.call('SET', KEYS[2], ARGV[2])
redis.call('SET', KEYS[4], ARGV[2])
if insert_trade then
    redis.call('LPUSH', KEYS[3], ARGV[3])
    redis.call('LTRIM', KEYS[3], 0, 99)
    return 3
end
return 1
"#,
    )
});

/// 返回独立的策略展示成交队列键；不复用真实成交表或用户订单键。
pub fn market_synthetic_trades_redis_key(symbol: &str) -> String {
    format!("market:synthetic-trades:{}", sanitize_symbol(symbol))
}

impl RedisMarketCache {
    /// 仅在 Redis ticker 与本轮已归档 ticker 完全一致时原子更新盘口和最近 100 条模拟成交。
    /// 每秒成交号判重；同号异载荷和旧成交拒绝，重放不追加。返回值标记是否插入新逐笔以控制广播。
    /// ticker 在检查后被另一轮替换的竞态由 Lua 单命令消除；本方法不触发任何真实订单。
    pub async fn save_synthetic_details(
        &self,
        ticker: MarketTickerCacheEntry,
        depth: MarketDepthCacheEntry,
        trade: Option<&MarketTradeTick>,
    ) -> Result<(MarketCacheWriteOutcome, bool), MarketCacheError> {
        let mut connection = self.manager.clone();
        let outcome: i64 = SAVE_DETAILS
            .key(market_ticker_redis_key(ticker.symbol()))
            .key(market_depth_redis_key(ticker.symbol()))
            .key(market_synthetic_trades_redis_key(ticker.symbol()))
            .key(format!("market:synthetic-details:{}", ticker.symbol()))
            .arg(serde_json::to_string(&ticker)?)
            .arg(serde_json::to_string(&depth)?)
            .arg(
                trade
                    .map(serde_json::to_string)
                    .transpose()?
                    .unwrap_or_default(),
            )
            .invoke_async(&mut connection)
            .await?;
        Ok((
            cache_write_outcome(if outcome == 3 { 1 } else { outcome }),
            outcome == 3,
        ))
    }

    /// 读取已有策略展示成交，按新到旧返回且最多 100 条；缺失缓存返回空，不伪造历史成交。
    /// JSON 损坏或 Redis 故障返回错误，不回退真实撮合表来冒充同一个行情源。
    pub async fn load_synthetic_trades(
        &self,
        symbol: &str,
        limit: u32,
    ) -> Result<Vec<MarketTradeTick>, MarketCacheError> {
        let mut connection = self.manager.clone();
        let rows: Vec<String> = connection
            .lrange(
                market_synthetic_trades_redis_key(symbol),
                0,
                limit.clamp(1, 100) as isize - 1,
            )
            .await?;
        rows.iter()
            .map(|row| serde_json::from_str(row).map_err(MarketCacheError::from))
            .collect()
    }
}
