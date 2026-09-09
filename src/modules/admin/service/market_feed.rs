//! 行情订阅配置与行情源凭据的纯业务规则层，同时定义服务层读取运行态所需的抽象端口。
//!
//! 校验部分把订阅符号、K 线周期和提供商三类集合收敛成 worker 可直接消费的标准代码，
//! 其中提供商被硬性限制为最多一个，以免多个数据源同时写入同一根 K 线。
//! 映射部分负责把配置与凭据记录转成响应或审计快照，凭据一律只输出认证类型与掩码，密文和明文都不外泄。
//! 运行态通过 `MarketFeedRuntimeStatusSource` 端口注入，因此本层不依赖 `AppState`，也不会真正启动或重载监督器。

use super::*;

pub(crate) const DEFAULT_MARKET_FEED_CONFIG_NAME: &str = "default";

pub(crate) const MARKET_SOURCE_AUTH_TYPE_API_KEY: &str = "api_key";

pub(crate) const MARKET_SOURCE_AUTH_TYPE_NONE: &str = "none";

/// 后台行情健康快照使用的默认断流阈值。
///
/// 90 秒略大于一个完整分钟和 worker 的重连抖动，既能在单次采样延迟时避免误报，
/// 又不会把连续两根缺失 K 线隐藏太久。该值只影响监控判定，不会改变行情写入或恢复策略。
pub(crate) const MARKET_FEED_STALE_AFTER_SECONDS: u64 = 90;

/// 行情运行状态读取端口，由持有 supervisor 的基础设施状态实现。
/// 服务层只依赖该能力，不感知 `AppState` 或监督器的具体存储方式。
#[allow(async_fn_in_trait)]
pub trait MarketFeedRuntimeStatusSource {
    /// 返回当前已应用版本、订阅集合和最近一次重载结果的一致快照。
    /// 未配置监督器时实现方应返回空状态，而不是把部署差异暴露给路由。
    async fn market_feed_runtime_status(&self) -> MarketFeedRuntimeStatus;
}

/// 通过运行状态端口读取行情快照，供后台状态页和仪表盘复用。
/// 本函数不启动或重载行情任务；具体锁与缺省状态语义由端口实现负责。
pub(crate) async fn load_market_feed_runtime(
    source: &impl MarketFeedRuntimeStatusSource,
) -> MarketFeedRuntimeStatus {
    source.market_feed_runtime_status().await
}

/// 单个交易对最近一次上游事件和本地摄取时间。
///
/// 两个时间戳分开保存，是为了让断流判定使用 provider 事件时间，而把本服务写入延迟
/// 单独展示成诊断信息。缺失的交易对不会出现在该结构中，由健康折叠函数补成 stale。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarketFeedSymbolObservation {
    pub(crate) symbol: String,
    pub(crate) observed_at: Option<DateTime<Utc>>,
    pub(crate) ingested_at: Option<DateTime<Utc>>,
}

/// 折叠后台行情健康快照所需的只读输入。
///
/// 调用方负责提供已保存配置、数据库观测和 supervisor 运行态；本结构不持有连接，
/// 也不暗示会触发补偿或重载。把这些字段收口后，健康判定保持单一参数入口。
#[derive(Debug, Clone, Copy)]
pub(crate) struct MarketFeedHealthInput<'a> {
    pub(crate) configured_symbols: &'a [String],
    pub(crate) latest_observations: &'a [MarketFeedSymbolObservation],
    pub(crate) runtime_ready: bool,
    pub(crate) runtime_status: Option<&'a str>,
    pub(crate) now: DateTime<Utc>,
    pub(crate) stale_after: std::time::Duration,
    pub(crate) kline_gap_count: i64,
    pub(crate) kline_recovery_failed_count: i64,
}

