//! 管理端权限解析与路由权限码映射。

use crate::{
    error::{AppError, AppResult},
    modules::admin::{domain::AdminScope, repository::AdminAccessRecord},
};
use serde_json::Value;
use std::collections::BTreeSet;

const ADMIN_PERMISSION_RESOURCES: &[&str] = &[
    "account.security",
    "admin.accounts",
    "agents",
    "agents.commission_rules",
    "agents.commissions",
    "audit.logs",
    "config_center",
    "content.news",
    "convert.orders",
    "convert.pairs",
    "dashboard",
    "earn.categories",
    "earn.products",
    "earn.subscriptions",
    "governance.changes",
    "governance.roles",
    "loan.orders",
    "loan.products",
    "margin.interest",
    "margin.liquidations",
    "margin.positions",
    "margin.products",
    "market.feed",
    "market.pairs",
    "market.strategies",
    "new_coin.distributions",
    "new_coin.locks",
    "new_coin.projects",
    "new_coin.purchases",
    "new_coin.subscriptions",
    "new_coin.unlocks",
    "prediction.assets",
    "prediction.markets",
    "prediction.orders",
    "prediction.settings",
    "prediction.sync",
    "risk.events",
    "risk.rules",
    "seconds.orders",
    "seconds.products",
    "spot.orders",
    "spot.trades",
    "support.conversations",
    "system.brand",
    "system.countries",
    "system.events",
    "system.security",
    "system.smtp",
    "system.uploads",
    "users",
    "users.kyc",
    "wallet.accounts",
    "wallet.address_pool",
    "wallet.assets",
    "wallet.deposits",
    "wallet.ledger",
    "wallet.networks",
    "wallet.quick_recharge",
    "wallet.withdrawals",
];

/// 把数据库权限 JSON 解析为管理员请求快照。
/// 同时兼容字符串数组与 `{ "permission": true }` 对象，便于存量角色渐进迁移；
/// 其他 JSON 形状或非字符串数组元素一律视为存储污染并返回内部错误，绝不回落为全权限。
pub(crate) fn admin_scope_from_record(record: AdminAccessRecord) -> AppResult<AdminScope> {
    let permissions = parse_permissions(record.permissions)?;
    Ok(AdminScope {
        admin_id: record.admin_id,
        username: record.username,
        must_change_password: record.must_change_password,
        auth_session_version: record.auth_session_version,
        role_id: record.role_id,
        role_name: record.role_name,
        permissions,
    })
}

/// 根据 HTTP 方法与后台路径返回必须具备的稳定权限码。
/// 登录、刷新、个人 2FA 和当前身份查询只需有效管理员身份，因此返回 `None`；
/// 未登记的后台业务路径返回 `admin.unmapped`，仅 `*` 可访问，避免新路由因遗漏映射而默认放行。
pub(crate) fn required_admin_permission(method: &str, raw_path: &str) -> Option<String> {
    let path = raw_path.strip_prefix("/admin/api/v1").unwrap_or(raw_path);
    if path.starts_with("/auth/") || path == "/auth" || path == "/access/me" {
        return None;
    }

    let resource = permission_resource(path).unwrap_or("admin.unmapped");
    let action = if matches!(method, "GET" | "HEAD" | "OPTIONS") {
        "read"
    } else {
        operational_action(path).unwrap_or("write")
    };
    Some(format!("{resource}.{action}"))
}

/// 返回可分配给角色的稳定权限码全集。
/// 每个资源统一暴露读取、写入、复核、运行操作和结算五种动作；没有对应路由的动作不会产生额外能力，
/// 但统一动作集合让角色编辑器不必从 HTTP 路径猜测字符串。返回顺序固定，便于缓存和差异审计。
pub(crate) fn admin_permission_catalog() -> Vec<String> {
    let mut permissions = BTreeSet::from(["*".to_owned()]);
    for resource in ADMIN_PERMISSION_RESOURCES {
        for action in ["read", "write", "review", "operate", "settle"] {
            permissions.insert(format!("{resource}.{action}"));
        }
    }
    permissions.into_iter().collect()
}

fn parse_permissions(value: Value) -> AppResult<BTreeSet<String>> {
    let mut permissions = BTreeSet::new();
    match value {
        Value::Array(values) => {
            for value in values {
                let Value::String(permission) = value else {
                    return Err(invalid_permissions());
                };
                insert_permission(&mut permissions, permission)?;
            }
        }
        Value::Object(values) => {
            for (permission, enabled) in values {
                if enabled == Value::Bool(true) {
                    insert_permission(&mut permissions, permission)?;
                } else if enabled != Value::Bool(false) {
                    return Err(invalid_permissions());
                }
            }
        }
        _ => return Err(invalid_permissions()),
    }
    Ok(permissions)
}

