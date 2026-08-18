//! 配置中心 MySQL 只读聚合适配器。

use crate::{error::AppResult, modules::admin::repository::AdminConfigCenterFactRecord};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool};

#[derive(Debug, sqlx::FromRow)]
struct AdminConfigCenterFactRow {
    code: String,
    configured_count: u64,
    pending_apply_count: u64,
    published_version: Option<u64>,
    applied_version: Option<u64>,
    runtime_status: String,
    last_modified_at: Option<DateTime<Utc>>,
    last_applied_at: Option<DateTime<Utc>>,
    last_tested_at: Option<DateTime<Utc>>,
    recent_error: Option<String>,
}

/// 配置中心权威事实查询。
/// 十三个分支只选择计数、版本、状态和时间；错误列先在数据库侧限制到 512 字符，所有凭据密文、掩码和配置 JSON 均不进入结果。
pub(crate) const ADMIN_CONFIG_CENTER_FACTS_SQL: &str = r#"
SELECT 'prediction_settings' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(MAX(revision) AS UNSIGNED) AS published_version,
       CAST(MAX(revision) AS UNSIGNED) AS applied_version,
       CASE
           WHEN COUNT(*) = 0 THEN 'unknown'
           WHEN MAX(sync_enabled) = FALSE THEN 'stopped'
           WHEN COALESCE(SUM(last_sync_status IN ('failed', 'error')), 0) > 0 THEN 'error'
           WHEN COALESCE(SUM(last_sync_status = 'running'), 0) > 0 THEN 'running'
           WHEN COALESCE(SUM(last_sync_status = 'success'), 0) > 0 THEN 'healthy'
           ELSE 'unknown'
       END AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CASE
           WHEN COALESCE(SUM(last_sync_status IN ('failed', 'error')), 0) > 0
           THEN LEFT(MAX(last_sync_error), 512)
           ELSE NULL
       END AS recent_error
FROM prediction_settings
WHERE id = 1

UNION ALL

