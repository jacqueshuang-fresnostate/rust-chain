use super::*;

#[derive(Debug)]
pub(crate) struct AdminRiskRuleListFilter {
    pub(crate) rule_type: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminRiskEventListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) decision: Option<String>,
    pub(crate) risk_level: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

pub(crate) async fn save_admin_security_policy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    policy: &UserSecurityPolicy,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value, updated_by)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE
               policy_value = VALUES(policy_value),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(USER_SECURITY_POLICY_KEY)
    .bind(SqlxJson(policy.clone()))
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn list_admin_risk_rules(
    pool: &Pool<MySql>,
    filter: AdminRiskRuleListFilter,
) -> AppResult<(Vec<RiskRuleResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, rule_type, target_type, target_id, config_json, enabled,
                  created_by, created_at, updated_at
           FROM risk_rules"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM risk_rules");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(rule_type) = filter.rule_type.clone() {
            builder.push(" AND rule_type = ");
            builder.push_bind(rule_type);
        }
        if let Some(target_type) = filter.target_type.clone() {
            builder.push(" AND target_type = ");
            builder.push_bind(target_type);
        }
        if let Some(enabled) = filter.enabled {
            builder.push(" AND enabled = ");
            builder.push_bind(enabled);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

pub(crate) async fn insert_risk_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule: RiskRuleWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO risk_rules (rule_type, target_type, target_id, config_json, enabled, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(rule.rule_type)
    .bind(rule.target_type)
    .bind(rule.target_id)
    .bind(SqlxJson(rule.config_json))
    .bind(rule.enabled)
    .bind(rule.created_by)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn load_risk_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<RiskRuleResponse> {
    sqlx::query_as::<_, RiskRuleResponse>(
        r#"SELECT id, rule_type, target_type, target_id, config_json, enabled,
                  created_by, created_at, updated_at
           FROM risk_rules
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_risk_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<RiskRuleResponse> {
    sqlx::query_as::<_, RiskRuleResponse>(
        r#"SELECT id, rule_type, target_type, target_id, config_json, enabled,
                  created_by, created_at, updated_at
           FROM risk_rules
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn update_risk_rule_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
    enabled: bool,
) -> AppResult<()> {
    sqlx::query("UPDATE risk_rules SET enabled = ? WHERE id = ?")
        .bind(enabled)
        .bind(rule_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn list_admin_risk_events(
    pool: &Pool<MySql>,
    filter: AdminRiskEventListFilter,
) -> AppResult<(Vec<RiskEventResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, actor_type, actor_id, event_type, risk_level,
                  decision, reason, payload_json, created_at
           FROM risk_events"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM risk_events");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "user_id", user_id);
        }
        push_user_email_filter(builder, "user_id", filter.email.clone());
        if let Some(decision) = filter.decision.clone() {
            builder.push(" AND decision = ");
            builder.push_bind(decision);
        }
        if let Some(risk_level) = filter.risk_level.clone() {
            builder.push(" AND risk_level = ");
            builder.push_bind(risk_level);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY created_at DESC, id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}
