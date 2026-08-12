use super::*;

pub(crate) const DEFAULT_MARKET_FEED_CONFIG_NAME: &str = "default";

pub(crate) const MARKET_SOURCE_AUTH_TYPE_API_KEY: &str = "api_key";

pub(crate) const MARKET_SOURCE_AUTH_TYPE_NONE: &str = "none";

/// 统一读取市场推送运行时状态，避免 routes 层重复处理空 supervisor。
pub(crate) async fn load_market_feed_runtime(state: &AppState) -> MarketFeedRuntimeStatus {
    match &state.market_feed_supervisor {
        Some(supervisor) => supervisor.status().await,
        None => Default::default(),
    }
}

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

pub(crate) fn validate_market_source_auth_type(auth_type: &str) -> AppResult<String> {
    let normalized = auth_type.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "none" | "api_key" => Ok(normalized),
        _ => Err(AppError::Validation(
            "market source credential auth_type is invalid".to_owned(),
        )),
    }
}

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

pub(crate) fn market_source_credential_target_id(provider: &str) -> u64 {
    provider
        .as_bytes()
        .iter()
        .fold(0_u64, |acc, byte| acc + u64::from(*byte))
}

pub(crate) fn sanitize_market_feed_reload_error(error: &str) -> String {
    error.chars().take(512).collect()
}

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
