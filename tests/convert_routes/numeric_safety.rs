use super::*;

#[tokio::test]
async fn numeric_safety_convert_preserves_existing_dust_and_rolls_back_overflow()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    for (opening_balance, should_succeed) in [("0.001", true), ("99999999999999999999", false)] {
        let settings = test_settings();
        let user_id = create_user(&pool).await;
        let from_asset = create_asset(&pool, "NCS").await;
        let to_asset = create_asset_with_precision(&pool, "NCT", 2).await;
        let pair_id = seed_convert_pair(&pool, from_asset, to_asset).await;
        let quote_id = seed_convert_quote(&pool, user_id, pair_id, from_asset, to_asset).await;
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 10), (?, ?, ?)")
            .bind(user_id).bind(from_asset).bind(user_id).bind(to_asset)
            .bind(decimal(opening_balance)).execute(&pool).await?;
        let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
        let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/convert/confirm")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"quote_id":quote_id}).to_string()))?,
            )
            .await?;
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 8192).await?;
        assert_eq!(
            status,
            if should_succeed {
                StatusCode::OK
            } else {
                StatusCode::BAD_REQUEST
            },
            "{}",
            String::from_utf8_lossy(&body)
        );
        let balances: Vec<(u64, BigDecimal)> = sqlx::query_as(
            "SELECT asset_id, available FROM wallet_accounts WHERE user_id = ? ORDER BY asset_id",
        )
        .bind(user_id)
        .fetch_all(&pool)
        .await?;
        let find = |asset| {
            balances
                .iter()
                .find(|row| row.0 == asset)
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(
            find(from_asset),
            decimal(if should_succeed { "0" } else { "10" })
        );
        assert_eq!(
            find(to_asset),
            decimal(opening_balance)
                + if should_succeed {
                    decimal("20")
                } else {
                    decimal("0")
                }
        );
        let order_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM convert_orders WHERE quote_id = ?")
                .bind(&quote_id)
                .fetch_one(&pool)
                .await?;
        let ledger_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type='convert_order' AND ref_id = ?",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await?;
        let journal_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM platform_financial_journal WHERE context='convert' AND ref_id = ?",
        ).bind(&quote_id).fetch_one(&pool).await?;
        if should_succeed {
            assert_eq!((order_count, ledger_count, journal_count), (1, 2, 4));
            let (amount, after): (BigDecimal, BigDecimal) = sqlx::query_as(
                "SELECT amount, balance_after FROM wallet_ledger WHERE ref_type='convert_order' AND ref_id = ? AND asset_id = ?",
            ).bind(&quote_id).bind(to_asset).fetch_one(&pool).await?;
            assert_eq!(amount, decimal("20"));
            assert_eq!(after, decimal("20.001"));
        } else {
            assert_eq!((order_count, ledger_count, journal_count), (0, 0, 0));
            let status: String =
                sqlx::query_scalar("SELECT status FROM convert_quotes WHERE quote_id = ?")
                    .bind(&quote_id)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(status, "quoted");
        }
        cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    }
    Ok(())
}
