use bigdecimal::BigDecimal;
use sqlx::MySqlPool;
use std::str::FromStr;
use uuid::Uuid;

pub struct AgentCommissionFixture {
    pub agent_id: u64,
    agent_user_id: u64,
    rule_id: u64,
}

pub async fn seed_direct_agent_commission(
    pool: &MySqlPool,
    referred_user_id: u64,
    product_type: &str,
    commission_rate: &str,
) -> Result<AgentCommissionFixture, sqlx::Error> {
    let suffix = Uuid::now_v7().simple().to_string();
    let agent_user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("commission-agent-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(pool)
        .await?
        .last_insert_id();
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code, path) VALUES (?, ?, '')")
        .bind(agent_user_id)
        .bind(format!("commission-agent-{suffix}"))
        .execute(pool)
        .await?
        .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}"))
        .bind(agent_id)
        .execute(pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'agent', ?, 1, ?)"#,
    )
    .bind(referred_user_id)
    .bind(agent_id)
    .bind(agent_id)
    .bind(format!("/{agent_id}/{referred_user_id}"))
    .execute(pool)
    .await?;
    let rule_id = sqlx::query(
        r#"INSERT INTO agent_commission_rules
           (agent_id, product_type, commission_rate, status)
           VALUES (?, ?, ?, 'active')"#,
    )
    .bind(agent_id)
    .bind(product_type)
    .bind(BigDecimal::from_str(commission_rate).expect("valid commission rate"))
    .execute(pool)
    .await?
    .last_insert_id();

    Ok(AgentCommissionFixture {
        agent_id,
        agent_user_id,
        rule_id,
    })
}

pub async fn cleanup_direct_agent_commission(
    pool: &MySqlPool,
    referred_user_id: u64,
    fixture: AgentCommissionFixture,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM agent_commission_records WHERE user_id = ?")
        .bind(referred_user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM agent_commission_rules WHERE id = ?")
        .bind(fixture.rule_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
        .bind(referred_user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM agents WHERE id = ?")
        .bind(fixture.agent_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(fixture.agent_user_id)
        .execute(pool)
        .await?;
    Ok(())
}
