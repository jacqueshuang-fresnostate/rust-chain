//! 跟随行情的真实三库回归；显式配置本地服务后执行，缺少配置按项目惯例提示跳过。

use std::{error::Error, panic::AssertUnwindSafe, str::FromStr, time::Duration};

use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, TimeDelta, Timelike, Utc};
use exchange_api::{
    modules::market::{
        MarketKlineSnapshot, MarketKlineValues, RedisMarketCache, adapters::MarketIngestionService,
        sanitize_symbol, synthetic_default::forming_default_1m_values,
    },
    workers::synthetic_market::run_once_with_dependencies,
};
use futures_util::FutureExt;
use mongodb::bson::{Document, doc};
use redis::AsyncCommands;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{MySqlPool, mysql::MySqlConnectOptions, mysql::MySqlPoolOptions};
use uuid::Uuid;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("decimal fixture")
}

fn field(value: &Value, key: &str) -> BigDecimal {
    decimal(value[key].as_str().expect("decimal string in market JSON"))
}

fn minute(time: DateTime<Utc>) -> DateTime<Utc> {
    time.with_second(0).unwrap().with_nanosecond(0).unwrap()
}

async fn scenario_minute() -> DateTime<Utc> {
    let now = Utc::now();
    let boundary = minute(now)
        + if now.second() > 20 {
            TimeDelta::minutes(1)
        } else {
            TimeDelta::zero()
        };
    let ready = boundary + TimeDelta::milliseconds(3_200);
    if ready > now {
        tokio::time::sleep(Duration::from_millis(
            (ready - now).num_milliseconds() as u64
        ))
        .await;
    }
    assert!(Utc::now() >= boundary + TimeDelta::seconds(3));
    boundary
}

struct FollowFixture {
    bootstrap: MySqlPool,
    database: String,
    pool: MySqlPool,
    mongo: mongodb::Database,
    redis: redis::aio::ConnectionManager,
    ingestion: MarketIngestionService,
    pair: u64,
    reference: u64,
    symbol: String,
    reference_symbol: String,
    trigger: String,
}

impl FollowFixture {
    async fn new() -> TestResult<Option<Self>> {
        let (Ok(mysql_url), Ok(mongo_url), Ok(redis_url)) = (
            std::env::var("DATABASE_URL"),
            std::env::var("DEFAULT_MARKET_TEST_MONGO_URL"),
            std::env::var("DEFAULT_MARKET_TEST_REDIS_URL"),
        ) else {
            eprintln!(
                "skipping follow runtime integration: isolated DATABASE_URL and DEFAULT_MARKET_TEST_MONGO_URL/REDIS_URL are required"
            );
            return Ok(None);
        };
        assert!(mongo_url.starts_with("mongodb://127.0.0.1:"));
        assert!(redis_url.starts_with("redis://127.0.0.1:"));
        let options = MySqlConnectOptions::from_str(&mysql_url)?;
        assert!(matches!(options.get_host(), "localhost" | "127.0.0.1"));
        assert!(
            options
                .get_database()
                .is_some_and(|name| name.starts_with("codex_"))
        );
        let bootstrap = MySqlPoolOptions::new()
            .max_connections(2)
            .connect_with(options.clone())
            .await?;
        let suffix = Uuid::now_v7().simple().to_string()[16..24].to_owned();
        let database = format!("codex_default_follow_runtime_{suffix}");
        eprintln!("Creating isolated follow fixture database {database}");
        sqlx::raw_sql(&format!("CREATE DATABASE `{database}`"))
            .execute(&bootstrap)
            .await?;
        let pool = MySqlPoolOptions::new()
            .max_connections(8)
            .connect_with(options.database(&database))
            .await?;
        sqlx::migrate!().run(&pool).await?;
        let mut assets = Vec::new();
        for prefix in ["FT", "FB", "FQ"] {
            assets.push(sqlx::query("INSERT INTO assets(symbol,name,precision_scale,asset_type,status) VALUES(?,?,8,'coin','active')")
                .bind(format!("{prefix}{suffix}")).bind("Local follow fixture asset")
                .execute(&pool).await?.last_insert_id());
        }
        let raw_symbol = format!("FT{suffix}-FQ{suffix}");
        let raw_reference = format!("FB{suffix}-FQ{suffix}");
        let mut pairs = Vec::new();
        for (base, symbol, kind) in [
            (assets[0], raw_symbol.as_str(), "strategy"),
            (assets[1], raw_reference.as_str(), "external"),
        ] {
            pairs.push(sqlx::query("INSERT INTO trading_pairs(base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,status,market_type) VALUES(?,?,?,8,2,1,'active',?)")
                .bind(base).bind(assets[2]).bind(symbol).bind(kind)
                .execute(&pool).await?.last_insert_id());
        }
        let config = json!({
            "mode":"follow", "follow":{
                "reference_pair_id":pairs[1], "multiplier":"1",
                "max_move_ratio":"0.5", "stale_after_seconds":15
            },
            "volatility":"0.02", "mean_reversion":"0", "wick_strength":"0.1",
            "price_min":"0.001", "price_max":"1", "volume_min":"60", "volume_max":"60"
        });
        sqlx::query("INSERT INTO market_default_generators(pair_id,enabled,initial_price,config_json,version,seed) VALUES(?,TRUE,0.1,?,1,'follow-runtime-fixture')")
            .bind(pairs[0]).bind(sqlx::types::Json(config)).execute(&pool).await?;
        sqlx::query("INSERT INTO market_default_generator_versions(pair_id,version,initial_price,config_json,seed) SELECT pair_id,version,initial_price,config_json,seed FROM market_default_generators WHERE pair_id=?")
            .bind(pairs[0]).execute(&pool).await?;
        let mongo = mongodb::Client::with_uri_str(mongo_url)
            .await?
            .database(&database);
        let redis = redis::Client::open(redis_url)?
            .get_connection_manager()
            .await?;
        let ingestion =
            MarketIngestionService::new(RedisMarketCache::new(redis.clone()), mongo.clone())
                .with_mysql(Some(pool.clone()));
        Ok(Some(Self {
            bootstrap,
            database,
            pool,
            mongo,
            redis,
            ingestion,
            pair: pairs[0],
            reference: pairs[1],
            symbol: sanitize_symbol(&raw_symbol),
            reference_symbol: sanitize_symbol(&raw_reference),
            trigger: format!("fail_follow_{suffix}"),
        }))
    }

