use super::*;
use serde_json::json;

async fn post(
    app: &axum::Router,
    token: &str,
    path: &str,
    body: Value,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))?,
        )
        .await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576).await?;
    let payload = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw": String::from_utf8_lossy(&bytes)}));
    Ok((status, payload))
}

#[tokio::test]
async fn margin_numeric_input_envelopes_partial_settlement_dust_and_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    for mode in ["isolated", "cross"] {
        let settings = test_settings();
        let mut tx = pool.begin().await?;
        let user_id = create_user(&mut tx).await;
        let (base, base_symbol) = create_asset(&mut tx, "NMB").await;
        let (asset, quote_symbol) = create_asset(&mut tx, "NMQ").await;
        let symbol = format!("{base_symbol}-{quote_symbol}");
        let pair = create_pair(&mut tx, base, asset, &symbol).await;
        let product = seed_margin_product_with_mode(&mut tx, pair, asset, mode, vec!["3"]).await;
        sqlx::query("UPDATE assets SET precision_scale = 8 WHERE id = ?")
            .bind(asset)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE margin_products SET min_margin = 1, max_margin = NULL WHERE id = ?")
            .bind(product)
            .execute(&mut *tx)
            .await?;
        let wallet_table = if mode == "cross" {
            "margin_wallet_accounts"
        } else {
            "wallet_accounts"
        };
        let initial = decimal("100.000000000000000007");
        sqlx::query(&format!("INSERT INTO {wallet_table}(user_id,asset_id,available,frozen,locked) VALUES (?,?,?,0.000000000000000002,0.000000000000000003)"))
            .bind(user_id).bind(asset).bind(&initial).execute(&mut *tx).await?;
        tx.commit().await?;
        let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
        let app = user_routes().with_state(
            AppState::new(settings)
                .with_mysql(pool.clone())
                .with_redis(redis.clone()),
        );
        cache_margin_ticker(&redis, &symbol, "3").await?;
        for amount in [
            "1.0000000000000000001",
            "1.000000001",
            "100000000000000000000",
            "99999999999999999999",
            "1e100000",
        ] {
            let (status, payload) = post(
                &app,
                &token,
                "/margin/positions",
                json!({
                    "product_id": product, "direction":"long", "margin_mode":mode,
                    "margin_amount":amount, "leverage":"3",
                    "idempotency_key":format!("invalid-{amount}"),
                }),
            )
            .await?;
            assert!(status.is_client_error(), "{amount}: {status} {payload}");
        }
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM margin_positions WHERE user_id = ?")
                .bind(user_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0);
        let before: BigDecimal = sqlx::query_scalar(&format!(
            "SELECT available FROM {wallet_table} WHERE user_id=? AND asset_id=?"
        ))
        .bind(user_id)
        .bind(asset)
        .fetch_one(&pool)
        .await?;
        assert_eq!(before, initial);

        let (status, opened) = post(
            &app,
            &token,
            "/margin/positions",
            json!({
                "product_id":product, "direction":"long", "margin_mode":mode,
                "margin_amount":"10.0000000100000000000", "leverage":"3",
                "idempotency_key":"numeric-open",
            }),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{opened}");
        let position_id = opened["position"]["id"].as_u64().unwrap();
        assert_eq!(
            decimal(opened["position"]["notional_amount"].as_str().unwrap()),
            decimal("30.00000003")
        );
        sqlx::query("UPDATE margin_positions SET interest_amount = 0.010000000000000001, interest_remainder=0.0000000000000000005 WHERE id=?")
            .bind(position_id).execute(&pool).await?;

        // An unrepresentable calculated PnL must roll back the execution receipt and wallet.
        cache_margin_ticker(&redis, &symbol, "99999999999999999999").await?;
        let path = format!("/margin/positions/{position_id}/close");
        let (status, _) = post(
            &app,
            &token,
            &path,
            json!({"percentage":100,"idempotency_key":"overflow-close"}),
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM margin_position_close_executions WHERE position_id=?",
        )
        .bind(position_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(count, 0);

        cache_margin_ticker(&redis, &symbol, "4").await?;
        let maximum = decimal("99999999999999999999.999999999999999999");
        sqlx::query(&format!(
            "UPDATE {wallet_table} SET available=? WHERE user_id=? AND asset_id=?"
        ))
        .bind(&maximum)
        .bind(user_id)
        .bind(asset)
        .execute(&pool)
        .await?;
        let (status, _) = post(
            &app,
            &token,
            &path,
            json!({
                "percentage":37,"idempotency_key":"overflow-wallet"
            }),
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let executions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM margin_position_close_executions WHERE position_id=?",
        )
        .bind(position_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(executions, 0);
        let untouched: BigDecimal = sqlx::query_scalar(&format!(
            "SELECT available FROM {wallet_table} WHERE user_id=? AND asset_id=?"
        ))
        .bind(user_id)
        .bind(asset)
        .fetch_one(&pool)
        .await?;
        assert_eq!(untouched, maximum);
        sqlx::query(&format!(
            "UPDATE {wallet_table} SET available=? WHERE user_id=? AND asset_id=?"
        ))
        .bind(&initial - decimal("10.00000001"))
        .bind(user_id)
        .bind(asset)
        .execute(&pool)
        .await?;
        let body = json!({"percentage":37,"idempotency_key":"numeric-partial"});
        let (status, partial) = post(&app, &token, &path, body.clone()).await?;
        assert_eq!(status, StatusCode::OK, "{partial}");
        let execution = &partial["execution"];
        assert_eq!(
            decimal(execution["close_margin_amount"].as_str().unwrap()),
            decimal("3.7")
        );
        assert_eq!(
            decimal(execution["realized_pnl"].as_str().unwrap()),
            decimal("3.7")
        );
        assert_eq!(
            decimal(execution["settlement_amount"].as_str().unwrap()),
            decimal("7.3963")
        );
        let (status, replay) = post(&app, &token, &path, body).await?;
        assert_eq!(status, StatusCode::OK, "{replay}");
        assert_eq!(replay["execution"], *execution);
        let carry: BigDecimal =
            sqlx::query_scalar("SELECT interest_remainder FROM margin_positions WHERE id=?")
                .bind(position_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(carry, decimal("0.0000000000000000005"));

        let (status, closed) = post(
            &app,
            &token,
            &path,
            json!({"percentage":100,"idempotency_key":"numeric-final"}),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{closed}");
        assert_eq!(
            decimal(
                closed["execution"]["close_interest_amount"]
                    .as_str()
                    .unwrap()
            ),
            decimal("0.006300000000000001")
        );
        assert_eq!(
            decimal(closed["execution"]["realized_pnl"].as_str().unwrap()),
            decimal("6.3")
        );
        let wallet: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(&format!(
            "SELECT available,frozen,locked FROM {wallet_table} WHERE user_id=? AND asset_id=?"
        ))
        .bind(user_id)
        .bind(asset)
        .fetch_one(&pool)
        .await?;
        assert_eq!(
            wallet.0,
            initial + decimal("10") - decimal("0.010000000000000001")
        );
        assert_eq!(wallet.1, decimal("0.000000000000000002"));
        assert_eq!(wallet.2, decimal("0.000000000000000003"));
        let journals: (i64, BigDecimal) = sqlx::query_as(
            "SELECT COUNT(DISTINCT transaction_key),COALESCE(SUM(amount),0) FROM platform_financial_journal WHERE asset_id=?")
            .bind(asset).fetch_one(&pool).await?;
        assert_eq!(journals.0, 3);
        assert_eq!(journals.1, decimal("0"));

        // The close path must checkpoint elapsed interest before shrinking principal,
        // even when the accrued increment is smaller than one asset unit.
        sqlx::query("UPDATE margin_products SET min_margin=0.00000001 WHERE id=?")
            .bind(product)
            .execute(&pool)
            .await?;
        let (status, tiny) = post(
            &app,
            &token,
            "/margin/positions",
            json!({
                "product_id":product,"direction":"long","margin_mode":mode,
                "margin_amount":"0.00000002","leverage":"3","idempotency_key":"numeric-carry-open",
            }),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{tiny}");
        let tiny_id = tiny["position"]["id"].as_u64().unwrap();
        let from = Utc::now() - chrono::TimeDelta::hours(2);
        sqlx::query("UPDATE margin_positions SET hourly_interest_rate=0.0375,interest_amount=0.000000000000000001,interest_accrued_at=? WHERE id=?")
            .bind(from.naive_utc()).bind(tiny_id).execute(&pool).await?;
        let (status, tiny_close) = post(
            &app,
            &token,
            &format!("/margin/positions/{tiny_id}/close"),
            json!({
                "percentage":50,"idempotency_key":"numeric-carry-partial",
            }),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{tiny_close}");
        let remaining: (BigDecimal, BigDecimal, BigDecimal, chrono::DateTime<Utc>) = sqlx::query_as(
            "SELECT borrowed_amount,interest_amount,interest_remainder,interest_accrued_at FROM margin_positions WHERE id=?")
            .bind(tiny_id).fetch_one(&pool).await?;
        assert_eq!(remaining.0, decimal("0.00000002"));
        assert_eq!(remaining.1, decimal("0.000000000000000001"));
        assert_eq!(remaining.2, decimal("0.000000003"));
        assert_eq!(
            remaining.3.timestamp_micros(),
            (from + chrono::TimeDelta::hours(2)).timestamp_micros(),
        );
    }
    Ok(())
}
