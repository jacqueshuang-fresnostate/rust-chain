//! risk bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 风控的外部依赖分三块：从 MySQL 实时读启用规则、把拒绝事件写入审计表、在 Redis 上做固定窗口限频计数。
//! 规则读取刻意不加任何缓存，保证后台启停规则立刻生效；限频计数走 Lua 脚本保持自增与过期的原子性。

use crate::{
    error::AppResult,
    modules::risk::{repository::RiskEventWrite, service::StoredRiskRule},
};
use redis::{Script, aio::ConnectionManager};
use serde_json::Value;
use sqlx::{MySql, Pool, types::Json as SqlxJson};
use std::sync::LazyLock;

/// 规则每次调用实时读取：风控开关必须改完即生效，不做缓存以免运营刚停用的规则还在拦单。
/// `ORDER BY id` 只为让日志与审计里的规则顺序稳定，策略解析本身与顺序无关。
/// 只取目标类型、目标标识与配置 JSON 三列，规则维度的语义解析全部留给服务层，基础设施不理解配置内容。
pub async fn load_enabled_risk_rules(pool: &Pool<MySql>) -> AppResult<Vec<StoredRiskRule>> {
    let rows = sqlx::query_as::<_, (String, Option<String>, SqlxJson<Value>)>(
        "SELECT target_type, target_id, config_json FROM risk_rules WHERE enabled = TRUE ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(target_type, target_id, config)| StoredRiskRule {
            target_type,
            target_id,
            config: config.0,
        })
        .collect())
}

/// 追加被拒绝请求的风控审计事件，保存用户、操作、规则快照和拒绝原因。
/// 该留痕不参与业务资金事务；应用层会记录写入故障但保留原风控拒绝结果。
/// 行为主体类型固定写死为 user 且主体标识复用用户 ID，当前没有由管理员或系统发起的风控事件来源。
pub async fn insert_risk_event(pool: &Pool<MySql>, event: RiskEventWrite) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO risk_events
           (user_id, actor_type, actor_id, event_type, risk_level, decision, reason, payload_json)
           VALUES (?, 'user', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(event.user_id)
    .bind(event.user_id)
    .bind(event.event_type)
    .bind(event.risk_level)
    .bind(event.decision)
    .bind(event.reason)
    .bind(SqlxJson(event.payload))
    .execute(pool)
    .await?;

    Ok(())
}

/// 计数与过期在同一段 Lua 里原子完成，进程在两条命令之间中断也不会留下没有 TTL 的计数键；
/// `EXPIRE ... NX` 只在缺失 TTL 时补设，既保持固定窗口不被窗口内的后续请求续期，又能修复历史遗留的无 TTL 键。
static RATE_LIMIT_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"local count = redis.call('INCR', KEYS[1])
redis.call('EXPIRE', KEYS[1], ARGV[1], 'NX')
return count"#,
    )
});

/// 拼出限频计数在 Redis 中的键名，由固定前缀加操作、规则作用域和用户 ID 四段组成。
/// 作用域参与命名是关键：同一用户在不同作用域下的规则各自独立计数，不会互相消耗配额。
/// 键名不含窗口长度，因此在线调整窗口秒数只影响后续 TTL 补设，不会切换到新的计数桶。
pub fn user_request_count_key(operation: &str, scope: &str, user_id: u64) -> String {
    format!("risk:rate:{operation}:{scope}:{user_id}")
}

/// 通过 Redis Lua 原子递增固定窗口计数并仅在缺失时设置 TTL，避免并发续期窗口。
/// Redis 故障返回错误，由应用层按既有放行策略处理；本函数不访问数据库或资金账户。
/// 返回自增后的最新计数，超出 u32 上界时饱和到最大值，使极端刷量场景必然判为超限而不会回绕成小数值。
pub async fn bump_user_request_count(
    redis: &ConnectionManager,
    user_id: u64,
    operation: &str,
    scope: &str,
    window_seconds: u32,
) -> AppResult<u32> {
    let mut connection = redis.clone();
    let count: u64 = RATE_LIMIT_SCRIPT
        .key(user_request_count_key(operation, scope, user_id))
        .arg(i64::from(window_seconds))
        .invoke_async(&mut connection)
        .await?;

    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}