SELECT 'market_feed' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(COALESCE(SUM(
           CASE
               WHEN applied_version IS NULL OR applied_version <> version THEN 1
               ELSE 0
           END
       ), 0) AS UNSIGNED) AS pending_apply_count,
       CAST(MAX(version) AS UNSIGNED) AS published_version,
       CAST(MAX(applied_version) AS UNSIGNED) AS applied_version,
       CASE
           WHEN COUNT(*) = 0 THEN 'unknown'
           WHEN COALESCE(SUM(last_reload_status = 'failed'), 0) > 0 THEN 'error'
           WHEN MAX(enabled) = FALSE THEN 'stopped'
           WHEN COALESCE(SUM(last_reload_status IN ('success', 'skipped')), 0) > 0 THEN 'healthy'
           ELSE 'unknown'
       END AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(last_reloaded_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CASE
           WHEN COALESCE(SUM(last_reload_status = 'failed'), 0) > 0
           THEN LEFT(MAX(last_reload_error), 512)
           ELSE NULL
       END AS recent_error
FROM market_feed_configs
WHERE name = 'default'

UNION ALL

SELECT 'market_strategy' AS code,
       CAST(COUNT(strategies.id) AS UNSIGNED) AS configured_count,
       CAST(COALESCE(SUM(
           CASE
               WHEN strategies.id IS NOT NULL
                    AND (latest.published_version IS NULL
                         OR runs.active_version IS NULL
                         OR latest.published_version <> runs.active_version)
               THEN 1
               ELSE 0
           END
       ), 0) AS UNSIGNED) AS pending_apply_count,
       CAST(MAX(latest.published_version) AS UNSIGNED) AS published_version,
       CAST(MAX(runs.active_version) AS UNSIGNED) AS applied_version,
       CASE
           WHEN COUNT(strategies.id) = 0 THEN 'unknown'
           WHEN COALESCE(SUM(
               runs.run_status IN ('failed', 'error')
               OR runs.recovery_status = 'failed'
               OR NULLIF(TRIM(runs.error_message), '') IS NOT NULL
           ), 0) > 0 THEN 'error'
           WHEN COALESCE(SUM(runs.run_status = 'running'), 0) > 0 THEN 'running'
           ELSE 'stopped'
       END AS runtime_status,
       MAX(latest.last_modified_at) AS last_modified_at,
       MAX(applied.created_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       (
           SELECT LEFT(strategy_errors.error_message, 512)
           FROM strategy_runs AS strategy_errors
           WHERE NULLIF(TRIM(strategy_errors.error_message), '') IS NOT NULL
           ORDER BY strategy_errors.updated_at DESC, strategy_errors.strategy_id DESC
           LIMIT 1
       ) AS recent_error
FROM market_strategies AS strategies
LEFT JOIN strategy_runs AS runs ON runs.strategy_id = strategies.id
LEFT JOIN (
    SELECT versions.strategy_id,
           MAX(versions.version) AS published_version,
           MAX(versions.created_at) AS last_modified_at
    FROM strategy_versions AS versions
    GROUP BY versions.strategy_id
) AS latest ON latest.strategy_id = strategies.id
LEFT JOIN strategy_versions AS applied
       ON applied.strategy_id = runs.strategy_id
      AND applied.version = runs.active_version

UNION ALL

SELECT 'kyc_rules' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM kyc_configs
WHERE name = 'default'

UNION ALL

SELECT 'security_policy' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM security_policy_configs
WHERE policy_key = 'user_security_policy'

UNION ALL

SELECT 'country_configs' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM country_configs

UNION ALL

SELECT 'loan_products' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(MAX(revision) AS UNSIGNED) AS published_version,
       CAST(MAX(revision) AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM loan_products

UNION ALL

SELECT 'margin_products' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM margin_products

UNION ALL

SELECT 'seconds_contract_products' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM seconds_contract_products

UNION ALL

SELECT 'earn_products' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM earn_products

UNION ALL

SELECT 'smtp' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       CASE
           WHEN COUNT(*) = 0 THEN 'unknown'
           WHEN MAX(enabled) = FALSE THEN 'stopped'
           WHEN EXISTS(
               SELECT 1
               FROM admin_audit_logs AS smtp_test_logs
               WHERE smtp_test_logs.action = 'smtp_config.test'
           ) THEN 'healthy'
           ELSE 'unknown'
       END AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       (
           SELECT MAX(smtp_test_logs.created_at)
           FROM admin_audit_logs AS smtp_test_logs
           WHERE smtp_test_logs.action = 'smtp_config.test'
       ) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM smtp_configs

UNION ALL

SELECT 'upload_storage' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       CASE
           WHEN COUNT(*) = 0 THEN 'unknown'
           WHEN MAX(enabled) = FALSE THEN 'stopped'
           ELSE 'unknown'
       END AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM upload_storage_configs
WHERE name = 'default'

UNION ALL

SELECT 'platform_brand' AS code,
       CAST(COUNT(*) AS UNSIGNED) AS configured_count,
       CAST(0 AS UNSIGNED) AS pending_apply_count,
       CAST(NULL AS UNSIGNED) AS published_version,
       CAST(NULL AS UNSIGNED) AS applied_version,
       'not_applicable' AS runtime_status,
       MAX(updated_at) AS last_modified_at,
       MAX(updated_at) AS last_applied_at,
       CAST(NULL AS DATETIME(6)) AS last_tested_at,
       CAST(NULL AS CHAR(512)) AS recent_error
FROM platform_brand_configs
WHERE name = 'default'
"#;

/// 在单条一致性读语句中聚合十三个配置域的权威事实。
/// 查询不启动事务、不加行锁且无写入副作用；任一表或字段不匹配当前迁移合同都会整体返回数据库错误，避免输出部分陈旧状态。
pub(crate) async fn load_admin_config_center_facts(
    pool: &Pool<MySql>,
) -> AppResult<Vec<AdminConfigCenterFactRecord>> {
    let rows = sqlx::query_as::<_, AdminConfigCenterFactRow>(ADMIN_CONFIG_CENTER_FACTS_SQL)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| AdminConfigCenterFactRecord {
            code: row.code,
            configured_count: row.configured_count,
            pending_apply_count: row.pending_apply_count,
            published_version: row.published_version,
            applied_version: row.applied_version,
            runtime_status: row.runtime_status,
            last_modified_at: row.last_modified_at,
            last_applied_at: row.last_applied_at,
            last_tested_at: row.last_tested_at,
            recent_error: row.recent_error,
        })
        .collect())
}