    async fn reference_tick(&self, price: &str, at: DateTime<Utc>) -> TestResult {
        assert!(
            at <= Utc::now(),
            "fixture never inserts future reference evidence"
        );
        let price = decimal(price);
        let key = hex::encode(Sha256::digest(format!(
            "bitget|{}|{}|{}",
            self.reference_symbol,
            at.timestamp_micros(),
            price.normalized()
        )));
        sqlx::query("INSERT INTO market_price_ticks(event_key,symbol,price,source,observed_at,generation,source_version) VALUES(?,?,?,'bitget',?,1,?)")
            .bind(&key).bind(&self.reference_symbol).bind(price).bind(at.naive_utc()).bind(&key)
            .execute(&self.pool).await?;
        Ok(())
    }

    async fn run(&self, at: DateTime<Utc>, owner: &str, succeeds: bool) -> TestResult {
        assert!(
            at <= Utc::now(),
            "worker observation must not be future dated"
        );
        let result =
            run_once_with_dependencies(&self.pool, &self.mongo, &self.ingestion, at, 10, owner)
                .await?;
        assert_eq!(
            result.scanned, 1,
            "isolated database has one generation candidate: {result:?}"
        );
        assert_eq!(
            (result.published, result.failed),
            if succeeds { (1, 0) } else { (0, 1) },
            "{result:?}"
        );
        Ok(())
    }

    async fn state(&self) -> TestResult<Value> {
        Ok(sqlx::query_scalar::<_, sqlx::types::Json<Value>>(
            "SELECT state_json FROM market_pair_generation_runs WHERE pair_id=?",
        )
        .bind(self.pair)
        .fetch_one(&self.pool)
        .await?
        .0)
    }

    async fn ticker(&mut self) -> TestResult<Value> {
        let raw: String = self
            .redis
            .get(format!("market:ticker:{}", self.symbol))
            .await?;
        Ok(serde_json::from_str(&raw)?)
    }