fn insert_permission(permissions: &mut BTreeSet<String>, permission: String) -> AppResult<()> {
    let permission = permission.trim();
    if permission.is_empty()
        || permission.len() > 128
        || !permission.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'*')
        })
    {
        return Err(invalid_permissions());
    }
    permissions.insert(permission.to_owned());
    Ok(())
}

fn invalid_permissions() -> AppError {
    AppError::Internal("admin role permissions contain an invalid value".to_owned())
}

fn permission_resource(path: &str) -> Option<&'static str> {
    let mappings = [
        ("/prediction/sync", "prediction.sync"),
        ("/prediction/settings", "prediction.settings"),
        ("/prediction/asset-configs", "prediction.assets"),
        ("/prediction/markets", "prediction.markets"),
        ("/prediction/orders", "prediction.orders"),
        ("/access/permissions", "governance.roles"),
        ("/config-center", "config_center"),
        ("/config-change-requests", "governance.changes"),
        ("/seconds-contracts/products", "seconds.products"),
        ("/seconds-contracts/orders", "seconds.orders"),
        ("/wallet/withdrawals", "wallet.withdrawals"),
        ("/wallet/deposits", "wallet.deposits"),
        ("/wallet/accounts", "wallet.accounts"),
        ("/wallet/ledger", "wallet.ledger"),
        ("/wallet/deposit-network-configs", "wallet.networks"),
        ("/wallet/deposit-address-pool", "wallet.address_pool"),
        ("/wallet/quick-recharge", "wallet.quick_recharge"),
        ("/deposit-network-configs", "wallet.networks"),
        ("/deposit-address-pool", "wallet.address_pool"),
        ("/quick-recharge", "wallet.quick_recharge"),
        ("/loan/products", "loan.products"),
        ("/loan/orders", "loan.orders"),
        ("/margin/products", "margin.products"),
        ("/margin/positions", "margin.positions"),
        ("/margin/liquidations", "margin.liquidations"),
        ("/margin/interest", "margin.interest"),
        ("/earn/categories", "earn.categories"),
        ("/earn/products", "earn.products"),
        ("/earn/subscriptions", "earn.subscriptions"),
        ("/spot/orders", "spot.orders"),
        ("/spot/trades", "spot.trades"),
        ("/spot/fills", "spot.orders"),
        ("/support/conversations", "support.conversations"),
        ("/market/strategies", "market.strategies"),
        ("/market/feed", "market.feed"),
        ("/market/pairs", "market.pairs"),
        ("/market-strategies", "market.strategies"),
        ("/market-feed", "market.feed"),
        ("/market-pairs", "market.pairs"),
        ("/trading-pairs", "market.pairs"),
        ("/new-coins/projects", "new_coin.projects"),
        ("/new-coins/subscriptions", "new_coin.subscriptions"),
        ("/new-coins/distributions", "new_coin.distributions"),
        ("/new-coins/purchases", "new_coin.purchases"),
        ("/new-coins/lock-positions", "new_coin.locks"),
        ("/new-coins/unlocks", "new_coin.unlocks"),
        ("/new-coins", "new_coin.projects"),
        ("/convert/pairs", "convert.pairs"),
        ("/convert/orders", "convert.orders"),
        ("/users/kyc", "users.kyc"),
        ("/kyc", "users.kyc"),
        ("/users", "users"),
        ("/agents", "agents"),
        ("/agent-commissions", "agents.commissions"),
        ("/agent-commission-rules", "agents.commission_rules"),
        ("/assets", "wallet.assets"),
        ("/news", "content.news"),
        ("/risk/events", "risk.events"),
        ("/risk", "risk.rules"),
        ("/countries", "system.countries"),
        ("/security-policy", "system.security"),
        ("/platform-brand", "system.brand"),
        ("/platform/brand", "system.brand"),
        ("/smtp", "system.smtp"),
        ("/upload/config", "system.uploads"),
        ("/uploads", "system.uploads"),
        ("/audit-logs", "audit.logs"),
        ("/dashboard", "dashboard"),
        ("/events", "system.events"),
    ];

    mappings
        .iter()
        .find_map(|(prefix, resource)| path.starts_with(prefix).then_some(*resource))
}

fn operational_action(path: &str) -> Option<&'static str> {
    if path.contains("/approve")
        || path.contains("/reject")
        || path.contains("/review")
        || path.contains("/confirm")
        || path.contains("/fail")
    {
        return Some("review");
    }
    if path.ends_with("/settle") {
        return Some("settle");
    }
    if path.contains("/reload")
        || path.contains("/restore")
        || path.contains("/recovery")
        || path.ends_with("/sync")
        || path.contains("/publish")
        || path.contains("/requeue")
        || path.ends_with("/apply")
    {
        return Some("operate");
    }
    None
}
