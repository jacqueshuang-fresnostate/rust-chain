use super::*;

#[tokio::test]
async fn prediction_terminal_journals_match_actual_payout_and_refund_with_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let (role_id, admin_id) = create_prediction_admin(&pool).await;
    let token = issue_token(
        &test_settings(),
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = admin_routes().with_state(AppState::new(test_settings()).with_mysql(pool.clone()));
    for (result, policy, expected_available, expected_legs) in [
        ("yes", "refund_stake_only", "115", 3),
        ("no", "refund_stake_only", "100", 2),
        ("invalid", "refund_stake_only", "110", 0),
        ("invalid", "refund_stake_and_fee", "111", 2),
    ] {
        let suffix = Uuid::now_v7().simple().to_string();
        let user_id =
            sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, 'test-hash')")
                .bind(format!("pj-{suffix}@example.test"))
                .execute(&pool)
                .await?
                .last_insert_id();
        let asset_id = sqlx::query("INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, 'Prediction journal', 8, 'coin', 'active')")
            .bind(format!("PJ{}", &suffix[20..])).execute(&pool).await?.last_insert_id();
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, 100, 10)")
            .bind(user_id).bind(asset_id).execute(&pool).await?;
        let market_id = sqlx::query("INSERT INTO prediction_markets (external_market_id, title, tags_json, yes_price, no_price) VALUES (?, 'Journal settlement', JSON_ARRAY(), 0.5, 0.5)")
            .bind(&suffix).execute(&pool).await?.last_insert_id();
        let order_id = sqlx::query(
            "INSERT INTO prediction_orders (user_id, market_id, quote_id, idempotency_key, outcome, asset_id, stake_amount, fee_amount, accepted_price, shares, theoretical_payout, effective_payout_cap) VALUES (?, ?, ?, ?, 'yes', ?, 10, 1, 0.5, 20, 20, 15)",
        ).bind(user_id).bind(market_id).bind(&suffix).bind(&suffix).bind(asset_id)
            .execute(&pool).await?.last_insert_id();
        for replay in [false, true] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/prediction/markets/{market_id}/settle"))
                        .header("authorization", format!("Bearer {token}"))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"result":result,"invalid_refund_policy":policy}).to_string(),
                        ))?,
                )
                .await?;
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
            assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
            let payload: Value = serde_json::from_slice(&body)?;
            assert_eq!(payload["changed"], !replay);
            let wallet: (BigDecimal, BigDecimal) = sqlx::query_as(
                "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
            )
            .bind(user_id)
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
            assert_eq!(wallet, (decimal(expected_available), decimal("0")));
            let journal: (i64, BigDecimal) = sqlx::query_as(
                "SELECT COUNT(*), COALESCE(SUM(amount), 0) FROM platform_financial_journal WHERE context = 'prediction' AND ref_id = ?",
            ).bind(order_id.to_string()).fetch_one(&pool).await?;
            assert_eq!(journal, (expected_legs, decimal("0")));
            let missing_historical_fee: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM platform_financial_journal WHERE transaction_key = ?",
            )
            .bind(format!("prediction:{order_id}:fee"))
            .fetch_one(&pool)
            .await?;
            assert_eq!(
                missing_historical_fee, 0,
                "legacy opening fee must not be fabricated"
            );
        }
        sqlx::query(
            "DELETE FROM platform_financial_journal WHERE context = 'prediction' AND ref_id = ?",
        )
        .bind(order_id.to_string())
        .execute(&pool)
        .await?;
        sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ?")
            .bind(user_id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM prediction_orders WHERE id = ?")
            .bind(order_id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM prediction_markets WHERE id = ?")
            .bind(market_id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ?")
            .bind(user_id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    Ok(())
}