    async fn count(&self) -> TestResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM market_price_ticks WHERE symbol=?")
                .bind(&self.symbol)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    async fn candle(&self, open: DateTime<Utc>) -> TestResult<Document> {
        Ok(self.mongo.collection::<Document>(&format!("market_klines_{}",self.symbol))
            .find_one(doc! {"interval":"1m","open_time":mongodb::bson::DateTime::from_millis(open.timestamp_millis())})
            .await?.expect("authoritative candle exists"))
    }

    async fn fail_archival(&self, enabled: bool) -> TestResult {
        let statement = if enabled {
            format!(
                "CREATE TRIGGER {} BEFORE INSERT ON market_price_ticks FOR EACH ROW BEGIN IF NEW.symbol='{}' AND NEW.source='default' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='follow fixture archive failure'; END IF; END",
                self.trigger, self.symbol
            )
        } else {
            format!("DROP TRIGGER {}", self.trigger)
        };
        sqlx::raw_sql(&statement).execute(&self.pool).await?;
        Ok(())
    }

    async fn enable_reference(&self, enabled: bool) -> TestResult {
        sqlx::query("UPDATE trading_pairs SET status=? WHERE id=?")
            .bind(if enabled { "active" } else { "disabled" })
            .bind(self.reference)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn cleanup(mut self) -> TestResult {
        let keys: Vec<String> = self.redis.keys(format!("market:*{}*", self.symbol)).await?;
        if !keys.is_empty() {
            let _: i64 = self.redis.del(keys).await?;
        }
        self.mongo.drop().await?;
        self.pool.close().await;
        sqlx::raw_sql(&format!("DROP DATABASE `{}`", self.database))
            .execute(&self.bootstrap)
            .await?;
        self.bootstrap.close().await;
        Ok(())
    }
}

async fn follow_and_recovery(fixture: &mut FollowFixture) -> TestResult {
    let boundary = scenario_minute().await;
    let open = boundary - TimeDelta::minutes(1);
    let at = |second| open + TimeDelta::seconds(second);
    fixture
        .reference_tick("10000", at(40) - TimeDelta::milliseconds(250))
        .await?;
    fixture.run(at(40), "follow-owner-a", true).await?;
    assert_eq!(
        field(&fixture.ticker().await?, "last_price"),
        decimal("0.1")
    );
    fixture
        .reference_tick("10100", at(41) - TimeDelta::milliseconds(500))
        .await?;
    fixture.run(at(41), "follow-owner-a", true).await?;
    let followed = fixture.ticker().await?;
    assert_eq!(field(&followed, "last_price"), decimal("0.101"));
    let state = fixture.state().await?;
    assert_eq!(state["follow"]["status"]["mode"], "following");
    assert_eq!(
        field(&state["follow"]["last_reference"], "price"),
        decimal("10100")
    );
    let count = fixture.count().await?;
    let trade_key = format!("market:synthetic-trades:{}", fixture.symbol);
    let trades: i64 = fixture.redis.llen(&trade_key).await?;
    let trade: Value =
        serde_json::from_str(&fixture.redis.lindex::<_, String>(&trade_key, 0).await?)?;
    let own_price = field(&followed, "last_price");
    assert_eq!(field(&trade, "price"), own_price);
    let depth: Value = serde_json::from_str(
        &fixture
            .redis
            .get::<_, String>(format!("market:depth:{}", fixture.symbol))
            .await?,
    )?;
    assert!(field(&depth["bids"][0], "price") <= own_price);
    assert!(own_price <= field(&depth["asks"][0], "price"));
    for interval in ["5m", "15m", "1h", "4h", "1d"] {
        let candle: Value = serde_json::from_str(
            &fixture
                .redis
                .get::<_, String>(format!("market:kline:{}:{interval}", fixture.symbol))
                .await?,
        )?;
        assert_eq!(
            field(&candle, "close"),
            own_price,
            "{interval} aggregates the followed own-price path"
        );
    }
    fixture
        .reference_tick("10500", at(41) - TimeDelta::milliseconds(250))
        .await?;
    fixture.run(at(41), "follow-owner-b", true).await?;
    assert_eq!(
        fixture.ticker().await?,
        followed,
        "same second freezes ticker despite a newer archived reference"
    );
    assert_eq!(fixture.state().await?["follow"], state["follow"]);
    assert_eq!(fixture.count().await?, count);
    assert_eq!(fixture.redis.llen::<_, i64>(&trade_key).await?, trades);

    // 模拟整体暂停的代际撤销；只有本测试持有的独立交易对受影响。
    let mut tx = fixture.pool.begin().await?;
    sqlx::query("UPDATE market_default_generators SET all_market_paused=TRUE WHERE pair_id=?")
        .bind(fixture.pair)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE market_pair_generation_runs SET generation=generation+1,active_source='none',strategy_id=NULL,strategy_version=NULL,default_version=NULL,lease_owner=NULL,lease_expires_at=NULL WHERE pair_id=?")
        .bind(fixture.pair).execute(&mut *tx).await?;
    sqlx::query("UPDATE market_default_generators SET all_market_paused=FALSE WHERE pair_id=?")
        .bind(fixture.pair)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    fixture.run(at(56), "follow-owner-b", true).await?;
    assert_eq!(
        field(&fixture.ticker().await?, "last_price"),
        decimal("0.101"),
        "resume must keep last archived gain across invalidated generation"
    );
    let stale = fixture.state().await?;
    assert_eq!(stale["follow"]["status"]["mode"], "fallback");
    assert!(
        !stale["follow"]["status"]["fallback_reason"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let independent: MarketKlineSnapshot = serde_json::from_value(stale["closed"].clone())?;
    let values = MarketKlineValues {
        open: independent.open().clone(),
        high: independent.high().clone(),
        low: independent.low().clone(),
        close: independent.close().clone(),
        volume: independent.volume().clone(),
    };
    let base56 = forming_default_1m_values(&values, open, at(56), 8, 2)?;
    let base57 = forming_default_1m_values(&values, open, at(57), 8, 2)?;
    fixture.run(at(57), "follow-owner-b", true).await?;
    let fallback_price = field(&fixture.ticker().await?, "last_price");
    assert_eq!(
        fallback_price,
        (decimal("0.101") * base57.close / base56.close).with_scale_round(8, RoundingMode::HalfUp),
        "fallback uses independent movement instead of stale BTC catch-up"
    );
    fixture
        .reference_tick("50000", at(58) - TimeDelta::milliseconds(250))
        .await?;
    fixture.run(at(58), "follow-owner-b", true).await?;
    assert_eq!(
        field(&fixture.ticker().await?, "last_price"),
        fallback_price,
        "recovery re-anchors rather than copying missed reference gain"
    );
    fixture
        .reference_tick("50500", at(59) - TimeDelta::milliseconds(250))
        .await?;
    fixture.run(at(59), "follow-owner-b", true).await?;
    let closing = field(&fixture.ticker().await?, "last_price");
    assert_eq!(
        closing,
        (&fallback_price * decimal("1.01")).with_scale_round(8, RoundingMode::HalfUp)
    );
    let before = fixture.candle(open).await?;
    fixture.run(boundary, "follow-owner-b", true).await?;
    let old = fixture.candle(open).await?;
    for key in ["open", "high", "low", "close", "volume"] {
        assert_eq!(
            decimal(old.get_str(key)?),
            decimal(before.get_str(key)?),
            "minute closure preserves actual {key}"
        );
    }
    assert_eq!(decimal(old.get_str("close")?), closing);
    assert_eq!(
        decimal(fixture.candle(boundary).await?.get_str("open")?),
        closing
    );
    let runtime: (String, Option<String>) = sqlx::query_as(
        "SELECT active_source,error_message FROM market_pair_generation_runs WHERE pair_id=?",
    )
    .bind(fixture.pair)
    .fetch_one(&fixture.pool)
    .await?;
    assert_eq!(runtime, ("default".into(), None));
    assert_eq!(
        fixture.state().await?["follow"]["status"]["mode"],
        "following"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM spot_trades")
            .fetch_one(&fixture.pool)
            .await?,
        0
    );
    eprintln!(
        "Real stores verified 1% following, same-second replay, pause generation, stale fallback, recovery and minute close"
    );
    Ok(())
}

async fn unpublished_frames(fixture: &mut FollowFixture) -> TestResult {
    let boundary = scenario_minute().await;
    let open = boundary - TimeDelta::minutes(1);
    let at = |second| open + TimeDelta::seconds(second);
    fixture
        .reference_tick("10000", at(54) - TimeDelta::milliseconds(250))
        .await?;
    fixture.run(at(54), "follow-owner", true).await?;
    fixture
        .reference_tick("10100", at(55) - TimeDelta::milliseconds(250))
        .await?;
    fixture.run(at(55), "follow-owner", true).await?;
    fixture
        .reference_tick("11000", at(56) - TimeDelta::milliseconds(250))
        .await?;
    fixture.fail_archival(true).await?;
    fixture.run(at(56), "follow-owner", false).await?;
    fixture.fail_archival(false).await?;
    assert_eq!(fixture.count().await?, 2);
    assert_eq!(
        field(&fixture.state().await?["follow"]["frame"], "close"),
        decimal("0.11")
    );
    assert_eq!(
        field(&fixture.ticker().await?, "last_price"),
        decimal("0.101")
    );
    fixture.enable_reference(false).await?;
    fixture.run(at(57), "follow-owner", true).await?;
    assert_eq!(
        field(&fixture.ticker().await?, "last_price"),
        decimal("0.101"),
        "fallback must discard unarchived pending jump"
    );
    assert_eq!(
        field(&fixture.state().await?["follow"]["frame"], "high"),
        decimal("0.101")
    );
    fixture.enable_reference(true).await?;
    fixture
        .reference_tick("50000", at(58) - TimeDelta::milliseconds(250))
        .await?;
    fixture.run(at(58), "follow-owner", true).await?;
    fixture
        .reference_tick("55000", at(59) - TimeDelta::milliseconds(250))
        .await?;
    fixture.fail_archival(true).await?;
    fixture.run(at(59), "follow-owner", false).await?;
    fixture.fail_archival(false).await?;
    assert_eq!(
        field(&fixture.state().await?["follow"]["frame"], "close"),
        decimal("0.1111")
    );
    let manual=sqlx::query("INSERT INTO market_strategies(pair_id,strategy_type,start_price,target_price,start_time,end_time,volatility,volume_min,volume_max,status) VALUES(?,'price_path',0.2,0.21,?,?,0.01,60,60,'active')")
        .bind(fixture.pair).bind(boundary.naive_utc()).bind((boundary+TimeDelta::minutes(30)).naive_utc())
        .execute(&fixture.pool).await?.last_insert_id();
    sqlx::query("INSERT INTO strategy_versions(strategy_id,version,effective_time,config_json,seed) VALUES(?,1,?,JSON_OBJECT(),'follow-handoff-manual')")
        .bind(manual).bind(boundary.naive_utc()).execute(&fixture.pool).await?;
    sqlx::query(
        "INSERT INTO strategy_runs(strategy_id,active_version,run_status) VALUES(?,1,'running')",
    )
    .bind(manual)
    .execute(&fixture.pool)
    .await?;
    fixture.run(boundary, "follow-owner", true).await?;
    let closed = fixture.candle(open).await?;
    assert_eq!(
        decimal(closed.get_str("close")?),
        decimal("0.101"),
        "manual takeover closes only the accepted follow path"
    );
    assert_eq!(
        decimal(closed.get_str("high")?),
        decimal("0.101"),
        "failed pending high must not enter historical OHLC"
    );
    let identity:(String,Option<u64>)=sqlx::query_as("SELECT source,strategy_id FROM market_price_ticks WHERE symbol=? ORDER BY observed_at DESC,id DESC LIMIT 1")
        .bind(&fixture.symbol).fetch_one(&fixture.pool).await?;
    assert_eq!(identity, ("strategy".into(), Some(manual)));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM spot_trades")
            .fetch_one(&fixture.pool)
            .await?,
        0
    );
    eprintln!(
        "Injected archive failures verified accepted-price fallback and manual close excluding unpublished jump"
    );
    Ok(())
}

#[tokio::test]
async fn follow_reference_replays_falls_back_recovers_and_closes_real_candles() -> TestResult {
    let Some(mut fixture) = FollowFixture::new().await? else {
        return Ok(());
    };
    let result = AssertUnwindSafe(follow_and_recovery(&mut fixture))
        .catch_unwind()
        .await;
    fixture.cleanup().await?;
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[tokio::test]
async fn unarchived_follow_frames_never_anchor_fallback_or_manual_handoff() -> TestResult {
    let Some(mut fixture) = FollowFixture::new().await? else {
        return Ok(());
    };
    let result = AssertUnwindSafe(unpublished_frames(&mut fixture))
        .catch_unwind()
        .await;
    fixture.cleanup().await?;
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