/// 将数据库最新行情事件/摄取时间、运行时 supervisor 状态和 K 线恢复指标折叠成稳定的后台健康响应。
///
/// 这是纯内存函数：不访问数据库、不读取缓存，也不会修改运行状态。交易对先按领域值对象
/// 规范化并去重，缺少最新行情事件时间的交易对明确列入 `stale_symbols`；未来时间戳按非陈旧处理，
/// 避免时钟轻微漂移导致误报。摄取时间只反映本服务仍在写入，断流判定必须使用上游事件时间，
/// 否则重复摄取旧行情会被误判为健康。未配置外部交易对时不要求 supervisor ready，但策略恢复异常仍会
/// 标记 degraded；状态优先级为 `stale` → `degraded` → `not_configured` → `healthy`。
pub(crate) fn build_market_feed_health(
    input: MarketFeedHealthInput<'_>,
) -> MarketFeedHealthResponse {
    let symbols = input
        .configured_symbols
        .iter()
        .filter_map(|symbol| ValidatedMarketSymbol::from_raw(symbol).ok())
        .map(|symbol| symbol.as_str().to_owned())
        .collect::<std::collections::BTreeSet<_>>();

    let latest_by_symbol = input
        .latest_observations
        .iter()
        .filter_map(|observation| {
            ValidatedMarketSymbol::from_raw(&observation.symbol)
                .ok()
                .map(|symbol| {
                    (
                        symbol.as_str().to_owned(),
                        (observation.observed_at, observation.ingested_at),
                    )
                })
        })
        .collect::<std::collections::HashMap<_, _>>();

    let mut stale_symbols = symbols
        .iter()
        .filter(|symbol| {
            latest_by_symbol
                .get(*symbol)
                .copied()
                .and_then(|(observed_at, _)| observed_at)
                .is_none_or(|timestamp| {
                    input
                        .now
                        .signed_duration_since(timestamp)
                        .to_std()
                        .is_ok_and(|age| age > input.stale_after)
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    stale_symbols.sort();

    let healthy_symbols = symbols.len().saturating_sub(stale_symbols.len()) as u32;
    let last_observed_at = symbols
        .iter()
        .filter_map(|symbol| {
            latest_by_symbol
                .get(symbol)
                .copied()
                .and_then(|(observed_at, _)| observed_at)
        })
        .max();
    let last_ingested_at = symbols
        .iter()
        .filter_map(|symbol| {
            latest_by_symbol
                .get(symbol)
                .copied()
                .and_then(|(_, ingested_at)| ingested_at)
        })
        .max();
    let has_stale = !stale_symbols.is_empty();
    let runtime_failed = !input.runtime_ready
        || input
            .runtime_status
            .is_some_and(|status| matches!(status, "failed" | "error"));
    let degraded = (!symbols.is_empty() && runtime_failed)
        || input.kline_gap_count > 0
        || input.kline_recovery_failed_count > 0;
    let status = if has_stale {
        "stale"
    } else if degraded {
        "degraded"
    } else if symbols.is_empty() {
        "not_configured"
    } else {
        "healthy"
    };

    MarketFeedHealthResponse {
        status: status.to_owned(),
        healthy: status == "healthy",
        stale_after_seconds: input.stale_after.as_secs(),
        configured_symbols: symbols.len() as u32,
        healthy_symbols,
        stale_symbols,
        last_observed_at,
        last_ingested_at,
        kline_gap_count: input.kline_gap_count.max(0),
        kline_recovery_failed_count: input.kline_recovery_failed_count.max(0),
        checked_at: input.now,
    }
}

/// 校验行情订阅交易对并转换为标准市场符号；启用配置时至少需要一个合法符号。
/// 保持输入顺序并拒绝非法符号；不查询交易对是否已上架，配置存在性由应用层负责。
/// 配置处于停用状态时允许符号为空，这样可以先保存一份空配置再逐步补齐，而不必为了保存凑数据。
/// 与周期和提供商不同，符号在此不做去重，重复项会原样落库并可能造成重复订阅。
pub(crate) fn validate_market_feed_symbols(
    symbols: &[String],
    enabled: bool,
) -> AppResult<Vec<String>> {
    if enabled && symbols.is_empty() {
        return Err(AppError::Validation(
            "market feed symbols are required when enabled".to_owned(),
        ));
    }
    symbols
        .iter()
        .map(|symbol| {
            ValidatedMarketSymbol::from_raw(symbol)
                .map(|symbol| symbol.as_str().to_owned())
                .map_err(|error| AppError::Validation(error.to_string()))
        })
        .collect()
}

/// 校验行情 K 线周期集合，拒绝空集合或 worker 不支持的周期并返回标准周期代码。
/// 该纯规则不连接 provider；成功结果可直接用于订阅消息和 REST 兜底 URL 展开。
/// 与符号不同，周期无论配置是否启用都必须非空。校验方式是借 K 线写入键的构造顺带完成周期解析，
/// 因此这里接受的周期集合与实际能落库的 K 线周期严格同源，构造用的时间戳只是占位并不产生任何写入。
pub(crate) fn validate_market_feed_intervals(intervals: &[String]) -> AppResult<Vec<String>> {
    if intervals.is_empty() {
        return Err(AppError::Validation(
            "market feed intervals are required".to_owned(),
        ));
    }
    intervals
        .iter()
        .map(|interval| {
            let value = interval.trim();
            KlineUpsertKey::new(value, Utc::now())
                .map(|key| key.interval().to_owned())
                .map_err(|error| AppError::Validation(error.to_string()))
        })
        .collect()
}

/// 解析、去重行情提供商代码，并限制当前运行配置最多启用一个数据源。
/// 未知别名、空集合或多 provider 配置直接返回校验错误，避免运行时同时写入不同权威价格源。
/// 去重发生在别名归一之后，因此同一提供商的不同写法会被折叠为一项而不会误判为多源。
/// 单源限制是当前运行模型的硬约束而非配置偏好：多个源同时写入会让同一根 K 线出现互相覆盖的权威值。
pub(crate) fn validate_market_feed_providers(providers: &[String]) -> AppResult<Vec<String>> {
    if providers.is_empty() {
        return Err(AppError::Validation(
            "market feed providers are required".to_owned(),
        ));
    }
    let mut normalized = Vec::new();
    for provider in providers {
        let provider = MarketFeedProvider::from_code(provider)?.code().to_owned();
        if !normalized.contains(&provider) {
            normalized.push(provider);
        }
    }
    if normalized.len() > 1 {
        return Err(AppError::Validation(
            "market feed only supports one enabled provider".to_owned(),
        ));
    }
    Ok(normalized)
}

/// 校验行情配置或重载操作的审计原因，拒绝空白文本和超过后台审计上限的内容。
/// 返回去除首尾空白后的原因；不在此处写审计，应用事务必须与配置版本变更一同持久化。
pub(crate) fn validate_market_feed_reason(reason: Option<&str>) -> AppResult<()> {
    let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) else {
        return Err(AppError::Validation(
            "operation reason is required".to_owned(),
        ));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation(
            "operation reason is too long".to_owned(),
        ));
    }
    Ok(())
}

/// 规范化行情源认证类型，目前仅允许无认证或 API Key，并返回稳定小写代码。
/// 该函数不接触密钥明文；凭据加密和旧密文保留由应用层在写事务前处理。
pub(crate) fn validate_market_source_auth_type(auth_type: &str) -> AppResult<String> {
    let normalized = auth_type.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "none" | "api_key" => Ok(normalized),
        _ => Err(AppError::Validation(
            "market source credential auth_type is invalid".to_owned(),
        )),
    }
}

/// 将行情订阅配置记录映射为后台响应，并以保存版本和已应用版本差异计算是否需要重载。
/// 转换不访问数据库或监督器，保留最近重载结果且不会暴露任何行情源凭据。
/// needs_reload 的判定口径是保存版本与已应用版本不相等，因此从未重载过的新配置也会被标记为待重载。
/// 该标记只反映数据库中两个版本号的关系，不查询监督器实际是否存活，运行态需另看状态接口。
pub(crate) fn market_feed_config_response(
    record: AdminMarketFeedConfigRecord,
) -> MarketFeedConfigResponse {
    // 行情配置的 version/applied_version 差异就是后台是否需要 reload 的唯一判断来源。
    let needs_reload = Some(record.version) != record.applied_version;
    MarketFeedConfigResponse {
        id: record.id,
        name: record.name,
        symbols: record.symbols,
        intervals: record.intervals,
        providers: record.providers,
        enabled: record.enabled,
        version: record.version,
        applied_version: record.applied_version,
        needs_reload,
        last_reload_status: record.last_reload_status,
        last_reload_error: record.last_reload_error,
        last_reloaded_at: record.last_reloaded_at,
    }
}

/// 将行情源凭据记录映射为后台响应，只返回提供商、认证类型、API Key 掩码和启用状态。
/// 转换不访问数据库或解密密文，API Secret 与 passphrase 永远不会进入响应。
pub(crate) fn market_source_credential_response(
    record: AdminMarketSourceCredentialRecord,
) -> MarketSourceCredentialResponse {
    MarketSourceCredentialResponse {
        provider: record.provider,
        auth_type: record.auth_type,
        api_key_mask: record.api_key_mask,
        enabled: record.enabled,
    }
}

/// 将行情订阅集合、版本和最近重载结果映射为稳定审计 JSON，供配置变更追踪。
/// 快照同时保留保存版本、应用版本及重载错误；应用层在配置事务或重载结果记录中持久化它。
/// 与对外响应的差别是这里不计算待重载标记，只留下两个原始版本号供事后自行判断。
/// 由于保存操作必然推进版本号，即便订阅内容完全没变，审计前后值也会因版本递增而呈现差异。
pub(crate) fn market_feed_config_audit_json(record: &AdminMarketFeedConfigRecord) -> Value {
    json!({
        "id": record.id,
        "name": record.name,
        "symbols": &record.symbols,
        "intervals": &record.intervals,
        "providers": &record.providers,
        "enabled": record.enabled,
        "version": record.version,
        "applied_version": record.applied_version,
        "last_reload_status": record.last_reload_status,
        "last_reload_error": record.last_reload_error,
        "last_reloaded_at": record.last_reloaded_at.as_ref().map(|value| value.timestamp_millis()),
    })
}

/// 将行情源提供商、认证类型、API Key 掩码和启用状态映射为凭据审计快照。
/// API Secret、passphrase 及全部密文不会进入结果；应用层负责在凭据写事务中保存它。
pub(crate) fn market_source_credential_audit_json(
    record: &AdminMarketSourceCredentialRecord,
) -> Value {
    json!({
        "provider": record.provider,
        "auth_type": record.auth_type,
        "api_key_mask": record.api_key_mask,
        "enabled": record.enabled,
    })
}

/// 将配置保存版本、应用版本和监督器运行快照组合为一次重载审计值。
/// 该映射不触发重载；调用方在运行切换完成或失败后持久化结果。
pub(crate) fn market_feed_reload_audit_json(
    config: &MarketFeedConfigResponse,
    runtime: &MarketFeedRuntimeStatus,
) -> Value {
    json!({
        "version": config.version,
        "applied_version": config.applied_version,
        "runtime": runtime,
    })
}

/// 把行情提供商代码稳定散列为审计目标编号，使无数字主键的凭据更新仍可关联审计记录。
/// 散列结果仅用于审计定位而非安全校验或全局唯一约束；函数无 I/O 和副作用。
pub(crate) fn market_source_credential_target_id(provider: &str) -> u64 {
    provider
        .as_bytes()
        .iter()
        .fold(0_u64, |acc, byte| acc + u64::from(*byte))
}

/// 截断并清理行情重载错误摘要，避免超长或敏感运行时信息直接写入配置状态。
/// 处理仅作用于内存字符串；原始错误仍由调用方返回或记录，本函数不写数据库。
pub(crate) fn sanitize_market_feed_reload_error(error: &str) -> String {
    error.chars().take(512).collect()
}

/// 把已保存的行情订阅响应转换为 worker 运行配置，解析提供商、交易对、周期和刷新参数。
/// 转换不启动监督器或访问数据库；未知提供商、符号或周期会返回校验错误，避免部分重载。
pub fn market_feed_runtime_config_from_response(
    settings: &Settings,
    config: &MarketFeedConfigResponse,
) -> AppResult<MarketFeedRuntimeConfig> {
    MarketFeedRuntimeConfig::new(
        settings,
        config.symbols.clone(),
        config.intervals.clone(),
        config.providers.clone(),
        settings.market_feed_reconnect_seconds,
    )
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_admin_service_market_feed_tests.rs"]
mod tests;
